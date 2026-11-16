//! Curva's hatches, dimensions and annotation styles as Rhino's.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A hatch as Rhino holds one: its plane at the hatch's base point, its outlines as 2D curves
/// in that plane, and its pattern by its place in the file's table.
pub(super) fn hatch(
    h: &curva_document::annotation::Hatch,
    patterns: &BTreeMap<String, i32>,
) -> Result<model::Hatch, String> {
    let z = cross(h.x, h.y);
    let o = h.base;
    // From the world into the plane: each axis's coordinate, measured from the base point.
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let into: [[f64; 4]; 4] = [
        [h.x[0], h.y[0], z[0], 0.0],
        [h.x[1], h.y[1], z[1], 0.0],
        [h.x[2], h.y[2], z[2], 0.0],
        [-dot(o, h.x), -dot(o, h.y), -dot(o, z), 1.0],
    ];
    let loops = h
        .loops
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let flat = c.mapped_affine(into).map_err(|e| e.0)?;
            Ok((i == 0, flat2(curve(&flat)?)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let pattern = *patterns
        .get(&h.pattern.to_lowercase())
        .ok_or("a hatch pattern Curva does not have")?;
    Ok(model::Hatch {
        plane: Plane {
            origin: o,
            x: h.x,
            y: h.y,
            z,
        },
        scale: h.scale,
        rotation: h.rotation,
        pattern,
        loops,
        base: [0.0; 2],
    })
}

/// A curve in a plane's coordinates written as a 2D one, as hatch outlines are.
fn flat2(c: model::Curve) -> model::Curve {
    match c {
        model::Curve::Nurbs(mut n) => {
            n.dimension = 2;
            model::Curve::Nurbs(n)
        }
        model::Curve::Poly { segments, params } => model::Curve::Poly {
            segments: segments.into_iter().map(flat2).collect(),
            params,
        },
        other => other,
    }
}

/// A note's horizontal alignment as Rhino 5 numbered it: 0 left, 1 centre, 2 right.
pub(super) fn align_code(align: curva_document::annotation::TextAlign) -> u32 {
    match align {
        curva_document::annotation::TextAlign::Left => 0,
        curva_document::annotation::TextAlign::Centre => 1,
        curva_document::annotation::TextAlign::Right => 2,
    }
}

/// The annotation style with this id: the document's own, or the standard ones for its unit;
/// the first when there is none of that id.
pub(super) fn annotation_style(
    state: &State,
    style: u64,
) -> curva_document::annotation::AnnotationStyle {
    let styles = if state.annotation_styles.is_empty() {
        curva_document::annotation::standard_styles(matches!(state.unit, Unit::Feet | Unit::Inches))
    } else {
        state.annotation_styles.clone()
    };
    styles
        .iter()
        .find(|s| s.id == style)
        .or_else(|| styles.first())
        .cloned()
        .unwrap_or_else(|| curva_document::annotation::standard_styles(false).remove(0))
}

/// The Rhino style an annotation takes: by Curva style and its text height (in the model's
/// unit, as bits); the last part, an alignment code, is 0, notes overriding theirs.
pub(super) type StyleKey = (u64, u64, u32);
/// The Rhino styles written, by [`StyleKey`].
pub(super) type StyleIds = BTreeMap<StyleKey, Uuid>;

/// A Curva style as Rhino's, with the given text height (in the model's unit) and notes'
/// horizontal alignment: its font, sizes in the model's unit and decimals, with no id or name
/// (an override's, until the caller gives it the style's).
pub(super) fn rhino_style(state: &State, style: u64, height: f64, align: u32) -> model::DimStyle {
    let style = annotation_style(state, style);
    let scale = style.scale_to(state.unit, None);
    model::DimStyle {
        id: Uuid::NIL,
        name: String::new(),
        text_height: height,
        h_align: align,
        v_align: 3,
        arrow_size: style.arrow_size() * scale,
        extension_extension: style.extension_extension() * scale,
        extension_offset: style.extension_offset() * scale,
        text_gap: style.text_gap() * scale,
        length_resolution: i32::try_from(style.linear_resolution()).unwrap_or(2),
        font: style.font.clone(),
    }
}

/// An annotation's Rhino style: its Curva style as it is.
pub(super) fn dimension_key(state: &State, style: u64) -> StyleKey {
    let held = annotation_style(state, style);
    (held.id, held.height_in(state.unit).to_bits(), 0)
}

/// A linear dimension as Rhino holds one: its plane at the first point, its X the direction it
/// measures along (rotated) or from point to point (aligned), the other points in that plane.
pub(super) fn linear_dimension(
    d: &curva_document::annotation::Dimension,
    styles: &StyleIds,
    state: &State,
) -> Result<model::LinearDimension, String> {
    use curva_document::annotation::DimensionKind;
    let normal = cross(d.x, d.y);
    let unit = |v: P3| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (l > 1e-12).then(|| v.map(|c| c / l))
    };
    let (along, rotated) = match d.kind {
        DimensionKind::Horizontal => (d.x, true),
        DimensionKind::Vertical => (d.y, true),
        DimensionKind::Rotated(a) => (
            std::array::from_fn(|k| d.x[k] * a.cos() + d.y[k] * a.sin()),
            true,
        ),
        DimensionKind::Aligned => {
            let v: P3 = std::array::from_fn(|k| d.second[k] - d.first[k]);
            let off: f64 = (0..3).map(|k| v[k] * normal[k]).sum();
            let flat: P3 = std::array::from_fn(|k| v[k] - normal[k] * off);
            (unit(flat).ok_or("a dimension between one point")?, false)
        }
    };
    let up = cross(normal, along);
    let local = |p: P3| {
        let v: P3 = std::array::from_fn(|k| p[k] - d.first[k]);
        [
            (0..3).map(|k| v[k] * along[k]).sum::<f64>(),
            (0..3).map(|k| v[k] * up[k]).sum::<f64>(),
        ]
    };
    let style = *styles
        .get(&dimension_key(state, d.style))
        .ok_or("a dimension without its style")?;
    Ok(model::LinearDimension {
        plane: Plane {
            origin: d.first,
            x: along,
            y: up,
            z: normal,
        },
        style,
        own: None,
        rotated,
        text: d.text.clone().unwrap_or_else(|| "<>".to_owned()),
        second: local(d.second),
        line: local(d.line),
    })
}

/// A plane at `origin` with the directions `x` and `y`.
pub(super) fn plane_at(origin: P3, x: P3, y: P3) -> Plane {
    Plane {
        origin,
        x,
        y,
        z: cross(x, y),
    }
}

/// A point in a plane's coordinates.
pub(super) fn local(plane: &Plane, p: P3) -> [f64; 2] {
    let v: P3 = std::array::from_fn(|k| p[k] - plane.origin[k]);
    [
        (0..3).map(|k| v[k] * plane.x[k]).sum(),
        (0..3).map(|k| v[k] * plane.y[k]).sum(),
    ]
}
