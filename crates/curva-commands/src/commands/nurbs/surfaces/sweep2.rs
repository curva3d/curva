//! The `Sweep2` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::commands::support::*;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// `Sweep2`: cross sections swept along two rails, their ends on them; any number of sections,
/// anywhere along the rails, as in Rhino (#667).
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
            // Any number of cross sections, anywhere along the rails, as in Rhino (#667).
            steps.push(StepDef::objects("prompt.sweep2.profile").curves());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, a)), Some((_, b))) = (first_curve(cx, 0), first_curve(cx, 1)) else {
            return Ok(Made::default());
        };
        let sections: Vec<(u64, Curve)> = curves_of(cx.doc, objects(cx, 2));
        let Some((id, _)) = sections.first().cloned() else {
            return Ok(Made::default());
        };
        let curves: Vec<Curve> = sections.into_iter().map(|(_, c)| c).collect();
        let body = Body::swept_along_rails(&a, &b, &curves).map_err(|e| failed(&e))?;
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
