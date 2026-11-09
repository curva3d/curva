//! `Hide`: takes objects out of sight until `Show`.

use super::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Hide;

impl Command for Hide {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.hide.objects");
        }
        doc.hide(&chosen);
        Step::Done(Some(Message::with(
            "done.hide",
            vec![chosen.len().to_string()],
        )))
    }
}
