//! The `Sweep2` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Body;

/// `Sweep2`: a profile swept along two rails, its ends on them.
#[derive(Default)]
pub struct Sweep2;

impl Recipe for Sweep2 {
    const NAME: &'static str = "sweep2";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.sweep2.first").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.sweep2.second").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::objects("prompt.sweep2.profile").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, a)), Some((_, b)), Some((id, profile))) =
            (first_curve(cx, 0), first_curve(cx, 1), first_curve(cx, 2))
        else {
            return Ok(Made::default());
        };
        let body = Body::swept_two_rails(&a, &b, &profile).map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.sweep2")),
            ..Made::default()
        })
    }
}
