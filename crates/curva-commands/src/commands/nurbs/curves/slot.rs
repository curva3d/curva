//! The `Slot` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, cross, sub, unit};

/// `Slot`: a slot from the centres of its two ends, then its width.
#[derive(Default)]
pub struct Slot;

impl Recipe for Slot {
    const NAME: &'static str = "slot";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.slot.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.slot.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.slot.width").from(b));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(width) = cx.values.get(2).and_then(Value::number) else {
            return Ok(Made {
                guides: Curve::line(a, b).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, y] = cx.plane();
        let normal = cross(x, y);
        let along = unit(sub(b, a)).ok_or_else(|| Message::new("error.reference_point"))?;
        let side = cross(normal, along);
        let r = width / 2.0;
        if r <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        let parts = vec![
            Curve::line(plus(a, side, -r), plus(b, side, -r)),
            Curve::arc(b, plus(b, side, -r), plus(b, side, r), along, side),
            Curve::line(plus(b, side, r), plus(a, side, r)),
            Curve::arc(
                a,
                plus(a, side, r),
                plus(a, side, -r),
                plus([0.0; 3], along, -1.0),
                plus([0.0; 3], side, -1.0),
            ),
        ]
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| failed(&e))?;
        let slot = Curve::compound(parts).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(slot)],
            message: Some(Message::new("done.slot")),
            ..Made::default()
        })
    }
}
