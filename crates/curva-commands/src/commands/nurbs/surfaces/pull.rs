//! The `Pull` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `Pull`: curves pulled onto surfaces, to their nearest points.
#[derive(Default)]
pub struct Pull;

impl Recipe for Pull {
    const NAME: &'static str = "pull";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.pull.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.pull.onto").bodies().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 1) else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (_, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let pulled = body.pulled(&curve).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(pulled),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.pull",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
