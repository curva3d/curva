//! The `ReplaceBlock` command.

use super::support::*;
use crate::commands::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `ReplaceBlock`: points instances at another definition: the instances, then its name.
#[derive(Default)]
pub struct ReplaceBlock {
    instances: Vec<u64>,
}

impl Command for ReplaceBlock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.instances.is_empty() => {
                let Some(definition) = doc.definition_named(name.trim()).map(|d| d.id) else {
                    return Step::Failed(Message::with("error.no_block", vec![name]));
                };
                let count = doc.replace_definition(&self.instances, definition);
                Step::Done(Some(Message::with(
                    "done.replaceblock",
                    vec![count.to_string()],
                )))
            }
            Some(Answer::Objects(list)) => {
                self.instances = instances_among(doc, &list);
                Step::Ask(Request::new("prompt.replaceblock.name", Kind::Path))
            }
            None => {
                self.instances = instances_among(doc, &doc.selection());
                if self.instances.is_empty() {
                    return ask_objects("prompt.replaceblock.objects");
                }
                Step::Ask(Request::new("prompt.replaceblock.name", Kind::Path))
            }
            _ => unexpected(),
        }
    }
}
