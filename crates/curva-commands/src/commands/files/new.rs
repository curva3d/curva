//! `New`: starts an empty model, asking first if that would discard anything.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

const YES: &str = "Yes";
const NO: &str = "No";

/// The running command.
#[derive(Default)]
pub struct New;

fn start_over(doc: &mut Document) -> Step {
    *doc = Document::default();
    Step::Done(Some(Message::new("done.new")))
}

impl Command for New {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None if doc.state().objects.is_empty() => start_over(doc),
            None => Step::Ask(Request {
                options: vec![YES, NO],
                ..Request::new("prompt.new.confirm", Kind::Choice)
            }),
            Some(Answer::Option(YES)) => start_over(doc),
            Some(Answer::Option(NO)) => Step::Done(Some(Message::new("note.new_kept"))),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn empties_the_model() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "1,0", "new", "yes"]);
        assert!(doc.state().objects.is_empty());
        assert!(doc.path().is_none());
    }

    #[test]
    fn asks_before_discarding_anything() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["line", "0,0", "1,0", "new"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(processor.is_running());
    }

    #[test]
    fn no_keeps_the_model() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["line", "0,0", "1,0", "new", "no"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }

    #[test]
    fn another_answer_keeps_the_model_and_asks_again() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["line", "0,0", "1,0", "new", "maybe"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(processor.is_running());
    }

    #[test]
    fn an_empty_model_does_not_ask() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["new"]);
        assert!(!processor.is_running());
    }
}
