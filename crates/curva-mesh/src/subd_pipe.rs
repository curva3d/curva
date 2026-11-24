//! Rhino 8's `MultiPipe` for straight lines. Measured (23 November 2026, `-MultiPipe` with
//! curves picked by id): it asks "Select curves to pipe. Press Enter when done", then "Radius <1>
//! ( Cap=No  Struts=1 )". A pipe is a SubD round each run of segments joined end to end:
//!
//! - every ring has 4 cage points, 1.5 × the radius out from the line (so the smooth surface is
//!   the radius away), the first along Rhino's perpendicular to the line (`PerpendicularTo`;
//!   along the first bend's plane when there is a bend), the next turned a quarter towards
//!   "first × direction";
//! - a ring at each free end, creased (Cap=Yes closes each end with one face instead, nothing
//!   creased), Struts rings evenly along each segment's part clear of its corners, and at each
//!   corner one mitred ring: each point where its line along the previous segment meets its line
//!   along the next, which is 1.5 × radius × tan(half the turn) back from the corner on the inside;
//! - past a corner the ring turns with the line, by the least turn that takes one direction to
//!   the next, and each corner's ring lies along the plane of its bend, the rings between turning
//!   evenly to it (measured 24 November 2026 on a curve in space: the twist from one corner's plane
//!   to the next is spread over the struts between, half of it at the strut of Struts=1);
//! - a closed run gives rings all round, with no ends.
//!
//! One straight line of length 10: 12 points, 8 faces, rings at 0, 5 and 10; Struts=2: 16 points;
//! a square corner: 20 points with the middle ring at 4.25; a closed square: 32 points.
//! Networks with junctions are `SubD::pipes` (`subd_pipe_net`). Curved curves are not here yet.

use crate::{Mesh, Point, SmoothError, SubD};

pub(crate) fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| a[k] - b[k])
}

pub(crate) fn add(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| a[k] + b[k])
}

pub(crate) fn scale(a: Point, s: f64) -> Point {
    a.map(|c| c * s)
}

pub(crate) fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|k| a[k] * b[k]).sum()
}

pub(crate) fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn unit(a: Point) -> Option<Point> {
    let l = dot(a, a).sqrt();
    (l > 1e-12).then(|| scale(a, 1.0 / l))
}

/// Rhino's perpendicular to `v` (openNURBS's `ON_3dVector::PerpendicularTo`), made unit.
pub(crate) fn perpendicular_to(v: Point) -> Point {
    let [x, y, z] = v.map(f64::abs);
    let (i, j, k, a, b) = if y > x {
        if z > y {
            (2, 1, 0, v[2], -v[1])
        } else if z >= x {
            (1, 2, 0, v[1], -v[2])
        } else {
            (1, 0, 2, v[1], -v[0])
        }
    } else if z > x {
        (2, 0, 1, v[2], -v[0])
    } else if z > y {
        (0, 2, 1, v[0], -v[2])
    } else {
        (0, 1, 2, v[0], -v[1])
    };
    let mut out = [0.0; 3];
    out[i] = b;
    out[j] = a;
    out[k] = 0.0;
    unit(out).unwrap_or([0.0, 0.0, 1.0])
}

/// `v` turned by the least turn that takes the direction `d1` to `d2`.
pub(crate) fn transported(v: Point, d1: Point, d2: Point) -> Point {
    let c = dot(d1, d2);
    if c <= -1.0 + 1e-12 {
        return scale(v, -1.0);
    }
    let both = add(d1, d2);
    add(
        sub(v, scale(both, dot(v, both) / (1.0 + c))),
        scale(d2, 2.0 * dot(v, d1)),
    )
}

/// `v` (square to `d`) turned by `angle` round `d`.
pub(crate) fn rotated(v: Point, d: Point, angle: f64) -> Point {
    add(scale(v, angle.cos()), scale(cross(d, v), angle.sin()))
}

/// `a` brought within an eighth of a turn of nothing by whole quarter turns.
pub(crate) fn quarter_wrapped(a: f64) -> f64 {
    a - std::f64::consts::FRAC_PI_2 * (a / std::f64::consts::FRAC_PI_2).round()
}

/// The angle round `d` from `u` to the nearest of the four ring directions of `f`.
pub(crate) fn twist(u: Point, f: Point, d: Point) -> f64 {
    quarter_wrapped(dot(d, cross(u, f)).atan2(dot(u, f)))
}

/// The ring direction at a corner where `d1` turns to `d2`, for a ring arriving along `d1` with
/// direction `carried`: Rhino lays each corner's ring along the plane of its bend, so the quarter
/// turn of that nearest `carried`; with no bend, `carried`.
pub(crate) fn bent(carried: Point, d1: Point, d2: Point) -> Point {
    let Some(along) = unit(cross(d1, d2)).and_then(|n| unit(cross(n, d1))) else {
        return carried;
    };
    let mut best = along;
    let mut v = along;
    for _ in 0..3 {
        v = cross(d1, v);
        if dot(v, carried) > dot(best, carried) {
            best = v;
        }
    }
    best
}

/// `ring`'s points turned round so each lies nearest the point of `next` it joins.
fn in_step(ring: [Point; 4], next: [Point; 4]) -> [Point; 4] {
    let cost = |k: usize| -> f64 {
        (0..4)
            .map(|i| {
                let v = sub(ring[(i + k) % 4], next[i]);
                dot(v, v)
            })
            .sum()
    };
    let k = (0..4)
        .min_by(|&a, &b| cost(a).total_cmp(&cost(b)))
        .unwrap_or(0);
    std::array::from_fn(|i| ring[(i + k) % 4])
}

/// The mitred ring at `corner`, where the segment along `d1` turns to `d2`: each point where its
/// line along the first segment (offset `size` along the ring started by `u1`) meets its line
/// along the next.
pub(crate) fn mitred(corner: Point, u1: Point, [d1, d2]: [Point; 2], size: f64) -> [Point; 4] {
    let u2 = cross(u1, d1);
    let c = dot(d1, d2);
    [u1, u2, scale(u1, -1.0), scale(u2, -1.0)].map(|o| {
        let o = scale(o, size);
        let back = dot(o, d2) / (1.0 + c).max(1e-12);
        sub(add(corner, o), scale(d1, back))
    })
}

/// How a pipe is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipeOptions {
    /// The pipe's radius.
    pub radius: f64,
    /// Whether open ends are closed with a face.
    pub cap: bool,
    /// How many rings go along each segment between its ends.
    pub struts: usize,
}

impl Default for PipeOptions {
    /// Rhino's starting options, measured: Radius 1, Cap=No, Struts=1.
    fn default() -> Self {
        Self {
            radius: 1.0,
            cap: false,
            struts: 1,
        }
    }
}

/// The first ring direction of a run whose corners (the segment directions in and out at each
/// point, `None` at open ends) are `corners` and whose first segment runs along `first`: along the
/// first bend's plane, the segments before the bend running straight on from the first, except
/// round a closed run whose first point is the bend.
fn first_ring(corners: &[Option<(Point, Point)>], first: Point, closed: bool) -> Point {
    let first_bend = corners.iter().enumerate().find_map(|(i, c)| {
        let (a, b) = (*c)?;
        unit(cross(a, b)).map(|n| (i, n, a, b))
    });
    match first_bend {
        Some((i, n, d_in, d_out)) => {
            let along = unit(cross(n, d_in)).unwrap_or_else(|| perpendicular_to(d_in));
            if closed && i == 0 {
                transported(along, d_in, d_out)
            } else {
                along
            }
        }
        None => perpendicular_to(first),
    }
}

/// The rings of a pipe along the polyline `points` (a closed one's first not repeated).
pub(crate) fn rings(
    points: &[Point],
    closed: bool,
    options: PipeOptions,
) -> Result<Vec<[Point; 4]>, SmoothError> {
    let bad = || SmoothError("a pipe needs a run of segments of some length".to_owned());
    let m = points.len();
    let segments = if closed { m } else { m - 1 };
    if m < 2 || (closed && m < 3) {
        return Err(bad());
    }
    let dirs: Vec<Point> = (0..segments)
        .map(|i| unit(sub(points[(i + 1) % m], points[i])).ok_or_else(bad))
        .collect::<Result<_, _>>()?;
    let lengths: Vec<f64> = (0..segments)
        .map(|i| dot(sub(points[(i + 1) % m], points[i]), dirs[i]))
        .collect();
    // The corner at point i joins segment i - 1 to segment i.
    let corner = |i: usize| -> Option<(Point, Point)> {
        (closed || (i > 0 && i < m - 1))
            .then(|| (dirs[(i + segments - 1) % segments], dirs[i % segments]))
    };
    let corners: Vec<Option<(Point, Point)>> = (0..m).map(corner).collect();
    let mut u1 = first_ring(&corners, dirs[0], closed);
    let size = 1.5 * options.radius;
    let ring_at = |centre: Point, u1: Point, d: Point| -> [Point; 4] {
        let u2 = cross(u1, d);
        [u1, u2, scale(u1, -1.0), scale(u2, -1.0)].map(|o| add(centre, scale(o, size)))
    };
    // How far each corner's ring reaches back along its segments, on the inside.
    let clear = |i: usize| -> f64 {
        corner(i).map_or(0.0, |(a, b)| {
            let c = dot(a, b).clamp(-1.0, 1.0);
            size * ((1.0 - c) / (1.0 + c).max(1e-12)).sqrt()
        })
    };
    let mut out: Vec<[Point; 4]> = Vec::new();
    if !closed {
        out.push(ring_at(points[0], u1, dirs[0]));
    }
    for s in 0..segments {
        let (d, end) = (dirs[s], (s + 1) % m);
        let (from, to) = (clear(s), lengths[s] - clear(end));
        // The rings turn evenly along the segment to lie along the next corner's bend.
        let turn = corner(end).map_or(0.0, |(d1, d2)| twist(u1, bent(u1, d1, d2), d));
        for k in 1..=options.struts {
            #[allow(clippy::cast_precision_loss)]
            let part = k as f64 / (options.struts + 1) as f64;
            let centre = add(points[s], scale(d, from + (to - from) * part));
            out.push(ring_at(centre, rotated(u1, d, turn * part), d));
        }
        if let Some((d1, d2)) = corner(end) {
            let u = rotated(u1, d, turn);
            out.push(mitred(points[end], u, [d1, d2], size));
            u1 = unit(transported(u, d1, d2)).unwrap_or(u);
        }
    }
    if closed {
        // The corner at the first point came last; it belongs first, its points in step with
        // the ring after it.
        out.rotate_right(1);
        out[0] = in_step(out[0], out[1]);
    } else {
        out.push(ring_at(points[m - 1], u1, dirs[segments - 1]));
    }
    Ok(out)
}

impl SubD {
    /// A pipe round the polyline `points` (closed: its first point not repeated), as Rhino's
    /// MultiPipe (see the module notes).
    ///
    /// # Errors
    /// When the polyline has fewer than two points or a segment of no length.
    pub fn pipe(points: &[Point], closed: bool, options: PipeOptions) -> Result<Self, SmoothError> {
        let rings = rings(points, closed, options)?;
        let n = rings.len();
        let at = |r: usize, k: usize| u32::try_from((r % n) * 4 + k % 4).unwrap_or(u32::MAX);
        let joins = if closed { n } else { n - 1 };
        let mut faces: Vec<Vec<u32>> = (0..joins)
            .flat_map(|r| {
                (0..4).map(move |k| vec![at(r, k), at(r + 1, k), at(r + 1, k + 1), at(r, k + 1)])
            })
            .collect();
        if options.cap && !closed {
            faces.push(vec![at(0, 0), at(0, 1), at(0, 2), at(0, 3)]);
            faces.push(vec![at(n - 1, 3), at(n - 1, 2), at(n - 1, 1), at(n - 1, 0)]);
        }
        let cage = Mesh::new(rings.concat(), &faces).map_err(|e| SmoothError(e.to_string()))?;
        Ok(Self::from_cage(cage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(p: Point) -> Point {
        p.map(|c| (c * 1e4).round() / 1e4 + 0.0)
    }

    fn points_of(subd: &SubD) -> Vec<Point> {
        let mut out: Vec<Point> = subd.cage().vertices().iter().map(|&p| round(p)).collect();
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }

    fn line() -> Vec<Point> {
        vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
    }

    #[test]
    fn a_line_pipes_as_in_rhino() {
        let subd = SubD::pipe(&line(), false, PipeOptions::default()).unwrap();
        let cage = subd.cage();
        assert_eq!(
            (cage.vertex_count(), cage.edge_count(), cage.face_count()),
            (12, 20, 8)
        );
        assert_eq!(subd.creases().count(), 8, "the end rings");
        assert!(points_of(&subd).contains(&[5.0, 1.5, 0.0]));
        assert!(points_of(&subd).contains(&[10.0, 0.0, -1.5]));
        let struts = SubD::pipe(
            &line(),
            false,
            PipeOptions {
                struts: 2,
                ..PipeOptions::default()
            },
        )
        .unwrap();
        assert!(points_of(&struts).contains(&[3.3333, 0.0, 1.5]));
        let capped = SubD::pipe(
            &line(),
            false,
            PipeOptions {
                cap: true,
                ..PipeOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            (capped.cage().face_count(), capped.creases().count()),
            (10, 0)
        );
        let wide = SubD::pipe(
            &line(),
            false,
            PipeOptions {
                radius: 2.0,
                ..PipeOptions::default()
            },
        )
        .unwrap();
        assert!(points_of(&wide).contains(&[0.0, 3.0, 0.0]));
    }

    #[test]
    fn rings_start_on_rhinos_perpendicular() {
        let slanted =
            SubD::pipe(&[[0.0; 3], [3.0, 4.0, 0.0]], false, PipeOptions::default()).unwrap();
        assert!(points_of(&slanted).contains(&[1.2, -0.9, 0.0]));
        let upright =
            SubD::pipe(&[[0.0; 3], [0.0, 0.0, 10.0]], false, PipeOptions::default()).unwrap();
        assert!(points_of(&upright).contains(&[1.5, 0.0, 5.0]));
    }

    #[test]
    fn corners_are_mitred_as_in_rhino() {
        let square = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let l = SubD::pipe(&square, false, PipeOptions::default()).unwrap();
        assert_eq!((l.cage().vertex_count(), l.cage().face_count()), (20, 16));
        let p = points_of(&l);
        for q in [
            [8.5, 1.5, 0.0],
            [11.5, -1.5, 0.0],
            [4.25, 0.0, 1.5],
            [10.0, 5.75, 1.5],
        ] {
            assert!(p.contains(&q), "{q:?}");
        }
        let shallow = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 4.0, 0.0]];
        let p = points_of(&SubD::pipe(&shallow, false, PipeOptions::default()).unwrap());
        for q in [
            [9.7111, 1.5, 0.0],
            [10.2889, -1.5, 0.0],
            [4.8556, 0.0, 1.5],
            [15.1341, 2.0536, 1.5],
        ] {
            assert!(p.contains(&q), "{q:?}");
        }
        let bent = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [15.0, 5.0, 5.0]];
        let p = points_of(&SubD::pipe(&bent, false, PipeOptions::default()).unwrap());
        for q in [
            [0.0, 1.0607, 1.0607],
            [9.2235, 1.0607, 1.0607],
            [10.7765, -1.0607, -1.0607],
        ] {
            assert!(p.contains(&q), "{q:?}");
        }
    }

    #[test]
    fn rings_turn_through_corners_in_two_planes() {
        let path = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [10.0, 10.0, 10.0],
        ];
        let subd = SubD::pipe(&path, false, PipeOptions::default()).unwrap();
        assert_eq!(subd.cage().vertex_count(), 28);
        let p = points_of(&subd);
        for q in [
            [10.0, 8.5, 1.5],
            [10.0, 11.5, -1.5],
            [11.5, 10.0, 5.75],
            [10.0, 8.5, 10.0],
        ] {
            assert!(p.contains(&q), "{q:?}");
        }
    }

    #[test]
    fn a_closed_square_pipes_as_in_rhino() {
        let square = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let subd = SubD::pipe(&square, true, PipeOptions::default()).unwrap();
        let cage = subd.cage();
        assert_eq!((cage.vertex_count(), cage.face_count()), (32, 32));
        assert_eq!(subd.creases().count(), 0);
        assert!(cage.is_closed());
        let p = points_of(&subd);
        for q in [
            [8.5, 8.5, 0.0],
            [-1.5, -1.5, 0.0],
            [5.0, 1.5, 0.0],
            [5.0, 11.5, 0.0],
        ] {
            assert!(p.contains(&q), "{q:?}");
        }
    }

    #[test]
    fn faces_point_outwards() {
        let subd = SubD::pipe(&line(), false, PipeOptions::default()).unwrap();
        let cage = subd.cage();
        let face = cage.face(0);
        let p: Vec<Point> = face.iter().map(|&v| cage.vertices()[v as usize]).collect();
        let normal = cross(sub(p[1], p[0]), sub(p[2], p[1]));
        let centre = scale(p.iter().fold([0.0; 3], |s, &q| add(s, q)), 0.25);
        let out = [0.0, centre[1], centre[2]];
        assert!(dot(normal, out) > 0.0);
    }
}
