//! `Redo`: repeats one undone command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Redo;

impl Command for Redo {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let key = if doc.redo() {
            "done.redo"
        } else {
            "note.nothing_redo"
        };
        Step::Done(Some(Message::new(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn repeats_what_undo_took_back() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["line", "0,0", "1,0"]);
        let made = doc.state().clone();
        processor.submit(&mut doc, "undo");
        processor.submit(&mut doc, "redo");
        assert_eq!(doc.state(), &made);
    }

    #[test]
    fn says_so_when_there_is_nothing_to_redo() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["redo"]);
        assert_eq!(processor.log(), ["Nothing to redo."]);
    }
}
