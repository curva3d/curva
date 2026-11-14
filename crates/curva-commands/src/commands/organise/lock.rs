//! The `Lock` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};

/// `Lock`: locks objects: they stay in sight, drawn dimmed, but cannot be picked or changed.
#[derive(Default)]
pub struct Lock;

impl Recipe for Lock {
    const NAME: &'static str = "Lock";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.lock.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let chosen = objects(cx, 0).to_vec();
        Ok(Made {
            message: Some(Message::with("done.lock", vec![chosen.len().to_string()])),
            edits: vec![Edit::Lock(chosen)],
            ..Made::default()
        })
    }
}
