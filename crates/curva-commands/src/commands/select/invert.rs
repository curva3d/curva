//! `Invert`: selects what is not selected, and the reverse.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Invert;

impl Command for Invert {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let chosen = doc.selection();
        let others: Vec<u64> = doc
            .selectable()
            .into_iter()
            .filter(|id| !chosen.contains(id))
            .collect();
        doc.select(&others);
        Step::Done(Some(Message::with(
            "done.select",
            vec![others.len().to_string()],
        )))
    }
}
