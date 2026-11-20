//! The `FilletSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `FilletSrf` (or, with `CHAMFER`, `ChamferSrf`): a fillet (or chamfer) surface of a radius
/// between two surfaces, on the sides facing each other.
#[derive(Default)]
pub struct FilletSrf<const CHAMFER: bool>;

impl<const CHAMFER: bool> Recipe for FilletSrf<CHAMFER> {
    const NAME: &'static str = if CHAMFER { "chamfersrf" } else { "filletsrf" };

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.filletsrf.first").bodies().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.filletsrf.second").bodies().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.filletsrf.radius").remembered("Radius", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((id, a)), Some((_, b)), Some(radius)) =
            (first_body(cx, 0), first_body(cx, 1), number(cx, 2))
        else {
            return Ok(Made::default());
        };
        if radius <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let made = a
            .filleted_with(&b, radius, CHAMFER)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(made),
                from: id,
                history: None,
            }],
            message: Some(Message::new(if CHAMFER {
                "done.chamfersrf"
            } else {
                "done.filletsrf"
            })),
            ..Made::default()
        })
    }
}
