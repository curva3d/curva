//! `Show`: brings back every hidden object.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Show;

impl Command for Show {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let shown = doc.show_all();
        Step::Done(Some(Message::with("done.show", vec![shown.to_string()])))
    }
}
