//! The `Group` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};

/// `Group`: groups objects, so picking one picks them all.
#[derive(Default)]
pub struct Group;

impl Recipe for Group {
    const NAME: &'static str = "Group";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.group.objects").at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let chosen = objects(cx, 0).to_vec();
        Ok(Made {
            message: Some(Message::with("done.group", vec![chosen.len().to_string()])),
            edits: vec![Edit::Group(chosen)],
            ..Made::default()
        })
    }
}
