//! The `ExplodeBlock` command.

use super::support::*;
use crate::commands::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `ExplodeBlock`: turns instances back into the objects of their definitions.
#[derive(Default)]
pub struct ExplodeBlock;

impl Command for ExplodeBlock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let instances = instances_among(doc, &chosen);
        if instances.is_empty() {
            return ask_objects("prompt.explodeblock.objects");
        }
        let made = doc.explode_instances(&instances);
        doc.select(&made);
        Step::Done(Some(Message::with(
            "done.explodeblock",
            vec![instances.len().to_string()],
        )))
    }
}
