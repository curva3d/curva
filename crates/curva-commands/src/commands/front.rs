//! `Front`: turns the last-used viewport into the Front view.

use crate::{Answer, Command, Step, ViewName, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Front;

impl Command for Front {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Show(ViewName::Front))
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
        let mut processor = run(&mut doc, &["Front"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Show(ViewName::Front))
        );
        assert_eq!(processor.take_view_request(), None);
    }
}
