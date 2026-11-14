//! The `Ungroup` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Edit, Made, Recipe, StepDef};

/// `Ungroup`: breaks up the groups of the objects chosen.
#[derive(Default)]
pub struct Ungroup;

impl Recipe for Ungroup {
    const NAME: &'static str = "Ungroup";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.ungroup.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let chosen = objects(cx, 0).to_vec();
        let groups: std::collections::BTreeSet<u64> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| chosen.contains(&o.id))
            .filter_map(|o| o.group)
            .collect();
        Ok(Made {
            message: Some(Message::with(
                "done.ungroup",
                vec![groups.len().to_string()],
            )),
            edits: vec![Edit::Ungroup(chosen)],
            ..Made::default()
        })
    }
}
