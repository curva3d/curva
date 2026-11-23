//! The block MultiPipe puts where three or more lines meet. Measured in Rhino 8 (23 November
//! 2026): the block is a closed cage with one four-sided face for each line, so it has two points
//! more than it has lines (5 for three lines, 6 for a flat cross of four, 8 for six lines along
//! the axes), and each line's pipe joins its face. Where the points go:
//!
//! - **Lines whose directions end on one flat circle** (any three lines, any number in one plane):
//!   one point either side of that circle's plane, along its axis, and one between each pair of
//!   neighbouring lines, halfway along the circle's arc between them (for lines in one plane: the
//!   mitre between them). A T has 5: above, below, the two inside corners and the point opposite
//!   the stem.
//! - **Otherwise** the directions' convex hull: a point off each hull face, along its outward
//!   normal; a line on three faces shares one more point with a neighbour on three faces, halfway
//!   between the two lines (four lines towards a tetrahedron's corners give 6).
//!
//! Every point is pushed out until it is 1.5 × the radius from the lines it sits between, the
//! same distance as the rings of the pipes. Where Rhino's choice could not be told from the
//! measurements: which neighbours share a point when several pairings work is a fixed order here
//! (Rhino's tetrahedron and four-of-five cases come out the same), the off-face point of a face
//! through the junction itself sits square to the face (Rhino tips it slightly towards the face's
//! lines: by 18° for a cross with a fourth line up), and the poles of five or more lines in one
//! plane sit square to the plane (Rhino tips them by about 11° for five). A line with more than
//! four neighbours on the hull (a fan of six round a seventh) is refused.

use crate::SmoothError;
use crate::subd_pipe::{add, cross, dot, perpendicular_to, scale, sub, unit};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

type Point = [f64; 3];

const FLAT: f64 = 1e-9;

/// The block at one junction.
pub(crate) struct Node {
    /// Its cage points.
    pub points: Vec<Point>,
    /// For each line, in the order given: the four points of its face (indices into `points`).
    pub faces: Vec<[usize; 4]>,
}

impl Node {
    /// How far along the line `arm` (unit, away from `centre`) the line's face reaches.
    pub fn reach(&self, centre: Point, arm: Point, line: usize) -> f64 {
        self.faces[line]
            .iter()
            .map(|&p| dot(sub(self.points[p], centre), arm))
            .fold(f64::MIN, f64::max)
    }

    /// The way the line's pipe rings sit as they leave the block: one of the four directions
    /// (square to `arm`) its ring points take. Rhino turns the ring to the average of where the
    /// face's corners lie round the line, each a quarter turn on from the last.
    pub fn frame(&self, centre: Point, arm: Point, line: usize) -> Point {
        let e = perpendicular_to(arm);
        let f = cross(arm, e);
        let mut angles: Vec<f64> = self.faces[line]
            .iter()
            .map(|&p| {
                let v = sub(self.points[p], centre);
                dot(v, f).atan2(dot(v, e))
            })
            .collect();
        angles.sort_by(f64::total_cmp);
        #[allow(clippy::cast_precision_loss)]
        let offsets: Vec<f64> = angles
            .iter()
            .enumerate()
            .map(|(k, a)| a - FRAC_PI_2 * k as f64)
            .collect();
        // A plain average, the angles taken from Rhino's perpendicular round to just under a half
        // turn either way: this is what settles the cases where two turns are equally near
        // (four lines out of five gave 22.5° one way, not the other).
        let turn = offsets.iter().sum::<f64>() / 4.0;
        add(scale(e, turn.cos()), scale(f, turn.sin()))
    }
}

/// `a` brought within an eighth of a turn of nothing by whole quarter turns.
pub(crate) fn quarter_wrapped(a: f64) -> f64 {
    a - FRAC_PI_2 * (a / FRAC_PI_2).round()
}

/// The point along `direction` from `centre` that is `size` from the line along `arm`.
fn out_to(centre: Point, direction: Point, arm: Point, size: f64) -> Point {
    let c = dot(direction, arm);
    add(
        centre,
        scale(direction, size / (1.0 - c * c).max(1e-12).sqrt()),
    )
}

fn junction_error(why: &str) -> SmoothError {
    SmoothError(format!("MultiPipe cannot join these lines: {why}"))
}

/// The block where the lines along `arms` (unit, away from `centre`) meet, `size` out from them.
pub(crate) fn node(centre: Point, arms: &[Point], size: f64) -> Result<Node, SmoothError> {
    for (i, a) in arms.iter().enumerate() {
        if arms[..i].iter().any(|b| dot(*a, *b) > 1.0 - 1e-9) {
            return Err(junction_error("two of them run on top of each other"));
        }
    }
    let n = unit(cross(sub(arms[1], arms[0]), sub(arms[2], arms[0])))
        .ok_or_else(|| junction_error("their directions are not distinct"))?;
    let h = dot(n, arms[0]);
    if arms.iter().all(|a| (dot(n, *a) - h).abs() < FLAT) {
        Ok(on_a_circle(
            centre,
            arms,
            size,
            if h < 0.0 { scale(n, -1.0) } else { n },
        ))
    } else {
        on_a_hull(centre, arms, size)
    }
}

/// Lines whose directions end on one circle, square to `n` (pointing to the circle's side).
fn on_a_circle(centre: Point, arms: &[Point], size: f64, n: Point) -> Node {
    let h = dot(n, arms[0]);
    let rho = (1.0 - h * h).max(0.0).sqrt();
    let e = unit(sub(arms[0], scale(n, h))).unwrap_or_else(|| perpendicular_to(n));
    let f = cross(n, e);
    let angle = |a: Point| dot(a, f).atan2(dot(a, e)).rem_euclid(TAU);
    let mut order: Vec<usize> = (0..arms.len()).collect();
    order.sort_by(|&i, &j| angle(arms[i]).total_cmp(&angle(arms[j])));
    let mut points = vec![
        add(centre, scale(n, size / rho)),
        sub(centre, scale(n, size / rho)),
    ];
    let mut faces = vec![[0, 1, 0, 0]; arms.len()];
    for (k, &i) in order.iter().enumerate() {
        let j = order[(k + 1) % order.len()];
        let (from, to) = (angle(arms[i]), angle(arms[j]));
        let gap = (to - from).rem_euclid(TAU);
        let mid = from + if gap < 1e-12 { PI } else { gap / 2.0 };
        let towards = add(
            scale(n, h),
            scale(add(scale(e, mid.cos()), scale(f, mid.sin())), rho),
        );
        points.push(out_to(centre, towards, arms[i], size));
        let p = points.len() - 1;
        faces[i][3] = p;
        faces[j][2] = p;
    }
    Node { points, faces }
}

/// A face of the directions' hull: its outward normal and the lines on it.
struct HullFace {
    normal: Point,
    lines: Vec<usize>,
}

fn hull(arms: &[Point]) -> Vec<HullFace> {
    let mut faces: Vec<HullFace> = Vec::new();
    let m = arms.len();
    for i in 0..m {
        for j in i + 1..m {
            for k in j + 1..m {
                let Some(mut normal) = unit(cross(sub(arms[j], arms[i]), sub(arms[k], arms[i])))
                else {
                    continue;
                };
                let h = dot(normal, arms[i]);
                let side: Vec<f64> = arms.iter().map(|a| dot(normal, *a) - h).collect();
                if side.iter().any(|s| *s > FLAT) {
                    if side.iter().any(|s| *s < -FLAT) {
                        continue;
                    }
                    normal = scale(normal, -1.0);
                }
                let lines: Vec<usize> = (0..m).filter(|&l| side[l].abs() <= FLAT).collect();
                if !faces.iter().any(|f| f.lines == lines) {
                    faces.push(HullFace { normal, lines });
                }
            }
        }
    }
    faces
}

/// Whether lines `a` and `b` are neighbours along an edge of the hull face.
fn along_edge(face: &HullFace, arms: &[Point], a: usize, b: usize) -> bool {
    if !face.lines.contains(&a) || !face.lines.contains(&b) {
        return false;
    }
    let e = perpendicular_to(face.normal);
    let f = cross(face.normal, e);
    let angle = |l: usize| dot(arms[l], f).atan2(dot(arms[l], e));
    let mut round: Vec<usize> = face.lines.clone();
    round.sort_by(|&x, &y| angle(x).total_cmp(&angle(y)));
    let at = |l: usize| round.iter().position(|&x| x == l).unwrap_or(0);
    let gap = (at(a) + round.len() - at(b)) % round.len();
    gap == 1 || gap == round.len() - 1
}

/// Pairs of lines on three faces each that can share a point: neighbours along a hull edge, not
/// opposite each other. In a fixed order: those meeting at the smallest angle first, then by the
/// shared point's direction, highest y, then x, then z.
fn pairings(faces: &[HullFace], arms: &[Point], threes: &[usize]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (k, &a) in threes.iter().enumerate() {
        for &b in &threes[k + 1..] {
            let opposite = dot(arms[a], arms[b]) < -1.0 + 1e-9;
            if !opposite && faces.iter().any(|f| along_edge(f, arms, a, b)) {
                pairs.push((a, b));
            }
        }
    }
    let key = |&(a, b): &(usize, usize)| {
        let u = unit(add(arms[a], arms[b])).unwrap_or([0.0; 3]);
        #[allow(clippy::cast_possible_truncation)]
        let r = |x: f64| (x * 1e9).round() as i64;
        (-r(dot(arms[a], arms[b])), -r(u[1]), -r(u[0]), -r(u[2]))
    };
    pairs.sort_by_key(key);
    pairs
}

/// A choice of `pairs` covering every line in `threes` once, first in the pairs' order.
fn matching(pairs: &[(usize, usize)], left: &[usize]) -> Option<Vec<(usize, usize)>> {
    let Some(&first) = left.first() else {
        return Some(Vec::new());
    };
    for &(a, b) in pairs {
        let other = if a == first {
            b
        } else if b == first {
            a
        } else {
            continue;
        };
        if !left.contains(&other) {
            continue;
        }
        let rest: Vec<usize> = left[1..].iter().copied().filter(|&l| l != other).collect();
        if let Some(mut found) = matching(pairs, &rest) {
            found.push((a, b));
            return Some(found);
        }
    }
    None
}

fn on_a_hull(centre: Point, arms: &[Point], size: f64) -> Result<Node, SmoothError> {
    let faces = hull(arms);
    let mut points: Vec<Point> = faces
        .iter()
        .map(|f| out_to(centre, f.normal, arms[f.lines[0]], size))
        .collect();
    let mut around: Vec<Vec<usize>> = (0..arms.len())
        .map(|l| {
            (0..faces.len())
                .filter(|&f| faces[f].lines.contains(&l))
                .collect()
        })
        .collect();
    if around.iter().any(|a| a.len() > 4) {
        return Err(junction_error(
            "one line has more than four neighbours round it",
        ));
    }
    let threes: Vec<usize> = (0..arms.len()).filter(|&l| around[l].len() == 3).collect();
    let pairs = pairings(&faces, arms, &threes);
    let matched = matching(&pairs, &threes)
        .ok_or_else(|| junction_error("their lines cannot be paired round the junction"))?;
    for (a, b) in matched {
        let towards = unit(add(arms[a], arms[b])).unwrap_or(arms[a]);
        points.push(out_to(centre, towards, arms[a], size));
        around[a].push(points.len() - 1);
        around[b].push(points.len() - 1);
    }
    let faces = around.iter().map(|a| [a[0], a[1], a[2], a[3]]).collect();
    Ok(Node { points, faces })
}
