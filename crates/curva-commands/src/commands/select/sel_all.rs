//! `SelAll`: selects everything that can be selected.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelAll;

impl Command for SelAll {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let all = doc.selectable();
        doc.select(&all);
        Step::Done(Some(Message::with(
            "done.select",
            vec![all.len().to_string()],
        )))
    }
}
