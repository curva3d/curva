//! Box's other ways, as Rhino 8 makes them (measured, #448): `Diagonal` (two opposite corners in
//! space), `3Point` (a corner, the end of the first edge, a point setting the width, then the
//! height) and `Vertical` (a corner, the end of an edge on the plane, a point setting the height,
//! then the depth, which goes the way the edge and the plane's up make: behind an edge along X).
//! Kept apart from `box_solid.rs` so the mesh and SubD boxes can take the same ways.

use crate::recipe::{Context, StepDef, Value};
use curva_kernel::{Point, cross, dot, sub, unit};

pub(crate) const DIAGONAL: &str = "Diagonal";
pub(crate) const THREE_POINT: &str = "3Point";
pub(crate) const VERTICAL: &str = "Vertical";

/// Whether `mode` is one of these ways.
pub(crate) fn is_way(mode: Option<&str>) -> bool {
    matches!(mode, Some(DIAGONAL | THREE_POINT | VERTICAL))
}

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    cross(x, y)
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    std::array::from_fn(|i| p[i] + direction[i] * amount)
}

/// The steps of `mode` given the answers so far.
pub(crate) fn steps(cx: &Context<'_>, mode: &str) -> Vec<StepDef> {
    let mut steps = vec![StepDef::point("prompt.box.first")];
    let Some(first) = point(cx, 0) else {
        return steps;
    };
    if mode == DIAGONAL {
        steps.push(StepDef::point("prompt.box.opposite").from(first));
        return steps;
    }
    steps.push(StepDef::point("prompt.box.edge_end").from(first));
    let Some(edge_end) = point(cx, 1) else {
        return steps;
    };
    let side = if mode == VERTICAL {
        "prompt.box.height_point"
    } else {
        "prompt.box.width_point"
    };
    steps.push(StepDef::point(side).from(edge_end));
    if let Some(third) = point(cx, 2)
        && let Some((_, axes, _)) = frame(cx, mode, first, edge_end, third)
    {
        let last = if mode == VERTICAL {
            "prompt.box.depth"
        } else {
            "prompt.box.height"
        };
        steps.push(StepDef::along(last, third, axes[2]));
    }
    steps
}

/// For `3Point` and `Vertical`: the box's corner, its three directions (each of length 1) and the
/// first two sides' lengths, from its first three points.
fn frame(
    cx: &Context<'_>,
    mode: &str,
    first: Point,
    edge_end: Point,
    third: Point,
) -> Option<(Point, [Point; 3], [f64; 2])> {
    let along = sub(edge_end, first);
    let length = dot(along, along).sqrt();
    let a = unit(along)?;
    let up = up(cx);
    // The second direction: across the edge on the plane for 3Point, up for Vertical.
    let b = if mode == VERTICAL {
        up
    } else {
        unit(cross(up, a))?
    };
    let reach = dot(sub(third, edge_end), b);
    let (b, second) = if reach < 0.0 {
        (b.map(|v| -v), -reach)
    } else {
        (b, reach)
    };
    // The third: up for 3Point, behind the vertical side for Vertical (as Rhino: X then up
    // gives a depth towards -Y).
    let c = if mode == VERTICAL {
        unit(cross(a, up))?
    } else {
        up
    };
    Some((first, [a, b, c], [length, second]))
}

/// The box `mode` makes from the answers, as a corner, two directions of length 1 and the sizes
/// along them and the direction they make, ready for `Body::cuboid_on`; `None` until it is known.
pub(crate) fn placed(cx: &Context<'_>, mode: &str) -> Option<(Point, Point, Point, [f64; 3])> {
    let first = point(cx, 0)?;
    if mode == DIAGONAL {
        let opposite = point(cx, 1)?;
        let [x, y] = cx.plane();
        let z = up(cx);
        let off = sub(opposite, first);
        let (dx, dy, dz) = (dot(off, x), dot(off, y), dot(off, z));
        let corner = plus(
            plus(plus(first, x, dx.min(0.0)), y, dy.min(0.0)),
            z,
            dz.min(0.0),
        );
        return Some((corner, x, y, [dx.abs(), dy.abs(), dz.abs()]));
    }
    let (corner, [a, b, c], [length, second]) =
        frame(cx, mode, first, point(cx, 1)?, point(cx, 2)?)?;
    let last = match cx.values.get(3)? {
        Value::Number(last) => *last,
        _ => return None,
    };
    // The cuboid grows along a × b; where the box goes the other way, its corner moves there.
    let made = cross(a, b);
    let (corner, last) = if dot(made, c) * last < 0.0 {
        (plus(corner, made, -last.abs()), last.abs())
    } else {
        (corner, last.abs())
    };
    Some((corner, a, b, [length, second, last]))
}
