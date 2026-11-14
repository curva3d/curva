//! `Perspective`: turns the last-used viewport into the Perspective view.

use crate::{Answer, Command, Step, ViewName, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Perspective;

impl Command for Perspective {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Show(ViewName::Perspective))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{ViewName, ViewRequest};
    use curva_document::Document;

    #[test]
    fn asks_for_the_view_and_finishes() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["Perspective"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Show(ViewName::Perspective))
        );
        assert_eq!(processor.take_view_request(), None);
    }
}
