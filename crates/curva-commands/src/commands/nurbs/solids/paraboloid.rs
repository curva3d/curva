//! The `Paraboloid` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::nurbs::curves::parabola::{VERTEX, from_focus};
use crate::commands::support::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, dot, gap, sub, unit};
use std::f64::consts::TAU;

/// `Paraboloid`, as in Rhino: its focus, the way it opens and a point on its rim (#707); with
/// `Vertex`, its vertex, its focus, then how deep it goes along its axis. A solid closed by a flat
/// top.
#[derive(Default)]
pub struct Paraboloid;

impl Recipe for Paraboloid {
    const NAME: &'static str = "paraboloid";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(VERTEX)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        if cx.mode != Some(VERTEX) {
            let mut steps =
                vec![StepDef::point("prompt.paraboloid.focus_first").offering(&[VERTEX])];
            if let Some(f) = point(cx, 0) {
                steps.push(StepDef::point("prompt.paraboloid.direction").from(f));
                if point(cx, 1).is_some() {
                    steps.push(StepDef::point("prompt.paraboloid.end").from(f));
                }
            }
            return steps;
        }
        let mut steps = vec![StepDef::point("prompt.parabola.vertex")];
        if let Some(v) = point(cx, 0) {
            steps.push(StepDef::point("prompt.parabola.focus").from(v));
            if let Some(f) = point(cx, 1) {
                steps.push(StepDef::along("prompt.paraboloid.depth", v, sub(f, v)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(v), Some(f)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        // From the focus, the vertex and focal length follow from the rim point, and so the depth.
        let from_rim = (cx.mode != Some(VERTEX))
            .then(|| point(cx, 2).and_then(|end| Some((from_focus(v, f, end)?, end))))
            .flatten();
        let depth = match from_rim {
            Some(((vertex, axis, _), end)) => Some(dot(sub(end, vertex), axis)),
            None if cx.mode == Some(VERTEX) => cx.values.get(2).and_then(Value::number),
            None => None,
        };
        let Some(depth) = depth.filter(|d| *d > 1e-9) else {
            return Ok(Made {
                guides: Curve::line(v, f).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, _] = cx.plane();
        let (v, axis, focal) = match from_rim {
            Some(((vertex, axis, focal), _)) => (vertex, axis, focal),
            None => (
                v,
                unit(sub(f, v)).ok_or_else(|| Message::new("error.reference_point"))?,
                gap(v, f),
            ),
        };
        let side = unit(sub(x, plus([0.0; 3], axis, dot(x, axis))))
            .ok_or_else(|| Message::new("error.reference_point"))?;

        let half = (4.0 * focal * depth).sqrt();
        // Half the parabola, exact, then across the top to the axis, spun a whole turn: both
        // ends on the axis, so it closes into a solid.
        let rim = plus(plus(v, side, half), axis, depth);
        let outline = Curve::compound(vec![
            parabola_piece(v, axis, side, focal, 0.0, half).map_err(|e| failed(&e))?,
            Curve::line(rim, plus(v, axis, depth)).map_err(|e| failed(&e))?,
        ])
        .map_err(|e| failed(&e))?;
        let body = Body::revolved(&[outline], v, axis, TAU, false).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.paraboloid")),
            ..Made::default()
        })
    }
}
