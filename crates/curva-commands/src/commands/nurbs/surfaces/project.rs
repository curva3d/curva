//! The `Project` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::commands::{bodies_of, curves_of};
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::Curve;

/// `Project`: curves projected straight down the construction plane's normal onto surfaces.
#[derive(Default)]
pub struct Project;

impl Recipe for Project {
    const NAME: &'static str = "project";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.project.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.project.onto").bodies());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let mut made = Made::default();
        let down = up(cx).map(|v| -v);
        for (id, body) in bodies_of(cx.doc, objects(cx, 1)) {
            made.derived.extend(added_curves(
                body.projected(&curves, down).map_err(|e| failed(&e))?,
                id,
            ));
        }
        made.message = Some(Message::with(
            "done.project",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
