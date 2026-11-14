//! The `Unlock` command.

use crate::Message;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};

/// `Unlock`: unlocks every locked object.
#[derive(Default)]
pub struct Unlock;

impl Recipe for Unlock {
    const NAME: &'static str = "Unlock";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        Vec::new()
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let count = cx.doc.state().objects.iter().filter(|o| o.locked).count();
        Ok(Made {
            message: Some(Message::with("done.unlock", vec![count.to_string()])),
            edits: vec![Edit::UnlockAll],
            ..Made::default()
        })
    }
}
