//! Curva's layouts' sheets as Rhino's pages: details, plain parts and text fields.

#[allow(clippy::wildcard_imports)]
use super::*;

/// An object on a layout, as Rhino keeps it: in millimetres on its page; a detail's frame as the
/// detail itself, with its camera and scale.
pub(super) fn sheet_geometry(
    o: &Object,
    layout: &curva_document::layouts::Layout,
    blocks: &BTreeMap<u64, Uuid>,
    patterns: &BTreeMap<String, i32>,
    styles: &StyleIds,
    state: &State,
    details: &BTreeMap<u64, Uuid>,
) -> Result<Geometry, String> {
    let mm = layout.unit.millimetres();
    let to_mm = [
        [mm, 0.0, 0.0, 0.0],
        [0.0, mm, 0.0, 0.0],
        [0.0, 0.0, mm, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // A millimetre sheet is drawn as it is, so its lines stay lines.
    let shape = if (mm - 1.0).abs() < 1e-12 {
        o.shape.clone()
    } else {
        curva_document::blocks::shape_placed(&o.shape, &to_mm).ok_or("scaling failed")?
    };
    let Some(detail) = layout.details.iter().find(|d| d.frame == o.id) else {
        return geometry_of(&shape, blocks, patterns, styles, state);
    };
    let Shape::Curve(frame) = &shape else {
        return Err("a detail without its frame".to_owned());
    };
    let boundary = outline(frame)?;
    let (low, high) = shape.bounds().ok_or("a detail without a size")?;
    let model_mm = state.unit.millimetres();
    Ok(Geometry::Detail(model::Detail {
        id: details.get(&detail.frame).copied().unwrap_or(Uuid::NIL),
        view: detail_camera(detail, [high[0] - low[0], high[1] - low[1]], model_mm),
        boundary,
        ratio: detail.scale.map_or(0.0, |s| 1.0 / s),
        display: model::display_mode_id(&detail.display),
        locked: detail.locked,
    }))
}

/// The camera a detail looks through, in model units: its standard view's direction at its
/// target, the frustum the frame's size covers at its scale (or a 50 mm lens for a perspective).
fn detail_camera(
    detail: &curva_document::layouts::Detail,
    frame_mm: [f64; 2],
    model_mm: f64,
) -> model::View {
    let (direction, up): (P3, P3) = match detail.projection.as_str() {
        "Top" => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        "Front" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        "Right" => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        _ => {
            let s = 1.0 / 3f64.sqrt();
            ([-s, s, -s], [0.0, 0.0, 1.0])
        }
    };
    let up = {
        let along: f64 = (0..3).map(|k| up[k] * direction[k]).sum();
        let v: P3 = std::array::from_fn(|k| up[k] - direction[k] * along);
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
        v.map(|c| c / l)
    };
    let target = detail.target;
    match detail.scale {
        Some(scale) => {
            let [w, h] = frame_mm.map(|v| v / scale / model_mm);
            let depth = 1000.0 * w.max(h).max(1.0);
            model::View {
                name: String::new(),
                location: std::array::from_fn(|k| target[k] - direction[k] * depth),
                direction,
                up,
                target,
                parallel: true,
                frustum: [
                    -w / 2.0,
                    w / 2.0,
                    -h / 2.0,
                    h / 2.0,
                    depth * 1e-3,
                    depth * 2.0,
                ],
            }
        }
        None => {
            let distance = detail.distance.max(1e-6);
            let near = distance / 100.0;
            // A 50 mm lens on 36 mm film across the frame's longer side.
            let half = near * 18.0 / 50.0;
            let aspect = frame_mm[1] / frame_mm[0].max(1e-12);
            model::View {
                name: String::new(),
                location: std::array::from_fn(|k| target[k] - direction[k] * distance),
                direction,
                up,
                target,
                parallel: false,
                frustum: [
                    -half,
                    half,
                    -half * aspect,
                    half * aspect,
                    near,
                    distance * 100.0,
                ],
            }
        }
    }
}

/// A detail's outline as Rhino keeps it: a 2D NURBS curve, of degree one for a polyline
/// (openNURBS' knot vector, without the end knots NURBS books add).
fn outline(frame: &Curve) -> Result<model::NurbsCurve, String> {
    if let Curve::Polyline { points, closed } = frame {
        let mut corners: Vec<[f64; 4]> = points.iter().map(|p| [p[0], p[1], 0.0, 1.0]).collect();
        if *closed && let Some(first) = corners.first().copied() {
            corners.push(first);
        }
        #[allow(clippy::cast_precision_loss)]
        let knots = (0..corners.len()).map(|i| i as f64).collect();
        return Ok(model::NurbsCurve {
            dimension: 2,
            order: 2,
            knots,
            points: corners,
            rational: false,
        });
    }
    let nurbs = frame.to_nurbs().map_err(|e| e.0)?;
    let model::Curve::Nurbs(mut boundary) = curve(&nurbs)? else {
        return Err("a detail frame that is not a NURBS curve".to_owned());
    };
    boundary.dimension = 2;
    Ok(boundary)
}

/// A scale bar, table or arc length dimension as the plain objects it draws, for a file that has
/// none (Rhino has no arc length dimension): its lines, its filled blocks and arrowheads as solid
/// hatches, and its words as notes, on its layer and sheet and in a group of their own. `None`
/// for any other object.
pub(super) fn plain_parts(
    o: &Object,
    state: &State,
    group: u64,
    before: usize,
) -> Option<Vec<Object>> {
    use curva_document::annotation::{Annotation, Hatch};
    use curva_document::sheet::SheetParts;
    let (parts, origin, x, y) = match &o.shape {
        Shape::Annotation(Annotation::ScaleBar(bar)) => (
            curva_document::sheet::scale_bar_parts(bar, state, o.space),
            bar.at.origin,
            bar.at.x,
            bar.at.y,
        ),
        Shape::Annotation(Annotation::Table(table)) => (
            curva_document::sheet::table_parts(table, state),
            table.at.origin,
            table.at.x,
            table.at.y,
        ),
        Shape::Annotation(a @ Annotation::ArcLength(d)) => {
            let drawn = curva_document::shape::annotation_parts(a, state, o.space)?;
            let parts = SheetParts {
                lines: drawn
                    .lines
                    .iter()
                    .filter_map(|(s, e)| Curve::line(*s, *e).ok())
                    .collect(),
                fills: drawn
                    .arrows
                    .iter()
                    .filter_map(|head| Curve::polyline(head.to_vec(), true).ok())
                    .map(|outline| vec![outline])
                    .collect(),
                texts: vec![drawn.text],
            };
            (parts, d.centre, d.x, d.y)
        }
        _ => return None,
    };
    let shapes = parts
        .lines
        .into_iter()
        .map(Shape::Curve)
        .chain(parts.fills.into_iter().map(|loops| {
            Shape::Annotation(Annotation::Hatch(Hatch {
                loops,
                pattern: "Solid".to_owned(),
                scale: 1.0,
                rotation: 0.0,
                base: origin,
                x,
                y,
            }))
        }))
        .chain(
            parts
                .texts
                .into_iter()
                .map(|t| Shape::Annotation(Annotation::Text(t))),
        );
    Some(
        shapes
            .enumerate()
            .map(|(i, shape)| Object {
                // Ids of their own, far from the model's, as these objects exist only in the file.
                id: u64::MAX - (before + i) as u64,
                shape,
                group: Some(group),
                ..o.clone()
            })
            .collect(),
    )
}

/// The fields Rhino works out itself, by the same names (Rhino 8's `information/text_fields.htm`).
/// Others read what only Curva holds (objects by Curva's numbers), so they go out worked out.
/// Document text, layout user text and notes travel in the file, so Rhino reads them itself.
const RHINO_FIELDS: [&str; 12] = [
    "DocumentText",
    "LayoutUserText",
    "Notes",
    "PageName",
    "PageNumber",
    "NumPages",
    "PageWidth",
    "PageHeight",
    "PaperName",
    "Date",
    "FileName",
    "ModelUnits",
];

/// Fields that measure an object, named by its id: Curva's object number, Rhino's object id.
pub(crate) const OBJECT_FIELDS: [&str; 3] = ["CurveLength", "Area", "Volume"];

/// A field with its first quoted part swapped by `swap`, the rest (further parts, arithmetic)
/// kept; `None` when it has no quoted part or `swap` gives nothing.
pub(crate) fn with_first_part(
    field: &str,
    swap: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let open = field.find("(\"")? + 2;
    let close = open + field[open..].find('"')?;
    let swapped = swap(&field[open..close])?;
    Some(format!("{}{swapped}{}", &field[..open], &field[close..]))
}

/// Words with their fields as Rhino will show them: its own fields kept, fields measuring an
/// object written to name it by its id going out, the rest worked out.
fn rhino_words(
    words: &str,
    state: &State,
    space: Option<u64>,
    fields: &curva_document::fields::FieldContext,
    page_ids: &BTreeMap<u64, Uuid>,
    object_ids: &BTreeMap<u64, Uuid>,
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
        if name == "LayoutUserText" {
            match layout_by_page_id(field, page_ids) {
                Some(rhinos) => out.push_str(&rhinos),
                None => out.push_str(&curva_document::fields::resolve(
                    field, state, space, fields,
                )),
            }
        } else if RHINO_FIELDS.contains(&name.as_str()) {
            out.push_str(field);
        } else if let Some(rhinos) = OBJECT_FIELDS
            .contains(&name.as_str())
            .then(|| {
                with_first_part(field, |n| {
                    object_ids.get(&n.trim().parse().ok()?).map(Uuid::text)
                })
            })
            .flatten()
        {
            out.push_str(&rhinos);
        } else {
            out.push_str(&curva_document::fields::resolve(
                field, state, space, fields,
            ));
        }
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// The two parts of a field's brackets, `"a","b"`, as written; `None` for any other form.
pub(crate) fn field_args(field: &str) -> Option<Vec<String>> {
    let inner = field.get(field.find('(')? + 1..field.rfind(')')?)?;
    let mut args = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        args.push(part.strip_prefix('"')?.strip_suffix('"')?.to_owned());
    }
    Some(args)
}

/// A `LayoutUserText` field as Rhino reads it: with one part as it is, with two the layout's
/// number swapped for its page's id. `None` when it names no layout going out.
fn layout_by_page_id(field: &str, page_ids: &BTreeMap<u64, Uuid>) -> Option<String> {
    match field_args(field)?.as_slice() {
        [_] => Some(field.to_owned()),
        [layout, key] => {
            let id = page_ids.get(&layout.trim().parse().ok()?)?;
            Some(format!("%<LayoutUserText(\"{}\",\"{key}\")>%", id.text()))
        }
        _ => None,
    }
}

/// An object with its notes' and leaders' fields as Rhino will show them.
pub(super) fn with_rhino_fields(
    o: &Object,
    state: &State,
    fields: &curva_document::fields::FieldContext,
    page_ids: &BTreeMap<u64, Uuid>,
    object_ids: &BTreeMap<u64, Uuid>,
) -> Object {
    use curva_document::annotation::Annotation;
    let mut out = o.clone();
    let words = |w: &str| rhino_words(w, state, o.space, fields, page_ids, object_ids);
    match &mut out.shape {
        Shape::Annotation(Annotation::Text(t)) => t.content = words(&t.content),
        Shape::Annotation(Annotation::Leader(l)) => l.content = words(&l.content),
        _ => {}
    }
    out
}
