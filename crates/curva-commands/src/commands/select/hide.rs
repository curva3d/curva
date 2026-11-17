//! `Hide`: takes objects out of sight until `Show`.

use crate::commands::{ask_objects, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Hide;

impl Command for Hide {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.hide.objects");
        };
        doc.hide(&chosen);
        Step::Done(Some(Message::with(
            "done.hide",
            vec![chosen.len().to_string()],
        )))
    }
}
