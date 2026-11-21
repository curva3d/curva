//! Rhino's views, construction planes, details and clipping planes as Curva's.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A file's view as a Curva saved view, its sizes multiplied by `scale`.
fn saved_view(v: &model::View, scale: f64) -> Option<SavedView> {
    let forward = unit3(v.direction)?;
    // Rhino keeps the up it was given, such as Z for a view looking down at an angle: squared
    // to the line of sight here.
    let up = unit3(sub(v.up, forward.map(|c| c * dot3(v.up, forward))))?;
    let eye = v.location.map(|c| c * scale);
    let target = v.target.map(|c| c * scale);
    // Curva's target lies on the line of sight.
    let along = dot3(sub(target, eye), forward);
    let along = if along > 1e-12 {
        along
    } else {
        0.5 * (v.frustum[4] + v.frustum[5]) * scale
    };
    let target: P3 = std::array::from_fn(|k| eye[k] + along * forward[k]);
    let distance = if v.parallel {
        // A flat view's zoom is its frustum's height.
        (v.frustum[3] - v.frustum[2]).abs() * 0.5 * scale / HALF_FIELD.tan()
    } else {
        along
    };
    if !(distance.is_finite() && distance > 0.0) {
        return None;
    }
    let pitch = (-forward[2]).clamp(-1.0, 1.0).asin();
    let yaw = (-forward[1]).atan2(-forward[0]);
    let looks = |axis: usize, sign: f64| v.parallel && forward[axis] * sign > 1.0 - 1e-9;
    let kind = if looks(2, -1.0) {
        "Top"
    } else if looks(1, 1.0) {
        "Front"
    } else if looks(0, -1.0) {
        "Right"
    } else {
        "Perspective"
    };
    // Yaw and pitch hold views with Z up; flat views along an axis keep their own directions, as
    // Curva's standard views do, and so does any view that is not upright.
    let right = unit3(cross3(forward, [0.0, 0.0, 1.0]));
    let z_up = kind == "Perspective"
        && right.is_some_and(|r| dot3(cross3(r, forward), up) > 1.0 - 1e-6)
        && pitch.abs() < 1.55;
    let axes = if z_up {
        None
    } else {
        let right = unit3(cross3(forward, up))?;
        Some([forward, right, cross3(right, forward)])
    };
    Some(SavedView {
        name: v.name.clone(),
        kind: kind.to_owned(),
        target,
        yaw,
        pitch,
        distance,
        axes,
        parallel: v.parallel,
        lens: None,
    })
}

/// A file's named views and construction planes for a Curva model of `unit`; views Curva cannot
/// use (no name, a broken camera) are counted in the report.
fn views_and_planes(
    model: &Model,
    unit: Unit,
    report: &mut Report,
) -> (Vec<SavedView>, Vec<SavedCPlane>) {
    let scale = millimetres_per(model.unit).unwrap_or(1.0) / unit.millimetres();
    let mut views = Vec::new();
    for v in &model.views {
        match saved_view(v, scale).filter(|s| !s.name.trim().is_empty()) {
            Some(saved) => views.push(saved),
            None => Report::count(&mut report.skipped, "unusable named views"),
        }
    }
    let planes = model
        .cplanes
        .iter()
        .filter_map(|c| {
            let x = unit3(c.plane.x)?;
            let y = unit3(sub(c.plane.y, x.map(|v| v * dot3(c.plane.y, x))))?;
            Some(SavedCPlane {
                name: c.name.clone(),
                origin: c.plane.origin.map(|v| v * scale),
                x,
                y,
            })
        })
        .collect();
    (views, planes)
}

/// Adds a file's named views and planes to a document, keeping any it already has by that name.
pub(super) fn add_views_and_planes(model: &Model, doc: &mut Document, report: &mut Report) {
    let (views, planes) = views_and_planes(model, doc.state().unit, report);
    for v in views {
        if doc.saved_view(&v.name).is_none() {
            Report::count(&mut report.brought, "named views");
            doc.save_view(v);
        }
    }
    for p in planes {
        if doc.saved_cplane(&p.name).is_none() {
            Report::count(&mut report.brought, "named construction planes");
            doc.save_cplane(p);
        }
    }
}

/// A detail from a file: its outline as a curve on the sheet (in millimetres), and what it shows:
/// its standard view by the way it looks (else a perspective), the point it looks at, and its
/// scale (Rhino keeps page units per model unit; Curva, millimetres per millimetre).
pub(super) fn detail_in(
    d: &model::Detail,
    page_mm: f64,
    file_mm: f64,
    model_scale: f64,
) -> Result<(Curve, curva_document::layouts::Detail), String> {
    let outline = curve(&model::Curve::Nurbs(d.boundary.clone())).map_err(|e| e.0)?;
    let Shape::Curve(frame) = scaled(Shape::Curve(outline), page_mm)? else {
        return Err("a detail without an outline".to_owned());
    };
    let v = &d.view;
    // Any of the six flat views, within a degree (#478: Back and Left elevations, and ones a
    // hair off square, came in as perspectives).
    let projection = if v.parallel {
        curva_document::layouts::flat_view_looking(v.direction).unwrap_or("Perspective")
    } else {
        "Perspective"
    };
    let target: P3 = v.target.map(|c| c * model_scale);
    let gap: f64 = (0..3)
        .map(|k| (v.target[k] - v.location[k]).powi(2))
        .sum::<f64>()
        .sqrt()
        * model_scale;
    Ok((
        frame,
        curva_document::layouts::Detail {
            frame: 0,
            projection: projection.to_owned(),
            target,
            distance: gap,
            // Rhino keeps page units per model unit (#478: it was taken the other way up, so a
            // 1:100 detail came in at 100:1).
            scale: (v.parallel && d.ratio > 0.0).then(|| d.ratio * page_mm / file_mm),
            display: model::display_mode_name(d.display).map_or_else(
                || {
                    if projection == "Perspective" {
                        "Shaded".to_owned()
                    } else {
                        "Wireframe".to_owned()
                    }
                },
                str::to_owned,
            ),
            locked: d.locked,
            hidden_layers: Vec::new(),
        },
    ))
}

/// A clipping plane's rectangle as a closed polyline whose corners run counter-clockwise about
/// its plane's normal, so that the side it keeps is the rectangle's own normal side, as Curva's
/// clipping planes take it unflipped.
pub(super) fn clipping_rectangle(c: &model::ClippingPlane) -> Result<Curve, KernelError> {
    let p = &c.plane;
    let at =
        |u: f64, v: f64| -> P3 { std::array::from_fn(|k| p.origin[k] + p.x[k] * u + p.y[k] * v) };
    let [[x0, x1], [y0, y1]] = c.extents;
    Curve::polyline(vec![at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)], true)
}
