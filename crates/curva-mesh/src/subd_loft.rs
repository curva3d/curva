//! Rhino 8's `SubDLoft` for curves. Measured (23 November 2026, `-SubDLoft` with RhinoCommon-made
//! curves picked by id): it asks "Select curves and boundary edge chains in loft order", then
//! "Adjust edge loop position. Press Enter when done ( Flip  Automatic  Natural  SnapToKnots=Yes )"
//! and "Lofting options ( ClosedLoft=No  AdjustSegmentCounts=No  DivisionsAlong=1 )". Each curve
//! becomes one ring of cage points, rings next to each other are joined by quads, and the first
//! and last rings are creased borders:
//!
//! - an open smooth curve gives the points of Rhino's SubD friendly curve
//!   (`NurbsCurve.CreateSubDFriendly`) less the one a clamped cubic adds next to each end: its own
//!   control points, with the second and second-last moved towards the ends to a share f of the
//!   way. Measured on 26 November 2026, f depends only on the knots, not on the shape, size or
//!   parameter scale. On evenly spaced knots it is 15/16 (a cubic of 5 or more points, any curve
//!   of degree 4 or 5), 121/128 (a quadratic of 5 or more points) or 1 (a 4-point cubic, a
//!   quadratic of 3 or 4 points). On uneven knots Rhino gives other halving fractions (63/64,
//!   123/128, …) by a rule not worked out, so those curves, and rational ones, are refused;
//! - a polyline keeps its corners: each segment in thirds, less the second and second-last point
//!   (four points (0,0), (2,0), (3,2), (5,2) give 8);
//! - a closed curve (a circle, a periodic cubic) gives its own control points (a circle of radius
//!   2: 8 points on a square's corners and sides);
//! - `DivisionsAlong` n puts n − 1 rings evenly on the straight lines between each two rings
//!   (three circles and 2: 40 points, 32 faces).
//!
//! Rings must have the same number of points (AdjustSegmentCounts, which Rhino offers to change
//! that, is not here), and their seams are taken as they are (Adjust edge loop position is not
//! here).

use crate::{Mesh, Point, SmoothError, SubD};
use curva_kernel::{Curve, Spline};

fn lerp(a: Point, b: Point, t: f64) -> Point {
    std::array::from_fn(|k| a[k] + t * (b[k] - a[k]))
}

fn same(a: Point, b: Point) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= 1e-12 * (1.0 + a[k].abs()))
}

/// Why an open smooth curve is refused: one with uneven knots, kinks or weights.
pub const OPEN_SMOOTH: &str = "open smooth curves are lofted only with evenly spaced knots";

fn failed(why: &str) -> SmoothError {
    SmoothError(why.to_owned())
}

/// A polyline's ring: each segment in thirds; an open one less its second and second-last point.
fn thirds(points: &[Point], closed: bool) -> Vec<Point> {
    let n = points.len();
    let segments = if closed { n } else { n.saturating_sub(1) };
    let mut out = Vec::new();
    for i in 0..segments {
        let (p, q) = (points[i], points[(i + 1) % n]);
        out.extend([p, lerp(p, q, 1.0 / 3.0), lerp(p, q, 2.0 / 3.0)]);
    }
    if !closed {
        out.extend(points.last().copied());
        out.remove(1);
        out.remove(out.len() - 2);
    }
    out
}

/// The share of the way Rhino's SubD friendly fit leaves an open curve's second point from its
/// first (and its second-last from its last), when that is known: a curve with no weights and
/// evenly spaced simple inner knots (see the module notes).
fn friendly_share(spline: &Spline) -> Option<f64> {
    if spline.weights().iter().any(|w| (w - 1.0).abs() > 1e-12) {
        return None;
    }
    let (start, end) = spline.domain();
    let mut stops: Vec<f64> = spline.knots.clone();
    stops.dedup_by(|a, b| (*a - *b).abs() <= (end - start).abs() * 1e-12);
    let spans: Vec<f64> = stops.windows(2).map(|w| w[1] - w[0]).collect();
    let simple = stops.len() + 2 * spline.degree == spline.knots.len();
    let even = spans
        .iter()
        .all(|s| (s - spans[0]).abs() <= spans[0].abs() * 1e-9);
    if !(simple && even) {
        return None;
    }
    let n = spline.points.len();
    Some(match spline.degree {
        2 if n >= 5 => 121.0 / 128.0,
        2 => 1.0,
        3 if n >= 5 => 15.0 / 16.0,
        3 => 1.0,
        _ => 15.0 / 16.0,
    })
}

/// A clamped spline's control points, the repeated last one left out.
fn closed_points(spline: &Spline) -> Vec<Point> {
    let mut points = spline.control_points();
    points.pop();
    points
}

/// The ring of SubD cage points Rhino's SubDLoft makes from a curve, and whether it is closed
/// (see the module notes).
///
/// # Errors
/// When the curve is a point or has no usable NURBS form.
pub fn loft_ring(curve: &Curve) -> Result<(Vec<Point>, bool), SmoothError> {
    let spline = |c: &Curve| Spline::of(c).map_err(|e| failed(&e.to_string()));
    match curve {
        Curve::Point { .. } => Err(failed("a point is not a curve to loft")),
        Curve::Line { start, end } => Ok((thirds(&[*start, *end], false), false)),
        Curve::Polyline { points, closed } => Ok((thirds(points, *closed), *closed)),
        Curve::Nurbs { degree, points, .. }
            if points.len() > 2 * degree
                && (0..*degree).all(|i| same(points[i], points[points.len() - degree + i])) =>
        {
            // Periodic: its own points, once round.
            Ok((points[..points.len() - degree].to_vec(), true))
        }
        other => {
            let s = spline(other)?;
            let points = s.control_points();
            if points.len() > 2 && same(points[0], points[points.len() - 1]) {
                return Ok((closed_points(&s), true));
            }
            if s.degree == 1 {
                return Ok((thirds(&points, false), false));
            }
            // Rhino's friendly fit: on uneven knots it moves the points next to the ends by amounts
            // not worked out, so those are not offered rather than offered differently.
            let share = friendly_share(&s).ok_or_else(|| failed(OPEN_SMOOTH))?;
            let mut ring = points;
            let n = ring.len();
            ring[1] = lerp(ring[0], ring[1], share);
            ring[n - 2] = lerp(ring[n - 1], ring[n - 2], share);
            Ok((ring, false))
        }
    }
}

impl SubD {
    /// Rings of cage points lofted, as Rhino's SubDLoft: `divisions` − 1 more rings evenly
    /// between each two, quads joining rings next to each other (the last back to the first
    /// when `closed_loft`), open borders creased. Rings are closed loops when `closed_rings`.
    ///
    /// # Errors
    /// When there are fewer than two rings, rings of different sizes, or rings too short.
    pub fn lofted(
        rings: &[Vec<Point>],
        closed_rings: bool,
        closed_loft: bool,
        divisions: usize,
    ) -> Result<Self, SmoothError> {
        let m = rings.first().map_or(0, Vec::len);
        if rings.len() < 2 || m < 2 || rings.iter().any(|r| r.len() != m) {
            return Err(failed("loft curves need the same number of points"));
        }
        let divisions = divisions.max(1);
        let count = rings.len();
        let joins = if closed_loft { count } else { count - 1 };
        let mut all: Vec<Vec<Point>> = Vec::new();
        for i in 0..count {
            all.push(rings[i].clone());
            if i < joins {
                let next = &rings[(i + 1) % count];
                for d in 1..divisions {
                    #[allow(clippy::cast_precision_loss)]
                    let t = d as f64 / divisions as f64;
                    all.push((0..m).map(|k| lerp(rings[i][k], next[k], t)).collect());
                }
            }
        }
        let rows = all.len();
        let at = |r: usize, k: usize| u32::try_from((r % rows) * m + k % m).unwrap_or(u32::MAX);
        let row_joins = if closed_loft { rows } else { rows - 1 };
        let across = if closed_rings { m } else { m - 1 };
        let faces: Vec<[u32; 4]> = (0..row_joins)
            .flat_map(|r| {
                (0..across).map(move |k| [at(r, k), at(r, k + 1), at(r + 1, k + 1), at(r + 1, k)])
            })
            .collect();
        let cage = Mesh::new(all.concat(), &faces).map_err(|e| failed(&e.to_string()))?;
        Ok(Self::from_cage(cage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(p: Point) -> Point {
        p.map(|c| (c * 1e4).round() / 1e4 + 0.0)
    }

    fn circle(z: f64, radius: f64) -> Curve {
        Curve::Circle {
            centre: [0.0, 0.0, z],
            radius,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
        }
    }

    fn cubic(points: Vec<Point>, degree: usize) -> Curve {
        let n = points.len();
        let inner = n - degree - 1;
        #[allow(clippy::cast_precision_loss)]
        let knots = std::iter::repeat_n(0.0, degree + 1)
            .chain((1..=inner).map(|k| k as f64))
            .chain(std::iter::repeat_n((inner + 1) as f64, degree + 1))
            .collect();
        Curve::Nurbs {
            degree,
            knots,
            points,
            weights: Vec::new(),
        }
    }

    fn wavy(n: i32) -> Vec<Point> {
        (0..n)
            .map(|x| [f64::from(x), 0.0, f64::from(x % 2)])
            .collect()
    }

    #[test]
    fn open_smooth_curves_are_refused_for_now() {
        // Unevenly spaced knots: Rhino 8 leaves the first end as it is and moves the last by
        // 1/16, by a rule not worked out.
        let uneven = Curve::Nurbs {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0, 1.0],
            points: wavy(5),
            weights: Vec::new(),
        };
        assert!(loft_ring(&uneven).is_err());
    }

    #[test]
    fn an_evenly_spaced_open_curve_gives_rhinos_friendly_points() {
        // Rhino 8's NurbsCurve.CreateSubDFriendly, less the point next to each end: the curve's
        // own points, the second and second-last 15/16 of the way out from the ends (cubic).
        let (ring, closed) = loft_ring(&cubic(wavy(5), 3)).unwrap();
        assert!(!closed);
        let ring: Vec<Point> = ring.into_iter().map(round).collect();
        assert_eq!(
            ring,
            vec![
                [0.0, 0.0, 0.0],
                [0.9375, 0.0, 0.9375],
                [2.0, 0.0, 0.0],
                [3.0625, 0.0, 0.9375],
                [4.0, 0.0, 0.0]
            ]
        );
        // A quadratic moves them to 121/128; a 4-point cubic keeps its points.
        let (ring, _) = loft_ring(&cubic(wavy(6), 2)).unwrap();
        assert_eq!(round(ring[1]), [0.9453, 0.0, 0.9453]);
        assert_eq!(round(ring[4]), [4.0547, 0.0, 0.0547]);
        let (ring, _) = loft_ring(&cubic(wavy(4), 3)).unwrap();
        assert_eq!(ring, wavy(4));
        // Two such curves loft into one row of quads.
        let lifted: Vec<Point> = wavy(5).iter().map(|p| [p[0], 3.0, p[2]]).collect();
        let rings = [
            loft_ring(&cubic(wavy(5), 3)).unwrap().0,
            loft_ring(&cubic(lifted, 3)).unwrap().0,
        ];
        let subd = SubD::lofted(&rings, false, false, 1).unwrap();
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (10, 4)
        );
    }

    #[test]
    fn a_polyline_keeps_its_corners() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 2.0, 0.0],
            [5.0, 2.0, 0.0],
        ];
        let (ring, _) = loft_ring(&Curve::Polyline {
            points,
            closed: false,
        })
        .unwrap();
        let ring: Vec<[f64; 2]> = ring
            .into_iter()
            .map(|p| [round(p)[0], round(p)[1]])
            .collect();
        assert_eq!(
            ring,
            vec![
                [0.0, 0.0],
                [1.3333, 0.0],
                [2.0, 0.0],
                [2.3333, 0.6667],
                [2.6667, 1.3333],
                [3.0, 2.0],
                [3.6667, 2.0],
                [5.0, 2.0]
            ]
        );
    }

    #[test]
    fn three_circles_loft_as_in_rhino() {
        let rings: Vec<Vec<Point>> = [(0.0, 2.0), (2.0, 1.0), (4.0, 2.0)]
            .iter()
            .map(|&(z, r)| loft_ring(&circle(z, r)).unwrap().0)
            .collect();
        assert_eq!(rings[0].len(), 8);
        assert!(
            rings[0].iter().any(|&p| round(p) == [2.0, 2.0, 0.0]),
            "a square's corner"
        );
        let subd = SubD::lofted(&rings, true, false, 1).unwrap();
        let cage = subd.cage();
        assert_eq!(
            (cage.vertex_count(), cage.edge_count(), cage.face_count()),
            (24, 40, 16)
        );
        assert_eq!(subd.creases().count(), 16, "the two end rings");
        let finer = SubD::lofted(&rings, true, false, 2).unwrap();
        assert_eq!(
            (finer.cage().vertex_count(), finer.cage().face_count()),
            (40, 32)
        );
        assert!(
            finer
                .cage()
                .vertices()
                .iter()
                .any(|&p| round(p) == [1.5, 0.0, 1.0])
        );
    }
}
