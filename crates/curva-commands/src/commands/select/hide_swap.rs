//! The `HideSwap` command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `HideSwap`: shows what was hidden and hides what was shown.
#[derive(Default)]
pub struct HideSwap;

impl Command for HideSwap {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let shown: Vec<u64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| !o.hidden)
            .map(|o| o.id)
            .collect();
        doc.show_all();
        doc.hide(&shown);
        Step::Done(Some(Message::with(
            "done.hideswap",
            vec![shown.len().to_string()],
        )))
    }
}
