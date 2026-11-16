//! Section styles in `.3dm` files (Rhino 8's `ON_SectionStyle`; spec `layouts.md` §3). Rhino 8
//! keeps an object's own style in its attributes (item 39, with item 29 saying the style comes
//! from the object), a layer's among the layer's items (item 35), and a clipping plane's as the
//! plane object's own style. Layout from openNURBS 8.x (`opennurbs_sectionstyle.cpp`,
//! `opennurbs_3dm_attributes.cpp`, `opennurbs_layer.cpp`), checked against files Rhino 8 saved.

use crate::archive::{Reader, Result, Writer, tcode};
use curva_document::clipping::SectionFill;
use curva_document::section::{SectionHatch, SectionStyle as Style};
use std::collections::BTreeMap;

/// Where an object's section style comes from, as Rhino numbers it: its layer.
pub const FROM_LAYER: u8 = 0;
/// Where an object's section style comes from: the object itself.
pub const FROM_OBJECT: u8 = 1;

/// Rhino's background fill modes: none, the viewport's (the object's colour), a colour.
const FILL_NONE: u8 = 0;
const FILL_VIEWPORT: u8 = 1;
const FILL_COLOUR: u8 = 2;

/// A section style as Rhino keeps it: the parts Curva carries.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionStyle {
    /// Rhino's background fill mode: 0 none, 1 the viewport's, 2 a colour.
    pub fill_mode: u8,
    /// The fill colour, red, green and blue, when set.
    pub fill_colour: Option<[u8; 3]>,
    /// The hatch pattern's index in the file's hatch pattern table, if it has one.
    pub hatch: Option<usize>,
    /// The hatch's scale.
    pub hatch_scale: f64,
    /// The hatch's rotation, in radians.
    pub hatch_rotation: f64,
}

impl Default for SectionStyle {
    /// Rhino's default style: the viewport's fill, no hatch.
    fn default() -> Self {
        Self {
            fill_mode: FILL_VIEWPORT,
            fill_colour: None,
            hatch: None,
            hatch_scale: 1.0,
            hatch_rotation: 0.0,
        }
    }
}

/// Writes a style as `ON_SectionStyle::Write` does (an anonymous chunk, version 1.1): its
/// component attributes, then the items that differ from Rhino's defaults, then 0.
pub fn write(w: &mut Writer, s: &SectionStyle) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 1);
    // The component attributes of a style with no name, index or id, as Rhino 8 writes them.
    w.begin_versioned(tcode::MODEL_ATTRIBUTES_CHUNK, 1, 0);
    w.u8(0);
    w.u8(1);
    w.uuid(crate::archive::Uuid::NIL);
    w.u8(0);
    w.u8(2);
    w.u8(2);
    w.end();
    if s.fill_mode != FILL_VIEWPORT {
        w.u8(1);
        w.u8(s.fill_mode);
    }
    if let Some([r, g, b]) = s.fill_colour {
        w.u8(2);
        w.color([r, g, b, 0]);
        w.color(crate::write::UNSET_COLOR);
    }
    if let Some(index) = s.hatch {
        w.u8(7);
        w.count(index);
    }
    if (s.hatch_scale - 1.0).abs() > f64::EPSILON {
        w.u8(8);
        w.f64(s.hatch_scale);
    }
    if s.hatch_rotation != 0.0 {
        w.u8(9);
        w.f64(s.hatch_rotation);
    }
    w.u8(0);
    w.end();
}

/// Reads a style written by `ON_SectionStyle::Write`. Items Curva does not carry (the boundary,
/// the fill rule, the hatch's colour) are read past; a boundary linetype ends the reading.
pub fn read(r: &mut Reader<'_>) -> Result<SectionStyle> {
    let (_, _, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    body.versioned(tcode::MODEL_ATTRIBUTES_CHUNK)?;
    let mut s = SectionStyle::default();
    loop {
        match body.u8()? {
            1 => s.fill_mode = body.u8()?,
            2 => {
                let c = body.color()?;
                body.color()?;
                // openNURBS writes an unset colour with all four bytes 0xFF.
                if c != [0xFF; 4] {
                    s.fill_colour = Some([c[0], c[1], c[2]]);
                }
            }
            3 => {
                body.bool()?;
            }
            4 | 10 => {
                body.color()?;
                body.color()?;
            }
            5 => {
                body.f64()?;
            }
            6 => {
                body.u8()?;
            }
            7 => s.hatch = usize::try_from(body.i32()?).ok(),
            8 => s.hatch_scale = body.f64()?,
            9 => s.hatch_rotation = body.f64()?,
            // 0 ends the items; 11 (a boundary linetype) and anything later are not carried.
            _ => break,
        }
    }
    Ok(s)
}

/// Curva's style as Rhino's: patterns by their index in the file's hatch pattern table, keyed
/// by their names in small letters.
pub fn to_rhino(style: &Style, pattern_index: &BTreeMap<String, i32>) -> SectionStyle {
    let (fill_mode, fill_colour) = match style.fill {
        SectionFill::None => (FILL_NONE, None),
        SectionFill::Solid(c) => (FILL_COLOUR, Some(c.map(colour_byte))),
    };
    let hatch = style.hatch.as_ref();
    SectionStyle {
        fill_mode,
        fill_colour,
        hatch: hatch
            .and_then(|h| pattern_index.get(&h.pattern.to_lowercase()))
            .and_then(|&i| usize::try_from(i).ok()),
        hatch_scale: hatch.map_or(1.0, |h| h.scale),
        hatch_rotation: hatch.map_or(0.0, |h| h.rotation),
    }
}

/// Rhino's style as Curva's: `patterns` names the file's hatch patterns by index. The
/// viewport's fill becomes Curva's standard fill.
pub fn from_rhino(s: &SectionStyle, patterns: &[String]) -> Style {
    let fill = match (s.fill_mode, s.fill_colour) {
        (FILL_NONE, _) => SectionFill::None,
        (FILL_COLOUR, Some(c)) => SectionFill::Solid(c.map(|v| f32::from(v) / 255.0)),
        _ => SectionFill::default(),
    };
    Style {
        fill,
        hatch: s
            .hatch
            .and_then(|i| patterns.get(i))
            .map(|pattern| SectionHatch {
                pattern: pattern.clone(),
                scale: s.hatch_scale,
                rotation: s.hatch_rotation,
            }),
    }
}

/// The hatch patterns section styles use, in small letters: the objects' own, every layer's and
/// every clipping plane's. The file's hatch pattern table must hold them.
pub fn patterns_used<'a>(
    state: &'a curva_document::State,
    objects: impl Iterator<Item = &'a curva_document::Object> + 'a,
) -> impl Iterator<Item = String> + 'a {
    let own = objects.filter_map(|o| o.section.as_ref());
    let layers = state.layers.iter().filter_map(|l| l.section.as_ref());
    let planes = state
        .clipping_planes
        .iter()
        .filter_map(|c| c.hatch.as_ref());
    own.chain(layers)
        .filter_map(|s| s.hatch.as_ref())
        .chain(planes)
        .map(|h| h.pattern.to_lowercase())
}

/// A colour channel from 0 to 1 as a byte.
fn colour_byte(v: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_reads_back_as_written() {
        let style = SectionStyle {
            fill_mode: FILL_COLOUR,
            fill_colour: Some([255, 0, 0]),
            hatch: Some(3),
            hatch_scale: 2.0,
            hatch_rotation: 0.5,
        };
        let mut w = Writer::default();
        write(&mut w, &style);
        let mut r = Reader::new(&w.out, 70);
        assert_eq!(read(&mut r).unwrap(), style);
    }

    #[test]
    fn styles_convert_both_ways() {
        let patterns = vec!["Solid".to_owned(), "Grid".to_owned()];
        let index: BTreeMap<String, i32> = [("solid".to_owned(), 0), ("grid".to_owned(), 1)]
            .into_iter()
            .collect();
        let curva = Style {
            fill: SectionFill::Solid([1.0, 0.0, 0.0]),
            hatch: Some(SectionHatch {
                pattern: "Grid".to_owned(),
                scale: 2.0,
                rotation: 0.5,
            }),
        };
        let rhino = to_rhino(&curva, &index);
        assert_eq!(rhino.fill_mode, FILL_COLOUR);
        assert_eq!(rhino.hatch, Some(1));
        assert_eq!(from_rhino(&rhino, &patterns), curva);
        let open = Style {
            fill: SectionFill::None,
            hatch: None,
        };
        assert_eq!(from_rhino(&to_rhino(&open, &index), &patterns), open);
        // Rhino's viewport fill comes in as Curva's standard fill.
        assert_eq!(
            from_rhino(&SectionStyle::default(), &patterns).fill,
            SectionFill::default()
        );
    }
}
