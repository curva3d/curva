//! The `ExtrudeFace` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// `ExtrudeFace`: pulls a flat face of a solid out (or in, for a negative distance) and joins it.
#[derive(Default)]
pub struct ExtrudeFace;

impl Recipe for ExtrudeFace {
    const NAME: &'static str = "extrudeface";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::faces("prompt.extrudeface.face").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.extrudecrv.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Faces(picked)), Some(distance)) = (cx.values.first(), number(cx, 1))
        else {
            return Ok(Made::default());
        };
        let Some((id, face)) = picked.first().copied() else {
            return Ok(Made::default());
        };
        let Some(body) = body_of(cx, id) else {
            return Ok(Made::default());
        };
        let grown = body.face_extruded(face, distance).map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(id, Shape::Body(grown))],
            message: Some(Message::new("done.extrudeface")),
            ..Made::default()
        })
    }
}
