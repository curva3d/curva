//! `New`: starts an empty model.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct New;

impl Command for New {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        *doc = Document::default();
        Step::Done(Some(Message::new("done.new")))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn empties_the_model() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "1,0", "new"]);
        assert!(doc.state().objects.is_empty());
        assert!(doc.path().is_none());
    }
}
