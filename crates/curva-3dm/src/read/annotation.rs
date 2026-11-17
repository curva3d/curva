//! Hatches, hatch patterns, linetypes, annotation styles and annotation as Rhino writes them.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Whether a hatch line or pattern is written the old (Rhino 5) way: always before version 60,
/// never after, and in version 60 files only when the next chunk is not an anonymous one
/// (openNURBS' `Internal_UseHatchReadV5`).
fn old_hatch_io(r: &Reader<'_>) -> bool {
    match r.version {
        v if v < 60 => true,
        60 => r.peek_tcode() != Some(tcode::ANONYMOUS_CHUNK),
        _ => false,
    }
}

fn doubles(r: &mut Reader<'_>) -> Result<Vec<f64>> {
    let n = r.count(8)?;
    (0..n).map(|_| r.f64()).collect()
}

/// A hatch line (`ON_HatchLine::Read`).
fn hatch_line(r: &mut Reader<'_>) -> Result<HatchLine> {
    let read = |r: &mut Reader<'_>| -> Result<HatchLine> {
        let angle = r.f64()?;
        let base = r.f64s::<2>()?;
        let offset = r.f64s::<2>()?;
        let dashes = doubles(r)?;
        Ok(HatchLine {
            angle,
            base,
            offset,
            dashes,
        })
    };
    if old_hatch_io(r) {
        r.chunk_version()?;
        read(r)
    } else {
        let (_, mut body) = r.anonymous()?;
        read(&mut body)
    }
}

/// A hatch pattern table record (`ON_HatchPattern::Read`, or `ReadV5` in older files).
pub(super) fn hatch_pattern(record: &mut Reader<'_>) -> Result<Option<HatchPattern>> {
    let (id, mut r) = class_object(record)?;
    if id != class::HATCH_PATTERN {
        return Ok(None);
    }
    let lines_of = |r: &mut Reader<'_>, count: usize| -> Result<Vec<HatchLine>> {
        (0..count).map(|_| hatch_line(r)).collect()
    };
    if old_hatch_io(&r) {
        let (_, minor) = r.chunk_version()?;
        let _index = r.i32()?;
        let kind = r.i32()?;
        let name = r.wstring()?;
        let description = r.wstring()?;
        let lines = if kind == 1 {
            let count = usize::try_from(r.i32()?).unwrap_or(0);
            lines_of(&mut r, count)?
        } else {
            Vec::new()
        };
        let id = if minor >= 2 { r.uuid()? } else { Uuid::NIL };
        return Ok(Some(HatchPattern {
            id,
            name,
            description,
            solid: kind != 1,
            lines,
        }));
    }
    let (_, mut body) = r.anonymous()?;
    let (id, name) = model_component(&mut body)?;
    let kind = body.u32()?;
    let description = body.wstring()?;
    let mut lines_chunk = body.expect(tcode::ANONYMOUS_CHUNK)?;
    let count = usize::try_from(lines_chunk.body.u32()?).unwrap_or(0);
    let lines = lines_of(&mut lines_chunk.body, count)?;
    Ok(Some(HatchPattern {
        id,
        name,
        description,
        solid: kind != 1,
        lines,
    }))
}

/// A linetype table record (`ON_Linetype::Read`): Rhino 6 and later write version 2 (the
/// component's attributes, then the segments), Rhino 5 version 1 (index, name, segments, id).
pub(super) fn linetype(record: &mut Reader<'_>) -> Result<Option<Linetype>> {
    let (class, mut r) = class_object(record)?;
    if class != class::LINETYPE {
        return Ok(None);
    }
    let (major, minor, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let segments = |body: &mut Reader<'_>| -> Result<Vec<(f64, bool)>> {
        let n = body.count(12)?;
        (0..n)
            .map(|_| Ok((body.f64()?, body.i32()? == 0)))
            .collect()
    };
    if major == 1 {
        let _index = body.i32()?;
        let name = body.wstring()?;
        let segments = segments(&mut body)?;
        let id = if minor >= 1 { body.uuid()? } else { Uuid::NIL };
        return Ok(Some(Linetype { id, name, segments }));
    }
    let (id, name) = component_attributes(&mut body)?;
    let segments = segments(&mut body)?;
    Ok(Some(Linetype { id, name, segments }))
}

/// openNURBS' `ReadModelComponentAttributes` for components written since Rhino 6: a bit set
/// saying which of id, parent id, index, name and status follow. Returns the id and name.
fn model_component(r: &mut Reader<'_>) -> Result<(Uuid, String)> {
    let (_, mut body) = r.anonymous()?;
    let bits = body.u32()?;
    let mut id = Uuid::NIL;
    let mut name = String::new();
    if bits & 0x01 != 0 {
        id = body.uuid()?;
    }
    if bits & 0x02 != 0 {
        body.uuid()?;
    }
    if bits & 0x04 != 0 {
        body.i32()?;
    }
    if bits & 0x08 != 0 {
        name = body.wstring()?;
    }
    Ok((id, name))
}

/// A hatch (`ON_Hatch::Read`): its plane, scale, turn, pattern, outlines and base point.
pub(super) fn hatch(r: &mut Reader<'_>) -> Result<Hatch> {
    let (_, minor) = r.chunk_version()?;
    let plane = plane(r)?;
    let scale = r.f64()?;
    let rotation = r.f64()?;
    let pattern = r.i32()?;
    let count = usize::try_from(r.i32()?).map_err(|_| bad("a bad loop count"))?;
    let mut loops = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        r.chunk_version()?;
        let outer = r.i32()? == 0;
        loops.push((outer, curve(r)?));
    }
    let base = if minor >= 2 { r.f64s::<2>()? } else { [0.0; 2] };
    Ok(Hatch {
        plane,
        scale,
        rotation,
        pattern,
        loops,
        base,
    })
}

/// A Rhino 6+ annotation style (`ON_DimStyle::Write`, version 1), read as far as the notes'
/// alignment: its id and name, sizes (the seventh is the text height), then every field in
/// order up to the alignments it also keeps in Rhino 5's numbering.
fn dim_style_fields(r: &mut Reader<'_>) -> Result<DimStyle> {
    let (minor, mut b) = r.anonymous()?;
    let (id, name) = component_attributes(&mut b)?;
    let sizes = b.f64s::<7>()?;
    // Text alignment (obsolete), angle format, length format, angle resolution.
    b.skip(4 * 4)?;
    let length_resolution = b.i32()?;
    b.i32()?; // the ignored text style index of Rhino 6 files
    b.f64()?; // length factor
    b.bool()?; // alternate units
    b.f64()?; // alternate length factor
    b.skip(4 * 2)?; // alternate length format and resolution
    for _ in 0..4 {
        b.wstring()?; // prefixes and suffixes
    }
    b.f64()?; // dimension line extension
    b.skip(2)?; // extension lines suppressed
    b.uuid()?; // parent style
    b.i32()?; // legacy override count
    if b.bool()? {
        let count = b.count(1)?;
        b.skip(count)?; // which fields are overridden
    }
    b.skip(4 * 2 + 8 * 4)?; // tolerance format, resolution, upper, lower, height, baseline
    b.skip(1 + 4 + 4)?; // text mask on, fill type, colour
    b.skip(8 + 4 + 16)?; // dimension scale, its source, source style
    b.skip(4 + 16 + 4 + 16 + 2 + 16)?; // colour sources and colours, plot weights
    b.skip(8 + 1)?; // fixed extension length
    b.skip(8 + 4 + 8)?; // text rotation, alternate tolerance resolution, tolerance height
    b.skip(2 + 4 + 4 + 8)?; // arrows suppressed, text move, arc symbol, stack height
    b.skip(4 + 8 * 3)?; // stack format, roundings
    b.skip(4 * 4 + 1)?; // zero suppression, alternate below
    b.skip(4 * 3 + 16 * 3)?; // arrow types and blocks
    b.i32()?; // leader content type (obsolete)
    let v_align = b.u32()?;
    b.skip(4 + 4 + 4 + 8 + 1 + 8)?; // leader alignment, angle style, curve, angle, landing
    let h_align = b.u32()?;
    let mut font = None;
    if minor >= 2 {
        b.u32()?; // leader horizontal alignment
        b.skip(2)?; // draw forward, signed ordinate
        b.chunk()?; // the style's scale
        b.u32()?; // unit system
        font = font_name(&mut b).ok().flatten();
    }
    Ok(DimStyle {
        id,
        name,
        text_height: sizes[6],
        h_align,
        v_align,
        extension_extension: sizes[0],
        extension_offset: sizes[1],
        arrow_size: sizes[2],
        text_gap: sizes[5],
        length_resolution,
        font,
    })
}

/// A font's family name (`ON_Font::Read`, Rhino 6 and later): its characteristics, then its
/// Windows name as a UTF-8 string chunk, which names the family.
fn font_name(r: &mut Reader<'_>) -> Result<Option<String>> {
    let (_, mut f) = r.anonymous()?;
    f.u32()?; // weight, style, stretch, underline and strikethrough
    let mut name = f.expect(tcode::UTF8_STRING_CHUNK)?;
    if name.body.u8()? != 1 {
        return Ok(None);
    }
    let left = name.body.left();
    let text = String::from_utf8_lossy(name.body.bytes(left)?)
        .trim_end_matches('\0')
        .trim()
        .to_owned();
    Ok((!text.is_empty()).then_some(text))
}

/// Where a Rhino 5 style sits: its index in the style table and its font's in the font table.
type V5Place = (i32, i32);

/// A dimension style table record. Rhino 5's form also gives its index in the table and its
/// font's, which annotation and the style name it by.
pub(super) fn dim_style(record: &mut Reader<'_>) -> Result<Option<(DimStyle, Option<V5Place>)>> {
    let (id, mut r) = class_object(record)?;
    if id != class::DIM_STYLE && id != class::V5_DIM_STYLE {
        return Ok(None);
    }
    if r.version < 60 {
        return v5_dim_style(&mut r).map(|(style, at)| Some((style, Some(at))));
    }
    if r.peek_tcode() != Some(tcode::ANONYMOUS_CHUNK) {
        return Ok(None);
    }
    dim_style_fields(&mut r).map(|style| Some((style, None)))
}

/// A Rhino 5 annotation style (`ON_V5x_DimStyle::Internal_Read_v5`): a bare chunk version, its
/// index, name, extension line extension and offset, arrow size, centre mark, text gap, text
/// alignment, arrow type, angle units, length and angle formats, length and angle resolutions,
/// its font's index, then (from version 1.1) its text height and (from 1.3) its id. Notes'
/// alignment takes Rhino 8's default for a Rhino 5 style: left and top.
fn v5_dim_style(r: &mut Reader<'_>) -> Result<(DimStyle, (i32, i32))> {
    let (major, minor) = r.chunk_version()?;
    if major != 1 {
        return Err(bad(format!(
            "a Rhino 5 annotation style of version {major}"
        )));
    }
    let index = r.i32()?;
    let name = r.wstring()?;
    let [
        extension_extension,
        extension_offset,
        arrow_size,
        _centre_mark,
        text_gap,
    ] = r.f64s::<5>()?;
    r.skip(4 * 5)?; // text alignment, arrow type, angle units, length and angle formats
    let length_resolution = r.i32()?;
    r.i32()?; // angle resolution
    let font = r.i32()?;
    let text_height = if minor >= 1 { r.f64()? } else { 1.0 };
    // Without an id, the style is named by one made from its index, as Rhino gives it a new one.
    let mut id = Uuid([0; 16]);
    id.0[..4].copy_from_slice(b"crv5");
    id.0[12..].copy_from_slice(&index.to_le_bytes());
    if minor >= 3 {
        r.f64()?; // length factor
        r.wstring()?; // prefix
        r.wstring()?; // suffix
        r.bool()?; // alternate units
        r.f64()?; // alternate length factor
        r.skip(4 * 4)?; // alternate length and angle formats and resolutions
        r.wstring()?; // alternate prefix
        r.wstring()?; // alternate suffix
        r.u32()?; // unused
        let own = r.uuid()?;
        if own != Uuid::NIL {
            id = own;
        }
    }
    Ok((
        DimStyle {
            id,
            name,
            text_height,
            h_align: 0,
            v_align: 3,
            arrow_size,
            extension_extension,
            extension_offset,
            text_gap,
            length_resolution,
            font: None,
        },
        (index, font),
    ))
}

/// A Rhino 5 font table record (`ON_Font::ReadV5`): a bare chunk version, its index, a
/// description, then its Windows face name as 64 UTF-16 units, which names the family.
pub(super) fn v5_font(record: &mut Reader<'_>) -> Result<Option<(i32, String)>> {
    let (_, mut r) = class_object(record)?;
    if r.chunk_version()?.0 != 1 {
        return Ok(None);
    }
    let index = r.i32()?;
    r.wstring()?; // description
    let units = (0..64).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    let face = String::from_utf16_lossy(&units[..end]).trim().to_owned();
    Ok((!face.is_empty()).then_some((index, face)))
}

/// The id Rhino 5 annotation is given while its file is read, standing for its style's index in
/// the style table until that is known.
fn v5_style_ref(index: i32) -> Uuid {
    let mut id = Uuid([0; 16]);
    id.0[..4].copy_from_slice(b"ref5");
    id.0[12..].copy_from_slice(&index.to_le_bytes());
    id
}

/// Names Rhino 5 annotation's style by its id, from the index it was read with; the default
/// style when the index names none.
pub(super) fn name_v5_style(geometry: &mut Geometry, styles: &[(i32, Uuid)]) {
    let style = match geometry {
        Geometry::Text(Text { style, .. })
        | Geometry::Dimension(LinearDimension { style, .. })
        | Geometry::Radial(RadialDimension { style, .. })
        | Geometry::Angular(AngularDimension { style, .. })
        | Geometry::Leader(Leader { style, .. }) => style,
        _ => return,
    };
    if style.0[..4] == *b"ref5" {
        let index = i32::from_le_bytes([style.0[12], style.0[13], style.0[14], style.0[15]]);
        *style = styles
            .iter()
            .find(|(i, _)| *i == index)
            .map_or(Uuid::NIL, |(_, id)| *id);
    }
}

/// A note (`ON_Text::Read`, then `ON_Annotation::Internal_ReadAnnotation` and
/// `ON_TextContent::Read`).
/// The part every annotation shares (`ON_Annotation::Internal_ReadAnnotation`): its words, the
/// turn they carry, its style, plane, kind (openNURBS' annotation type, from version 1) and the
/// style it sets for itself (from version 2).
struct AnnotationCore {
    rich: String,
    rotation: f64,
    style: Uuid,
    plane: Plane,
    kind: u32,
    own: Option<DimStyle>,
}

fn annotation_core(r: &mut Reader<'_>) -> Result<AnnotationCore> {
    let (version, mut a) = r.anonymous()?;
    let (_, mut content) = a.anonymous()?;
    let rich = content.wstring()?;
    let _obsolete_plane = plane(&mut content)?;
    let _width = content.f64()?;
    let rotation = content.f64()?;
    let style = a.uuid()?;
    let plane = plane(&mut a)?;
    let kind = if version >= 1 { a.u32()? } else { 0 };
    let mut own = None;
    if version >= 2 {
        let (_, mut overrides) = a.anonymous()?;
        if overrides.bool()? {
            let (id, mut data) = class_object(&mut overrides)?;
            if id == class::DIM_STYLE {
                own = Some(dim_style_fields(&mut data)?);
            }
        }
    }
    Ok(AnnotationCore {
        rich,
        rotation,
        style,
        plane,
        kind,
        own,
    })
}

pub(super) fn text(r: &mut Reader<'_>) -> Result<Text> {
    let (_, mut outer) = r.anonymous()?;
    let core = annotation_core(&mut outer)?;
    Ok(Text {
        rich: core.rich,
        plane: core.plane,
        style: core.style,
        own: core.own,
        rotation: core.rotation,
    })
}

/// A linear dimension (`ON_DimLinear::Read` around `ON_Dimension::Internal_ReadDimension`):
/// the annotation, its typed text, then its second point and dimension line point in its plane.
pub(super) fn linear_dimension(r: &mut Reader<'_>) -> Result<LinearDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let core = annotation_core(&mut d)?;
    let text = d.wstring()?;
    let second = outer.f64s::<2>()?;
    let line = outer.f64s::<2>()?;
    Ok(LinearDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        rotated: core.kind == 5,
        text,
        second,
        line,
    })
}

/// The part dimensions share after the annotation (`ON_Dimension::Internal_ReadDimension`):
/// returns the annotation and the typed text; the rest is how Rhino draws it by default.
fn dimension_core(d: &mut Reader<'_>) -> Result<(AnnotationCore, String)> {
    let core = annotation_core(d)?;
    let text = d.wstring()?;
    Ok((core, text))
}

/// A radius or diameter dimension (`ON_DimRadial::Read`): the dimension, then the point on the
/// circle and the text point, in its plane (centred on the circle).
pub(super) fn radial_dimension(r: &mut Reader<'_>) -> Result<RadialDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let (core, text) = dimension_core(&mut d)?;
    let radius_point = outer.f64s::<2>()?;
    let text_point = outer.f64s::<2>()?;
    Ok(RadialDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        diameter: core.kind == 3,
        text,
        radius_point,
        text_point,
    })
}

/// An angle dimension (`ON_DimAngular::Read`): the dimension, its sides' directions, where
/// its extension lines start, and a point on its arc, in its plane (at the corner).
pub(super) fn angular_dimension(r: &mut Reader<'_>) -> Result<AngularDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let (core, text) = dimension_core(&mut d)?;
    let sides = [outer.f64s::<2>()?, outer.f64s::<2>()?];
    let offsets = [outer.f64()?, outer.f64()?];
    let arc_point = outer.f64s::<2>()?;
    Ok(AngularDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        text,
        sides,
        offsets,
        arc_point,
    })
}

/// An ordinate dimension (`ON_DimOrdinate::Read`): the dimension, which coordinate it gives, the
/// point measured and the leader's end in its plane (at the base point), and its kink offsets.
pub(super) fn ordinate_dimension(r: &mut Reader<'_>) -> Result<OrdinateDimension> {
    let (_, mut outer) = r.anonymous()?;
    let (_, mut d) = outer.anonymous()?;
    let (core, text) = dimension_core(&mut d)?;
    let direction = outer.u32()?;
    let point = outer.f64s::<2>()?;
    let leader = outer.f64s::<2>()?;
    let kinks = [outer.f64()?, outer.f64()?];
    Ok(OrdinateDimension {
        plane: core.plane,
        style: core.style,
        own: core.own,
        text,
        direction,
        point,
        leader,
        kinks,
    })
}

/// A leader (`ON_Leader::Read`, version 1): the annotation, then its points in its plane.
pub(super) fn leader(r: &mut Reader<'_>) -> Result<Leader> {
    let (_, mut outer) = r.anonymous()?;
    let core = annotation_core(&mut outer)?;
    let count = outer.count(16)?;
    let points = (0..count)
        .map(|_| outer.f64s::<2>())
        .collect::<Result<Vec<_>>>()?;
    Ok(Leader {
        plane: core.plane,
        style: core.style,
        own: core.own,
        rich: core.rich,
        points,
    })
}

/// A Rhino 5 note: Rhino 5 put an unjustified note's top left at its plane's origin; its
/// justification bits as Rhino 6's alignments.
fn v5_note(words: String, plane: Plane, height: f64, justification: u32) -> Geometry {
    let (h_align, v_align) = if justification == 0 {
        (0, 3)
    } else {
        let h = if justification & 2 != 0 {
            1
        } else if justification & 4 != 0 {
            2
        } else {
            0
        };
        let v = if justification & (1 << 18) != 0 {
            3
        } else if justification & (1 << 17) != 0 {
            5
        } else {
            7
        };
        (h, v)
    };
    Geometry::Text(Text {
        rich: words,
        plane,
        style: v5_style_ref(0),
        own: Some(DimStyle {
            id: Uuid::NIL,
            name: String::new(),
            text_height: height,
            h_align,
            v_align,
            arrow_size: 1.0,
            extension_extension: 1.0,
            extension_offset: 0.5,
            text_gap: 0.25,
            length_resolution: 2,
            font: None,
        }),
        rotation: 0.0,
    })
}

/// A Rhino 5 linear (`kind` 1) or aligned (2) dimension: the first point at [0] (the plane's
/// origin), the second at [2], the dimension line from [1] to [3], always along the plane's X.
/// An aligned one's plane runs from its first point to its second; when it does not, Rhino
/// measures it along X as well.
fn v5_linear(kind: u32, points: &[[f64; 2]], plane: Plane, style: Uuid, words: String) -> Geometry {
    let point = |i: usize| points.get(i).copied().unwrap_or([0.0, 0.0]);
    let less = |p: [f64; 2], q: [f64; 2]| [p[0] - q[0], p[1] - q[1]];

    let first = point(0);
    let (from, to) = (point(1), point(3));
    let middle = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
    let along_x = kind == 1 || (point(2)[1] - first[1]).abs() > 1e-9;
    Geometry::Dimension(LinearDimension {
        plane: Plane {
            origin: std::array::from_fn(|k| {
                plane.origin[k] + plane.x[k] * first[0] + plane.y[k] * first[1]
            }),
            ..plane
        },
        style,
        own: None,
        rotated: along_x,
        text: words,
        second: less(point(2), first),
        line: less(middle, first),
    })
}

/// Rhino 5's annotation (`ON_OBSOLETE_V5_Annotation::Read` and its kinds' `Read`): its kind, plane,
/// points in that plane, typed words and text height; an angle dimension adds its angle and
/// radius. Files from openNURBS of October 2007 on wrap each part in a chunk; older ones start
/// with a bare chunk version. The style is named by its index in Rhino 5's style table, resolved
/// once the file is read; a note goes on the first style whatever it names, as Rhino 8 reads it,
/// keeping its height and alignment as its own.
pub(super) fn v5_annotation(r: &mut Reader<'_>) -> Result<std::result::Result<Geometry, String>> {
    let chunked = r.peek_tcode() == Some(tcode::ANONYMOUS_CHUNK);
    // A kind's own chunk, then the shared part's; older files have only a bare chunk version.
    let mut a = if chunked {
        r.anonymous()?.1.anonymous()?.1
    } else {
        r.chunk_version()?;
        r.clone()
    };
    let kind = a.u32()?;
    let _display = a.u32()?;
    let plane = plane(&mut a)?;
    let count = a.count(16)?;
    let points = (0..count)
        .map(|_| a.f64s::<2>())
        .collect::<Result<Vec<_>>>()?;
    let words = a.wstring()?;
    let _user_positioned = a.u32()?;
    let style = v5_style_ref(a.i32()?);
    let height = a.f64()?;
    let justification = if chunked { a.u32()? } else { 0 };
    let point = |i: usize| points.get(i).copied().unwrap_or([0.0, 0.0]);
    let less = |p: [f64; 2], q: [f64; 2]| [p[0] - q[0], p[1] - q[1]];
    Ok(Ok(match kind {
        // A note.
        7 => v5_note(words, plane, height, justification),
        // Linear (1) and aligned (2).
        1 | 2 => v5_linear(kind, &points, plane, style, words),
        // Diameter (4) and radius (5): the centre at [0], the arrow at [1], the text at [2].
        4 | 5 => {
            let centre = point(0);
            Geometry::Radial(RadialDimension {
                plane: Plane {
                    origin: std::array::from_fn(|k| {
                        plane.origin[k] + plane.x[k] * centre[0] + plane.y[k] * centre[1]
                    }),
                    ..plane
                },
                style,
                own: None,
                diameter: kind == 4,
                text: words,
                radius_point: less(point(1), centre),
                text_point: less(point(2), centre),
            })
        }
        // An angle (3): the corner at the plane's origin, a point on each side at [1] and [2], a
        // point on the arc at [3].
        3 => {
            let length = |p: [f64; 2]| (p[0] * p[0] + p[1] * p[1]).sqrt();
            let (start, end) = (point(1), point(2));
            Geometry::Angular(AngularDimension {
                plane,
                style,
                own: None,
                text: words,
                sides: [start, end],
                offsets: [length(start), length(end)],
                arc_point: point(3),
            })
        }
        // A leader (6): the arrow's tip first, the words at the last point.
        6 => Geometry::Leader(Leader {
            plane,
            style,
            own: None,
            rich: words,
            points,
        }),
        other => return Ok(Err(format!("Rhino 5 annotation of kind {other}"))),
    }))
}
