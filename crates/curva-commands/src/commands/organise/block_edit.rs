//! The `BlockEdit` command.

use super::support::*;
use crate::commands::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `BlockEdit`: edits a block definition in place through one of its instances; run again to
/// save it, every instance following.
#[derive(Default)]
pub struct BlockEdit;

impl Command for BlockEdit {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if doc.block_editing().is_some() {
            doc.finish_block_edit();
            return Step::Done(Some(Message::new("done.blockedit_saved")));
        }
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let Some(&instance) = instances_among(doc, &chosen).first() else {
            if chosen.is_empty() {
                return ask_objects("prompt.blockedit.instance");
            }
            return Step::Failed(Message::new("error.not_instance"));
        };
        match doc.start_block_edit(instance) {
            Some(editing) => {
                doc.select(&editing);
                Step::Done(Some(Message::new("done.blockedit_started")))
            }
            None => Step::Failed(Message::new("error.not_instance")),
        }
    }
}
