//! The `Paraboloid` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, dot, gap, sub, unit};
use std::f64::consts::TAU;

/// `Paraboloid`: its vertex, its focus, then how deep it goes along its axis; a solid closed by a
/// flat top.
#[derive(Default)]
pub struct Paraboloid;

impl Recipe for Paraboloid {
    const NAME: &'static str = "paraboloid";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
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
        let Some(depth) = cx
            .values
            .get(2)
            .and_then(Value::number)
            .filter(|d| *d > 1e-9)
        else {
            return Ok(Made {
                guides: Curve::line(v, f).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, _] = cx.plane();
        let axis = unit(sub(f, v)).ok_or_else(|| Message::new("error.reference_point"))?;
        let side = unit(sub(x, plus([0.0; 3], axis, dot(x, axis))))
            .ok_or_else(|| Message::new("error.reference_point"))?;
        let focal = gap(v, f);
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
