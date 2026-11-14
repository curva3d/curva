//! The `MatchProperties` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `MatchProperties`: copies layer, print width, linetype and name from one object to others:
/// the objects to change, then the one to match.
#[derive(Default)]
pub struct MatchProperties {
    targets: Vec<u64>,
}

impl Command for MatchProperties {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.targets.is_empty() {
            self.targets = chosen(doc, &answer);
            if self.targets.is_empty() {
                return ask_objects("prompt.matchproperties.targets");
            }
            return ask_objects("prompt.matchproperties.source");
        }
        match answer {
            Some(Answer::Objects(list)) if !list.is_empty() => {
                let count = doc.match_properties(list[0], &self.targets);
                Step::Done(Some(Message::with(
                    "done.matchproperties",
                    vec![count.to_string()],
                )))
            }
            _ => ask_objects("prompt.matchproperties.source"),
        }
    }
}
