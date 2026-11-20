//! `Undo`: steps back one command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Undo;

impl Command for Undo {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let key = if doc.undo() {
            "done.undo"
        } else {
            "note.nothing_undo"
        };
        Step::Done(Some(Message::new(key)))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn says_so_when_there_is_nothing_to_undo() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["undo"]);
        assert_eq!(processor.log(), ["Nothing to undo."]);
    }

    #[test]
    fn restores_the_exact_earlier_state() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["line", "0,0", "1,0"]);
        let one = doc.state().clone();
        for typed in ["line", "0,0", "0,1", "undo"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(doc.state(), &one);
    }
}
