//! The drawing's flat geometry: curves taken apart, flattened onto the view, measured, crossed
//! and matched, as Make2D splits and cleans its lines.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A curve taken apart into the pieces Make2D draws one by one: a polyline's segments, a joined
/// curve's parts.
pub(super) fn pieces_of(curve: &Curve) -> Vec<Curve> {
    match curve {
        Curve::Polyline { points, closed } => {
            let mut corners = points.clone();
            if *closed && let Some(first) = points.first() {
                corners.push(*first);
            }
            corners
                .windows(2)
                .filter_map(|w| Curve::line(w[0], w[1]).ok())
                .collect()
        }
        Curve::Compound { segments } => segments.iter().flat_map(pieces_of).collect(),
        Curve::Point { .. } => Vec::new(),
        other => vec![other.clone()],
    }
}

/// The ends of a line.
pub(super) fn ends(curve: &Curve) -> Option<(Point, Point)> {
    match curve {
        Curve::Line { start, end } => Some((*start, *end)),
        _ => None,
    }
}

/// The projection of a model curve: an exact line or mapped curve, or in perspective a
/// polyline through its points.
pub(super) fn flatten(curve: &Curve, view: &View, focus: f64) -> Option<Curve> {
    if let Some((a, b)) = ends(curve) {
        let (a, b) = (view.project(a, focus)?, view.project(b, focus)?);
        return Curve::line(a, b).ok();
    }
    if view.parallel {
        return curve.mapped_affine(view.flattening()).ok();
    }
    let points: Option<Vec<Point>> = curve
        .sample()
        .ok()?
        .into_iter()
        .map(|p| view.project(p, focus))
        .collect();
    Curve::polyline(points?, false).ok()
}

/// The size of a curve's projection, to drop lines seen end-on.
pub(super) fn extent(curve: &Curve) -> f64 {
    let Ok(points) = curve.sample() else {
        return 0.0;
    };
    let Some(&first) = points.first() else {
        return 0.0;
    };
    points
        .iter()
        .map(|p| length(sub(*p, first)))
        .fold(0.0, f64::max)
}

pub(super) fn bounds2(curve: &Curve) -> ([f64; 2], [f64; 2]) {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for p in curve.sample().unwrap_or_default() {
        for i in 0..2 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    (low, high)
}

/// Where a flat line `a` crosses a flat line `b`, strictly inside `a` and anywhere on `b`
/// including its ends (a line passing behind another's end changes visibility there).
pub(super) fn line_crossing(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let r = sub(a.1, a.0);
    let s = sub(b.1, b.0);
    let denom = r[0] * s[1] - r[1] * s[0];
    let scale = length(r) * length(s);
    if denom.abs() <= 1e-12 * scale {
        return None;
    }
    let w = sub(b.0, a.0);
    let t = (w[0] * s[1] - w[1] * s[0]) / denom;
    let u = (w[0] * r[1] - w[1] * r[0]) / denom;
    let inside = |v: f64| v > 1e-9 && v < 1.0 - 1e-9;
    let touching = |v: f64| (-1e-9..=1.0 + 1e-9).contains(&v);
    (inside(t) && touching(u)).then(|| plus(a.0, r, t))
}

/// The model point a point of a flat piece stands for: the point of `model` whose projection is
/// nearest `flat_point`.
pub(super) fn model_point(
    model: &Curve,
    flat_point: Point,
    view: &View,
    focus: f64,
) -> Option<Point> {
    let samples: Vec<Point> = if let Some((a, b)) = ends(model) {
        (0..=64)
            .map(|k| plus(a, sub(b, a), f64::from(k) / 64.0))
            .collect()
    } else {
        model.sample().ok()?
    };
    let mut best: Option<(f64, Point)> = None;
    for pair in samples.windows(2) {
        let (Some(pa), Some(pb)) = (view.project(pair[0], focus), view.project(pair[1], focus))
        else {
            continue;
        };
        let along = sub(pb, pa);
        let span = dot(along, along);
        let t = if span > 0.0 {
            (dot(sub(flat_point, pa), along) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let off = length(sub(plus(pa, along, t), flat_point));
        if best.is_none_or(|(b, _)| off < b) {
            best = Some((off, plus(pair[0], sub(pair[1], pair[0]), t)));
        }
    }
    best.map(|(_, p)| p)
}

/// The middle of a curve.
pub(super) fn middle(curve: &Curve) -> Option<Point> {
    if let Some((a, b)) = ends(curve) {
        return Some(plus(a, sub(b, a), 0.5));
    }
    let points = curve.sample().ok()?;
    points.get(points.len() / 2).copied()
}

/// Whether five points along `piece` all lie within `tolerance` of `other`.
pub(super) fn lies_on(piece: &Curve, other: &Curve, tolerance: f64) -> bool {
    let picks: Vec<Point> = if let Some((a, b)) = ends(piece) {
        (0..=4)
            .map(|k| plus(a, sub(b, a), f64::from(k) / 4.0))
            .collect()
    } else {
        let Ok(points) = piece.sample() else {
            return false;
        };
        if points.len() < 5 {
            points
        } else {
            (0..5).map(|k| points[k * (points.len() - 1) / 4]).collect()
        }
    };
    picks.iter().all(|p| {
        other
            .closest_point(*p)
            .is_ok_and(|q| length(sub(q, *p)) <= tolerance)
    })
}
