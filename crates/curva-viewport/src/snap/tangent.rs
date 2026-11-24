//! The Tan snap, as Rhino's: where a line from the last point just touches a curve. Curves are
//! searched as the segments they are drawn with; where the line from the last point turns from
//! one side of the curve to the other at a point between two segments, the circle through that
//! point and its neighbours gives the exact place, which for a circle or an arc is the true
//! tangent point.

use super::*;

/// The sharpest turn between two segments, in radians, that still counts as a smooth curve: a
/// polyline's corner has no tangent.
const SMOOTH_TURN: f64 = 0.35;

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale(a: Point, by: f64) -> Point {
    [a[0] * by, a[1] * by, a[2] * by]
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// The circle through three points: its centre, radius and the unit normal of its plane.
/// `None` for points in a line.
fn circle_through(p0: Point, p1: Point, p2: Point) -> Option<(Point, f64, Point)> {
    let (a, b) = (sub(p1, p0), sub(p2, p0));
    let normal = cross(a, b);
    let square = dot(normal, normal);
    if square < 1e-18 {
        return None;
    }
    let off = cross(sub(scale(b, dot(a, a)), scale(a, dot(b, b))), normal);
    let centre = add(p0, scale(off, 0.5 / square));
    let radius = length(sub(p1, centre));
    Some((centre, radius, scale(normal, 1.0 / square.sqrt())))
}

/// The points where lines from `from` touch the circle at `centre` of `radius` in the plane
/// of `normal`, `from` taken straight down onto that plane. None when `from` is inside it.
fn touching(centre: Point, radius: f64, normal: Point, from: Point) -> Vec<Point> {
    let off = sub(from, centre);
    let flat = sub(off, scale(normal, dot(off, normal)));
    let distance = length(flat);
    if distance <= radius + 1e-9 {
        return Vec::new();
    }
    let out = scale(flat, 1.0 / distance);
    let side = cross(normal, out);
    let cos = radius / distance;
    let sin = (1.0 - cos * cos).max(0.0).sqrt();
    [sin, -sin]
        .into_iter()
        .map(|s| {
            add(
                centre,
                add(scale(out, radius * cos), scale(side, radius * s)),
            )
        })
        .collect()
}

/// Where lines from `from` touch the curves the segments are part of, in the segments' order.
pub(super) fn tangent_points(segments: &[NearbySegment], from: Point) -> Vec<Point> {
    let mut points = Vec::new();
    for pair in segments.windows(2) {
        let (first, second) = (&pair[0], &pair[1]);
        if first.object != second.object || length(sub(first.b, second.a)) > 1e-9 {
            continue;
        }
        let (into, out) = (sub(first.b, first.a), sub(second.b, second.a));
        let turn = dot(into, out) / (length(into) * length(out)).max(1e-18);
        if turn < SMOOTH_TURN.cos() {
            continue;
        }
        let corner = first.b;
        let towards = sub(corner, from);
        // The line from the last point passes from one side of the curve to the other here.
        if length(towards) < 1e-9 || dot(cross(towards, into), cross(towards, out)) > 0.0 {
            continue;
        }
        let reach = length(into).max(length(out)) * 2.0;
        let exact = circle_through(first.a, corner, second.b)
            .map(|(centre, radius, normal)| touching(centre, radius, normal, from))
            .unwrap_or_default()
            .into_iter()
            .min_by(|a, b| length(sub(*a, corner)).total_cmp(&length(sub(*b, corner))))
            .filter(|point| length(sub(*point, corner)) <= reach);
        points.push(exact.unwrap_or(corner));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_from_outside_touch_a_circle_at_its_tangent_points() {
        let points = touching([0.0; 3], 10.0, [0.0, 0.0, 1.0], [20.0, 0.0, 5.0]);
        let half = 75.0_f64.sqrt();
        assert_eq!(points.len(), 2);
        for (point, y) in points.iter().zip([half, -half]) {
            assert!(length(sub(*point, [5.0, y, 0.0])) < 1e-9, "{point:?}");
        }
        assert!(touching([0.0; 3], 10.0, [0.0, 0.0, 1.0], [3.0, 0.0, 0.0]).is_empty());
    }

    #[test]
    fn three_points_of_a_circle_give_that_circle() {
        let on = |angle: f64| [4.0 + 7.0 * angle.cos(), -2.0 + 7.0 * angle.sin(), 1.0];
        let (centre, radius, normal) = circle_through(on(0.1), on(0.2), on(0.3)).unwrap();
        assert!(length(sub(centre, [4.0, -2.0, 1.0])) < 1e-6, "{centre:?}");
        assert!((radius - 7.0).abs() < 1e-6);
        assert!((normal[2].abs() - 1.0).abs() < 1e-9);
        assert!(circle_through([0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).is_none());
    }
}
