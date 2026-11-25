//! Rhino 8's Knot and Vertex snaps (#692). Knot: where a curve's spans meet, every distinct knot
//! of the curve as Rhino 8 writes it as NURBS (its ToNURBS): a polyline's vertices, a compound
//! curve's joins and its parts' own knots, a fitted curve's knots, a circle's quarters. Vertex:
//! a mesh's vertices, and a SubD's, where its edges meet on the surface.

use curva_kernel::{Curve, Point};

use super::{Site, SnapKind, site};

/// The Knot places on `curve`, drawn through `points`. A compound curve's parts give theirs
/// apart ([`super::sites_of`] goes through them), and a point has none.
pub(super) fn knot_sites(curve: &Curve, points: &[Point]) -> Vec<Site> {
    match curve {
        Curve::Point { .. } | Curve::Compound { .. } => Vec::new(),
        // A line's and polyline's spans meet at its vertices, the points it is drawn through.
        Curve::Line { .. } | Curve::Polyline { .. } => {
            points.iter().map(|p| site(SnapKind::Knot, *p)).collect()
        }
        _ => match curve.to_nurbs() {
            Ok(Curve::Nurbs {
                degree,
                knots,
                points,
                weights,
            }) => nurbs_knots(degree, &knots, &points, &weights)
                .into_iter()
                .map(|p| site(SnapKind::Knot, p))
                .collect(),
            _ => Vec::new(),
        },
    }
}

/// The points of a NURBS curve at each of its distinct knots, from the start of its domain to
/// the end.
fn nurbs_knots(degree: usize, knots: &[f64], points: &[Point], weights: &[f64]) -> Vec<Point> {
    let n = points.len();
    if n <= degree || knots.len() != n + degree + 1 {
        return Vec::new();
    }
    let (start, end) = (knots[degree], knots[n]);
    let mut values: Vec<f64> = knots[degree..=n].to_vec();
    values.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * (end - start).abs().max(1.0));
    values
        .into_iter()
        .filter_map(|u| at(degree, knots, points, weights, u.clamp(start, end)))
        .collect()
}

/// The point of a NURBS curve at parameter `u` in its domain, by de Boor's algorithm on the
/// weighted points.
fn at(degree: usize, knots: &[f64], points: &[Point], weights: &[f64], u: f64) -> Option<Point> {
    let n = points.len();
    // The span holding `u`: the last with knots[span] <= u, kept within the domain so the
    // domain's end evaluates on the last span.
    let span = (degree..n).rev().find(|&i| knots[i] <= u).unwrap_or(degree);
    let weight = |i: usize| weights.get(i).copied().unwrap_or(1.0);
    let mut d: Vec<[f64; 4]> = (0..=degree)
        .map(|j| {
            let i = span - degree + j;
            let w = weight(i);
            let p = points[i];
            [p[0] * w, p[1] * w, p[2] * w, w]
        })
        .collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = span - degree + j;
            let span_length = knots[i + degree + 1 - r] - knots[i];
            let alpha = if span_length.abs() < 1e-300 {
                0.0
            } else {
                (u - knots[i]) / span_length
            };
            d[j] = std::array::from_fn(|k| (1.0 - alpha) * d[j - 1][k] + alpha * d[j][k]);
        }
    }
    let [x, y, z, w] = d[degree];
    (w.abs() > 1e-300).then(|| [x / w, y / w, z / w])
}

/// Vertex places: every one of `points`.
pub(super) fn vertex_sites(points: &[Point]) -> Vec<Site> {
    points.iter().map(|p| site(SnapKind::Vertex, *p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9)
    }

    fn knots_of(curve: &Curve) -> Vec<Point> {
        let points = curve.sample().unwrap();
        knot_sites(curve, &points).iter().map(|s| s.point).collect()
    }

    #[test]
    fn a_polyline_s_knots_are_its_vertices() {
        let corners = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, 0.0]];
        let polyline = Curve::polyline(corners.clone(), false).unwrap();
        assert_eq!(knots_of(&polyline), corners);
    }

    #[test]
    fn a_circle_s_knots_are_its_quarters_as_rhino_writes_it() {
        let circle = Curve::circle([1.0, 2.0, 0.0], 5.0).unwrap();
        let knots = knots_of(&circle);
        // Rhino's circle as NURBS: four spans, its seam knot at the start and again at the end.
        assert_eq!(knots.len(), 5, "{knots:?}");
        for quarter in [
            [6.0, 2.0, 0.0],
            [1.0, 7.0, 0.0],
            [-4.0, 2.0, 0.0],
            [1.0, -3.0, 0.0],
        ] {
            assert!(
                knots.iter().any(|k| close(*k, quarter)),
                "{quarter:?} in {knots:?}"
            );
        }
    }

    #[test]
    fn a_nurbs_curve_s_knots_lie_on_it_where_its_spans_meet() {
        // Degree 2 with an inner knot at 0.5: one knot inside, at the curve's half-way span join.
        let curve = Curve::nurbs(
            2,
            vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 4.0, 0.0],
                [6.0, 4.0, 0.0],
                [8.0, 0.0, 0.0],
            ],
            Vec::new(),
        )
        .unwrap();
        let knots = knots_of(&curve);
        assert_eq!(knots.len(), 3);
        assert!(close(knots[0], [0.0, 0.0, 0.0]) && close(knots[2], [8.0, 0.0, 0.0]));
        // At u = 0.5 the curve is half way between control points 1 and 2: (4, 4, 0).
        assert!(close(knots[1], [4.0, 4.0, 0.0]), "{:?}", knots[1]);
    }
}
