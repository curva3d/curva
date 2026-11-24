//! Where Bridge puts its rows between two border runs, as Rhino 8 does (measured 24 November
//! 2026, `-Bridge` on edges picked by script, eight cases matched to 4 places):
//!
//! - each pair of joined points is spanned by a cubic Bézier, and the rows sit at even steps of it
//!   (Segments=2, 3 and 4 measured);
//! - the curve leaves each point along its outward direction: each border face gives the
//!   direction square to its border edge in the face, away from it (from the face's Newell
//!   normal, so a face that is not flat counts as a whole), and a point between two faces takes
//!   the unit average of theirs;
//! - each inner control point stands (2/3 × the gap) / (2 − |cos θ|) out from its end, the gap
//!   being the distance between the pair and θ the angle between the two directions: 2/3 of the
//!   gap for parallel or facing borders, 1/3 for square ones (a sweep of 13 angles);
//! - `Straightness` (0 to 1, 1 at first) mixes that curve with the straight line between the
//!   pair, 1 giving the straight line.

use crate::{Mesh, Point};

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: Point, s: f64) -> Point {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The vector at length 1, or zero when it has none.
fn unit(v: Point) -> Point {
    let n = dot(v, v).sqrt();
    if n > 0.0 {
        scaled(v, 1.0 / n)
    } else {
        [0.0; 3]
    }
}

/// The face's Newell normal, at length 1.
fn newell(at: &[Point], face: &[u32]) -> Point {
    let n = face.len();
    let sum = (0..n).fold([0.0; 3], |s, k| {
        let (a, b) = (at[face[k] as usize], at[face[(k + 1) % n] as usize]);
        add(s, cross(a, b))
    });
    unit(sum)
}

/// The outward direction of the face along border edge `a`–`b`: square to the edge, in the face,
/// away from it. Zero when no face has that edge.
fn edge_outward(cage: &Mesh, a: u32, b: u32) -> Point {
    let at = cage.vertices();
    for face in cage.faces() {
        let n = face.len();
        for k in 0..n {
            let (p, q) = (face[k], face[(k + 1) % n]);
            if (p, q) == (a, b) || (p, q) == (b, a) {
                // The face lies to the left of its edges round its normal.
                let along = sub(at[q as usize], at[p as usize]);
                return unit(cross(along, newell(at, face)));
            }
        }
    }
    [0.0; 3]
}

/// Each point of a border run's outward direction: its edges' directions, averaged.
fn outward(cage: &Mesh, run: &[u32]) -> Vec<Point> {
    let edges: Vec<Point> = run
        .windows(2)
        .map(|w| edge_outward(cage, w[0], w[1]))
        .collect();
    (0..run.len())
        .map(|k| {
            let before = if k > 0 { edges[k - 1] } else { [0.0; 3] };
            let after = edges.get(k).copied().unwrap_or([0.0; 3]);
            unit(add(before, after))
        })
        .collect()
}

/// The points of a bridge's inner rows, row by row from `first` towards `second` (`rows` − 1 of
/// them), on Rhino's curves at this `straightness`.
pub(crate) fn bridge_rows(
    cage: &Mesh,
    first: &[u32],
    second: &[u32],
    rows: u32,
    straightness: f64,
) -> Vec<Vec<Point>> {
    let at = cage.vertices();
    let (out_a, out_b) = (outward(cage, first), outward(cage, second));
    let s = straightness.clamp(0.0, 1.0);
    let controls: Vec<[Point; 4]> = (0..first.len())
        .map(|k| {
            let (p0, p3) = (at[first[k] as usize], at[second[k] as usize]);
            let (da, db) = (out_a[k], out_b[k]);
            let gap = dot(sub(p3, p0), sub(p3, p0)).sqrt();
            let reach = 2.0 / 3.0 * gap / (2.0 - dot(da, db).abs());
            let line = |t: f64| add(p0, scaled(sub(p3, p0), t));
            let mix =
                |curved: Point, straight: Point| add(scaled(curved, 1.0 - s), scaled(straight, s));
            let p1 = mix(add(p0, scaled(da, reach)), line(1.0 / 3.0));
            let p2 = mix(add(p3, scaled(db, reach)), line(2.0 / 3.0));
            [p0, p1, p2, p3]
        })
        .collect();
    (1..rows)
        .map(|r| {
            let t = f64::from(r) / f64::from(rows);
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * t * u * u, 3.0 * t * t * u, t * t * t];
            controls
                .iter()
                .map(|c| (0..4).fold([0.0; 3], |sum, i| add(sum, scaled(c[i], w[i]))))
                .collect()
        })
        .collect()
}

#[cfg(test)]
#[path = "subd_bridge_data.rs"]
mod rhino;

#[cfg(test)]
mod tests {
    use super::rhino::{self, Case};
    use crate::{Mesh, Point, SubD};

    /// A strip of two faces between an inner row and a border row of three points.
    fn strip(rows: &[[Point; 3]; 2]) -> SubD {
        let points: Vec<Point> = rows.iter().flatten().copied().collect();
        SubD::new(Mesh::new(points, &[[0, 1, 4, 3], [1, 2, 5, 4]]).unwrap())
    }

    fn near(a: Point, b: Point) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 2e-4)
    }

    /// The two strips' border rows bridged, against Rhino's cage point for point.
    fn bridges_as_in_rhino(case: &Case) {
        let two = strip(&case.a).joined(&strip(&case.b)).unwrap();
        let bridged = two
            .bridged(&[3, 4, 5], &[9, 10, 11], case.segments, case.straightness)
            .unwrap();
        let cage = bridged.cage();
        assert_eq!(cage.vertex_count(), case.points);
        assert_eq!(cage.face_count(), case.faces);
        for &p in case.cage {
            assert!(
                cage.vertices().iter().any(|&q| near(p, q)),
                "Rhino's point {p:?} is missing from {:?}",
                cage.vertices()
            );
        }
    }

    #[test]
    fn straight_and_tilted_strips_bridge_on_rhinos_curves() {
        for case in [
            &rhino::STRAIGHT_4,
            &rhino::HALF_3,
            &rhino::TILTED_3,
            &rhino::FACING_4,
        ] {
            bridges_as_in_rhino(case);
        }
    }

    #[test]
    fn bent_curved_fanned_and_leaning_strips_bridge_on_rhinos_curves() {
        for case in [
            &rhino::BENT_2,
            &rhino::CURVED_3,
            &rhino::FAN_2,
            &rhino::LEANING_3,
        ] {
            bridges_as_in_rhino(case);
        }
    }

    #[test]
    fn straightness_one_bridges_on_straight_lines() {
        let case = &rhino::TILTED_3;
        let two = strip(&case.a).joined(&strip(&case.b)).unwrap();
        let bridged = two.bridged(&[3, 4, 5], &[9, 10, 11], 2, 1.0).unwrap();
        let p = bridged.cage().vertices();
        // Halfway between (0, 1, 2) and (0, 4, 2).
        assert!(p.iter().any(|&q| near(q, [0.0, 2.5, 2.0])));
    }
}
