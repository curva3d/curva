//! The other ways Rhino 8 takes a round base (measured, #448): `2Point` (the two ends of a
//! diameter) and `3Point` (three points on the circle), for Cylinder and Cone; and for Sphere,
//! `3Point` (three points on a great circle) and `4Point` (four points on the sphere).

use crate::recipe::{Context, StepDef, Value};
use curva_kernel::{Point, cross, dot, gap, sub, unit};

pub(crate) const TWO_POINT: &str = "2Point";
pub(crate) const THREE_POINT: &str = "3Point";
pub(crate) const FOUR_POINT: &str = "4Point";

/// A round base: its centre, radius, and the plane it lies on (two directions in it).
pub(crate) struct Base {
    pub centre: Point,
    pub r: f64,
    pub x: Point,
    pub y: Point,
}

impl Base {
    /// The direction square to its plane.
    pub fn up(&self) -> Point {
        cross(self.x, self.y)
    }
}

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

/// How many points the `mode` way takes, or `None` for centre and radius.
pub(crate) fn points_taken(mode: Option<&str>) -> Option<usize> {
    match mode {
        Some(TWO_POINT) => Some(2),
        Some(THREE_POINT) => Some(3),
        Some(FOUR_POINT) => Some(4),
        _ => None,
    }
}

/// The steps for the points of the `mode` way, one more each time a point comes in.
pub(crate) fn point_steps(cx: &Context<'_>, mode: &str) -> Vec<StepDef> {
    let prompts: &[&'static str] = match mode {
        TWO_POINT => &["prompt.sphere.start", "prompt.sphere.end"],
        THREE_POINT => &[
            "prompt.round.first",
            "prompt.round.second",
            "prompt.round.third",
        ],
        _ => &[
            "prompt.round.first",
            "prompt.round.second",
            "prompt.round.third",
            "prompt.round.fourth",
        ],
    };
    let mut steps = Vec::new();
    for (k, prompt) in prompts.iter().enumerate() {
        let step = StepDef::point(prompt).offering(&[]);
        steps.push(
            match k.checked_sub(1).and_then(|before| point(cx, before)) {
                Some(from) => step.from(from),
                None => step,
            },
        );
        if point(cx, k).is_none() {
            break;
        }
    }
    steps
}

/// The base picked the `mode` way (2Point or 3Point), once all its points are in.
pub(crate) fn base(cx: &Context<'_>, mode: &str) -> Option<Base> {
    let (a, b) = (point(cx, 0)?, point(cx, 1)?);
    if mode == TWO_POINT {
        let [x, y] = cx.plane();
        let centre = std::array::from_fn(|i| (a[i] + b[i]) / 2.0);
        return Some(Base {
            centre,
            r: gap(a, b) / 2.0,
            x,
            y,
        });
    }
    // Facing the plane being drawn on's side, whichever way round the points went.
    let mut base = circle_through(a, b, point(cx, 2)?)?;
    let [x, y] = cx.plane();
    if dot(base.up(), cross(x, y)) < 0.0 {
        base.y = base.y.map(|v| -v);
    }
    Some(base)
}

/// The circle through three points, on their own plane; `None` when they lie on one line.
pub(crate) fn circle_through(a: Point, b: Point, c: Point) -> Option<Base> {
    let (ab, ac) = (sub(b, a), sub(c, a));
    let normal = cross(ab, ac);
    let n2 = dot(normal, normal);
    if n2 < 1e-18 {
        return None;
    }
    // The circumcentre: a + (|ac|²(n×ab) + |ab|²(ac×n)) / 2|n|².
    let (p, q) = (cross(normal, ab), cross(ac, normal));
    let (ab2, ac2) = (dot(ab, ab), dot(ac, ac));
    let off: Point = std::array::from_fn(|i| (ac2 * p[i] + ab2 * q[i]) / (2.0 * n2));
    let centre = std::array::from_fn(|i| a[i] + off[i]);
    let x = unit(sub(a, centre))?;
    let y = unit(cross(normal, x))?;
    Some(Base {
        centre,
        r: dot(off, off).sqrt(),
        x,
        y,
    })
}

/// The sphere through four points: its centre and radius; `None` when they lie on one plane.
pub(crate) fn sphere_through(a: Point, b: Point, c: Point, d: Point) -> Option<(Point, f64)> {
    // Equally far from `a` and from each other point: three linear equations for the offset.
    let rows = [sub(b, a), sub(c, a), sub(d, a)];
    let rhs = rows.map(|r| dot(r, r) / 2.0);
    let det = |m: [Point; 3]| dot(m[0], cross(m[1], m[2]));
    let whole = det(rows);
    if whole.abs() < 1e-12 {
        return None;
    }
    // Cramer's rule, a column at a time.
    let solve = |col: usize| {
        let mut m = rows;
        for (k, row) in m.iter_mut().enumerate() {
            row[col] = rhs[k];
        }
        det(m) / whole
    };
    let off = [solve(0), solve(1), solve(2)];
    let centre = std::array::from_fn(|i| a[i] + off[i]);
    Some((centre, dot(off, off).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_points_give_their_sphere() {
        // Rhino 8: 0,0,0 10,0,0 5,5,0 5,2,4 gives a sphere 10.0778 across, from x -0.0389.
        let (centre, r) =
            sphere_through([0.0; 3], [10.0, 0.0, 0.0], [5.0, 5.0, 0.0], [5.0, 2.0, 4.0]).unwrap();
        assert!((2.0 * r - 10.0778).abs() < 1e-3, "{r}");
        assert!((centre[0] - r + 0.0389).abs() < 1e-3, "{centre:?}");
        let flat = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        assert!(sphere_through(flat[0], flat[1], flat[2], flat[3]).is_none());
    }

    #[test]
    fn three_points_give_their_circle_on_their_own_plane() {
        let base = circle_through([0.0; 3], [4.0, 0.0, 0.0], [2.0, 2.0, 0.0]).unwrap();
        assert!((base.r - 2.0).abs() < 1e-9 && gap(base.centre, [2.0, 0.0, 0.0]) < 1e-9);
        let upright = circle_through([0.0; 3], [0.0, 4.0, 0.0], [0.0, 2.0, 2.0]).unwrap();
        assert!(base.up()[2].abs() > 0.999 && upright.up()[0].abs() > 0.999);
        assert!(circle_through([0.0; 3], [1.0; 3], [2.0; 3]).is_none());
    }
}
