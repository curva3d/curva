//! The `Radius` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::Curve;

/// `Radius`: the radius of a circle or an arc, clicked.
#[derive(Default)]
pub struct Radius;

impl Recipe for Radius {
    const NAME: &'static str = "radius";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::pick("prompt.radius.curve")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(at) = point(cx, 0) else {
            return Ok(Made::default());
        };
        let found = crate::commands::curve_at(cx.doc, at, crate::recipe::ON_CURVE)
            .map_err(|e| failed(&e))?;
        match found {
            Some((_, Curve::Circle { radius, .. } | Curve::Arc { radius, .. })) => Ok(Made {
                message: Some(Message::with("done.radius", vec![format!("{radius:.4}")])),
                ..Made::default()
            }),
            _ => Err(Message::new("error.radius")),
        }
    }
}
