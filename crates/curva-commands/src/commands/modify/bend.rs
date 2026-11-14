//! The `Bend` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Point, dot, gap, sub, unit};

/// `Bend`: bends objects along a spine: its start and end, then a point the bend passes
/// through.
#[derive(Default)]
pub struct Bend;

impl Recipe for Bend {
    const NAME: &'static str = "bend";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        axis_steps(
            cx,
            "prompt.bend.objects",
            Some(StepDef::point("prompt.bend.through")),
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(start), Some(end), Some(through)) = (point(cx, 1), point(cx, 2), point(cx, 3))
        else {
            return Ok(Made::default());
        };
        let e = unit(sub(end, start)).ok_or_else(|| Message::new("error.reference_point"))?;
        let to = sub(through, start);
        let side = sub(to, e.map(|v| v * dot(to, e)));
        let n = unit(side).ok_or_else(|| Message::new("error.bend_straight"))?;
        // The circle tangent to the spine at its start, through the bend point.
        let radius = dot(to, to) / (2.0 * dot(to, n));
        let length = gap(start, end);
        let centre = plus(start, n, radius);
        let map = move |p: Point| {
            let d = sub(p, start);
            let (s, off) = (dot(d, e), dot(d, n));
            let rest = sub(d, plus(e.map(|v| v * s), n, off));
            let s_in = s.clamp(0.0, length);
            let theta = s_in / radius;
            let on_arc = plus(
                plus(centre, n, -radius * theta.cos()),
                e,
                radius * theta.sin(),
            );
            let inward = unit(sub(centre, on_arc)).unwrap_or(n);
            let tangent: Point = std::array::from_fn(|i| e[i] * theta.cos() + n[i] * theta.sin());
            let bent = plus(plus(on_arc, inward, off), tangent, s - s_in);
            plus(bent, rest, 1.0)
        };
        deform_all(cx, objects(cx, 0), &map, "done.bend")
    }
}
