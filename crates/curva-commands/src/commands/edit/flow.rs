//! The `Flow` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Point, Spline, cross, dot, sub, unit};

/// `Flow`: moves objects from along one curve to along another, keeping where they sit along
/// and beside it.
#[derive(Default)]
pub struct Flow;

impl Recipe for Flow {
    const NAME: &'static str = "flow";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.flow.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.flow.base").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::objects("prompt.flow.target").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, base)), Some((_, target))) = (first_curve(cx, 1), first_curve(cx, 2)) else {
            return Ok(Made::default());
        };
        let (base, target) = (
            Spline::of(&base).map_err(|e| failed(&e))?,
            Spline::of(&target).map_err(|e| failed(&e))?,
        );
        let normal = up(cx);
        let frame = |s: &Spline, t: f64| -> (Point, Point, Point, Point) {
            let p = s.point(t);
            let tangent = unit(s.derivative(t)).unwrap_or([1.0, 0.0, 0.0]);
            let side = unit(cross(normal, tangent)).unwrap_or([0.0, 1.0, 0.0]);
            (p, tangent, side, cross(tangent, side))
        };
        let (b0, b1) = base.domain();
        let (t0, t1) = target.domain();
        let map = move |p: Point| {
            let t = base.closest(p);
            let (bp, bt, bs, bu) = frame(&base, t);
            let d = sub(p, bp);
            let (along, beside, above) = (dot(d, bt), dot(d, bs), dot(d, bu));
            let f = (t - b0) / (b1 - b0);
            let (tp, tt, ts, tu) = frame(&target, t0 + (t1 - t0) * f);
            plus(plus(plus(tp, tt, along), ts, beside), tu, above)
        };
        deform_all(cx, objects(cx, 0), &map, "done.flow")
    }
}
