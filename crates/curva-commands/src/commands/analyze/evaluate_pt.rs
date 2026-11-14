//! The `EvaluatePt` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};

/// `EvaluatePt`: the coordinates of a point.
#[derive(Default)]
pub struct EvaluatePt;

impl Recipe for EvaluatePt {
    const NAME: &'static str = "evaluatept";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point("prompt.evaluatept.at")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(p) = point(cx, 0) else {
            return Ok(Made::default());
        };
        Ok(Made {
            message: Some(Message::with(
                "done.evaluatept",
                vec![format!("{:.4}, {:.4}, {:.4}", p[0], p[1], p[2])],
            )),
            ..Made::default()
        })
    }
}
