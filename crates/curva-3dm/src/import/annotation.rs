//! Rhino's hatches, dimensions, leaders, notes and annotation styles as Curva's.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A hatch from a file, in the file's unit (the caller scales it): its outlines placed from its
/// plane into the world, and its pattern found by name among Curva's, else added from the file.
pub(super) fn hatch_in(
    h: &model::Hatch,
    patterns: &[model::HatchPattern],
    state: &mut State,
    file_mm: f64,
) -> Result<curva_document::annotation::Hatch, String> {
    let p = &h.plane;
    let out: [[f64; 4]; 4] = [
        [p.x[0], p.x[1], p.x[2], 0.0],
        [p.y[0], p.y[1], p.y[2], 0.0],
        [p.z[0], p.z[1], p.z[2], 0.0],
        [p.origin[0], p.origin[1], p.origin[2], 1.0],
    ];
    let loops = h
        .loops
        .iter()
        .map(|(_, c)| {
            curve(c)
                .and_then(|flat| flat.mapped_affine(out))
                .map_err(|e| e.0)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let from_file = usize::try_from(h.pattern)
        .ok()
        .and_then(|i| patterns.get(i));
    let (pattern, scale) = match from_file {
        None => ("Solid".to_owned(), 1.0),
        Some(file) if file.solid => ("Solid".to_owned(), 1.0),
        Some(file) => {
            // The file's own pattern, as Rhino draws it, even where Curva has one of the same
            // name: Rhino's Plus, Grid and the rest differ from Curva's own (#477). A pattern
            // the model already holds, from this file or before it, is kept, as Rhino keeps it.
            let held = state
                .hatch_patterns
                .iter()
                .find(|q| q.name.eq_ignore_ascii_case(&file.name))
                .map(|q| q.name.clone());
            match held {
                Some(name) => (name, h.scale),
                None => {
                    // Kept in millimetres.
                    let made = curva_document::hatch::Pattern {
                        name: file.name.clone(),
                        description: file.description.clone(),
                        lines: file
                            .lines
                            .iter()
                            .map(|l| curva_document::hatch::PatternLine {
                                angle: l.angle.to_degrees(),
                                origin: [l.base[0] * file_mm, l.base[1] * file_mm],
                                shift: l.offset[0] * file_mm,
                                spacing: l.offset[1] * file_mm,
                                dashes: l.dashes.iter().map(|d| d * file_mm).collect(),
                            })
                            .collect(),
                    };
                    state.hatch_patterns.push(made);
                    (file.name.clone(), h.scale)
                }
            }
        }
    };
    let base: P3 = std::array::from_fn(|k| p.origin[k] + p.x[k] * h.base[0] + p.y[k] * h.base[1]);
    Ok(curva_document::annotation::Hatch {
        loops,
        pattern,
        scale,
        rotation: h.rotation,
        base,
        x: p.x,
        y: p.y,
    })
}

/// A linear dimension from a file: its points out of its plane, measured along the plane's X
/// (rotated) or from point to point (aligned). Its looks come from Curva's current style.
pub(super) fn dimension_in(d: &model::LinearDimension) -> curva_document::annotation::Dimension {
    use curva_document::annotation::DimensionKind;
    let p = &d.plane;
    let at = |q: [f64; 2]| -> P3 {
        std::array::from_fn(|k| p.origin[k] + p.x[k] * q[0] + p.y[k] * q[1])
    };
    let text = d.text.trim();
    curva_document::annotation::Dimension {
        kind: if d.rotated {
            DimensionKind::Horizontal
        } else {
            DimensionKind::Aligned
        },
        first: p.origin,
        second: at(d.second),
        line: at(d.line),
        x: p.x,
        y: p.y,
        style: 0,
        text: (!text.is_empty() && text != "<>").then(|| d.text.clone()),
    }
}

/// A point in a plane's coordinates, in the world.
fn in_plane(p: &model::Plane, q: [f64; 2]) -> P3 {
    std::array::from_fn(|k| p.origin[k] + p.x[k] * q[0] + p.y[k] * q[1])
}

/// Typed dimension text, or none when it is only the value (`<>`).
fn typed(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty() && trimmed != "<>").then(|| text.to_owned())
}

/// A radius or diameter dimension from a file.
pub(super) fn radial_in(d: &model::RadialDimension) -> curva_document::annotation::RadialDimension {
    curva_document::annotation::RadialDimension {
        diameter: d.diameter,
        centre: d.plane.origin,
        on_curve: in_plane(&d.plane, d.radius_point),
        text_at: in_plane(&d.plane, d.text_point),
        x: d.plane.x,
        y: d.plane.y,
        style: 0,
        text: typed(&d.text),
    }
}

/// An ordinate dimension from a file: the coordinate it gives as Rhino implies it when the file
/// leaves it unset, and kinks at the default when unset.
pub(super) fn ordinate_in(
    d: &model::OrdinateDimension,
) -> curva_document::annotation::OrdinateDimension {
    use curva_document::annotation::OrdinateAxis;
    let axis = match d.direction {
        1 => OrdinateAxis::X,
        2 => OrdinateAxis::Y,
        _ => OrdinateAxis::implied(d.point, d.leader),
    };
    // openNURBS counts anything below its unset value's neighbourhood as unset.
    let set = |k: f64| k.is_finite() && k > -1.0e300;
    curva_document::annotation::OrdinateDimension {
        base: d.plane.origin,
        point: in_plane(&d.plane, d.point),
        leader: in_plane(&d.plane, d.leader),
        axis,
        kinks: (set(d.kinks[0]) && set(d.kinks[1])).then_some(d.kinks),
        x: d.plane.x,
        y: d.plane.y,
        style: 0,
        text: typed(&d.text),
    }
}

/// An angle dimension from a file: a point on each side where its extension line starts (or,
/// with none, as far out as its arc).
pub(super) fn angular_in(
    d: &model::AngularDimension,
) -> curva_document::annotation::AngleDimension {
    let arc = (d.arc_point[0].powi(2) + d.arc_point[1].powi(2)).sqrt();
    let side = |i: usize| {
        let s = d.sides[i];
        let length = (s[0] * s[0] + s[1] * s[1]).sqrt().max(1e-12);
        let reach = if d.offsets[i] > 1e-9 {
            d.offsets[i]
        } else {
            arc
        };
        in_plane(&d.plane, [s[0] / length * reach, s[1] / length * reach])
    };
    curva_document::annotation::AngleDimension {
        apex: d.plane.origin,
        first: side(0),
        second: side(1),
        arc_at: in_plane(&d.plane, d.arc_point),
        x: d.plane.x,
        y: d.plane.y,
        style: 0,
        text: typed(&d.text),
    }
}

/// Words whose `LayoutUserText` fields name a page by its id, with that page's layout number
/// instead, as Curva's field reads it.
fn layout_numbers_in(words: &str, page_ids: &BTreeMap<crate::archive::Uuid, u64>) -> String {
    let mut out = String::new();
    let mut rest = words;
    while let Some(start) = rest.find("%<LayoutUserText(") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(">%") else {
            break;
        };
        let field = &rest[start..start + end + 2];
        let swapped = match crate::export::field_args(field).as_deref() {
            Some([page, key]) => page_ids
                .get(&crate::archive::Uuid::parse(page))
                .map(|layout| format!("%<LayoutUserText(\"{layout}\",\"{key}\")>%")),
            _ => None,
        };
        out.push_str(swapped.as_deref().unwrap_or(field));
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Words whose fields measuring an object name it by the file's id, with Curva's number for it
/// instead; a field naming an object that did not come in is kept as it is.
pub(super) fn object_numbers_in(
    words: &str,
    numbers: &BTreeMap<crate::archive::Uuid, u64>,
) -> String {
    let mut out = String::new();
    let mut rest = words;
    while let Some(start) = rest.find("%<") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(">%") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let field = &rest[start..start + end + 2];
        let name: String = field[2..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        let swapped = crate::export::OBJECT_FIELDS
            .contains(&name.as_str())
            .then(|| {
                crate::export::with_first_part(field, |id| {
                    numbers
                        .get(&crate::archive::Uuid::parse(id.trim()))
                        .map(u64::to_string)
                })
            })
            .flatten();
        out.push_str(swapped.as_deref().unwrap_or(field));
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// A note or leader with its layout fields naming Curva's layouts.
pub(super) fn with_layout_numbers(
    shape: Shape,
    page_ids: &BTreeMap<crate::archive::Uuid, u64>,
) -> Shape {
    use curva_document::annotation::Annotation;
    match shape {
        Shape::Annotation(Annotation::Text(mut t)) => {
            t.content = layout_numbers_in(&t.content, page_ids);
            Shape::Annotation(Annotation::Text(t))
        }
        Shape::Annotation(Annotation::Leader(mut l)) => {
            l.content = layout_numbers_in(&l.content, page_ids);
            Shape::Annotation(Annotation::Leader(l))
        }
        other => other,
    }
}

/// A file's linetype as the document's: one of Curva's own when the name matches (Rhino's
/// standard linetypes share Curva's names), otherwise the document's own with the file's
/// pattern.
pub(super) fn linetype_in(state: &mut State, linetype: &model::Linetype) -> Linetype {
    let name = linetype.name.trim();
    if let Some(own) = Linetype::named(name).filter(|l| !l.is_by_layer()) {
        return own;
    }
    // Rhino's lengths are millimetres on paper whatever the model's unit, as Curva's are.
    state.add_linetype(name, &curva_document::alternating(&linetype.segments))
}

/// A leader from a file, its words made plain and a landing drawn, as Rhino's usual style does.
pub(super) fn leader_in(l: &model::Leader) -> curva_document::annotation::Leader {
    curva_document::annotation::Leader {
        points: l.points.iter().map(|q| in_plane(&l.plane, *q)).collect(),
        content: crate::notes::plain_text(&l.rich),
        x: l.plane.x,
        y: l.plane.y,
        landing: true,
        style: 0,
    }
}

/// The file's annotation styles as the document's, by Rhino id: one of the same name kept,
/// otherwise a new one with the file's font, sizes (from the file's unit to the style's
/// millimetres, or inches in an imperial model) and decimals. A style Curva wrote for a note of
/// another height or alignment ("ISO 3.5 7 Centre") goes back to the style it was made from.
pub(super) fn styles_in(
    state: &mut State,
    styles: &[model::DimStyle],
    file_mm: f64,
) -> BTreeMap<crate::archive::Uuid, u64> {
    use curva_document::annotation::{AnnotationStyle, standard_styles};
    let imperial = matches!(state.unit, Unit::Feet | Unit::Inches);
    if state.annotation_styles.is_empty() && !styles.is_empty() {
        state.annotation_styles = standard_styles(imperial);
    }
    let per_unit = file_mm / if imperial { 25.4 } else { 1.0 };
    let mut out = BTreeMap::new();
    for style in styles {
        let name = style.name.trim();
        let base = note_style_base(name, styles).unwrap_or(name);
        let id = match state
            .annotation_styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(base))
        {
            Some(held) => held.id,
            None => {
                let id = state
                    .annotation_styles
                    .iter()
                    .map(|s| s.id)
                    .max()
                    .map_or(0, |m| m + 1);
                state.annotation_styles.push(AnnotationStyle {
                    id,
                    name: base.to_owned(),
                    font: style.font.clone(),
                    text_height: style.text_height * per_unit,
                    imperial,
                    arrow_size: Some(style.arrow_size * per_unit),
                    extension_offset: Some(style.extension_offset * per_unit),
                    extension_extension: Some(style.extension_extension * per_unit),
                    text_gap: Some(style.text_gap * per_unit),
                    linear_resolution: u8::try_from(style.length_resolution).ok(),
                });
                id
            }
        };
        out.insert(style.id, id);
    }
    out
}

/// For a style named as Curva names one made for a note ("ISO 3.5 7 Centre"), the style it was
/// made from, when the file has it.
fn note_style_base<'a>(name: &'a str, styles: &[model::DimStyle]) -> Option<&'a str> {
    let (rest, align) = name.rsplit_once(' ')?;
    if !matches!(align, "Left" | "Centre" | "Right") {
        return None;
    }
    let (base, height) = rest.rsplit_once(' ')?;
    height.parse::<f64>().ok()?;
    styles.iter().any(|s| s.name.trim() == base).then_some(base)
}

/// The Rhino style an annotation from the file names.
pub(super) fn style_named(geometry: &Geometry) -> Option<crate::archive::Uuid> {
    match geometry {
        Geometry::Text(t) => Some(t.style),
        Geometry::Dimension(d) => Some(d.style),
        Geometry::Radial(d) => Some(d.style),
        Geometry::Angular(d) => Some(d.style),
        Geometry::Ordinate(d) => Some(d.style),
        Geometry::Leader(l) => Some(l.style),
        _ => None,
    }
}

/// An annotation with its Curva style set.
pub(super) fn with_style(shape: Shape, style: u64) -> Shape {
    use curva_document::annotation::Annotation;
    match shape {
        Shape::Annotation(Annotation::Text(mut t)) => {
            t.style = style;
            Shape::Annotation(Annotation::Text(t))
        }
        Shape::Annotation(Annotation::Dimension(mut d)) => {
            d.style = style;
            Shape::Annotation(Annotation::Dimension(d))
        }
        Shape::Annotation(Annotation::Radial(mut d)) => {
            d.style = style;
            Shape::Annotation(Annotation::Radial(d))
        }
        Shape::Annotation(Annotation::Angle(mut d)) => {
            d.style = style;
            Shape::Annotation(Annotation::Angle(d))
        }
        Shape::Annotation(Annotation::Leader(mut l)) => {
            l.style = style;
            Shape::Annotation(Annotation::Leader(l))
        }
        other => other,
    }
}
