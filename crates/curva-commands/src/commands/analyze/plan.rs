//! The `Plan` command.

use crate::recipe::{Context, Made, Recipe, StepDef};
use crate::{Message, ViewRequest};

/// `Plan`: turns the last-used viewport to look straight down on its construction plane.
#[derive(Default)]
pub struct Plan;

impl Recipe for Plan {
    const NAME: &'static str = "Plan";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        Vec::new()
    }

    fn make(&self, _cx: &Context<'_>) -> Result<Made, Message> {
        Ok(Made {
            view: Some(ViewRequest::Plan),
            ..Made::default()
        })
    }
}
