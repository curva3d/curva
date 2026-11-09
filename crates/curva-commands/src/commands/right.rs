//! `Right`: turns the last-used viewport into the Right view.

use crate::{Answer, Command, Step, ViewName, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Right;

impl Command for Right {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Show(ViewName::Right))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::{ViewName, ViewRequest};
    use curva_document::Document;

    #[test]
    fn asks_for_the_view_and_finishes() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["Right"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Show(ViewName::Right))
        );
        assert_eq!(processor.take_view_request(), None);
    }
}
