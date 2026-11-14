//! `Top`: turns the last-used viewport into the Top view.

use crate::{Answer, Command, Step, ViewName, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Top;

impl Command for Top {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Show(ViewName::Top))
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
        let mut processor = run(&mut doc, &["Top"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Show(ViewName::Top))
        );
        assert_eq!(processor.take_view_request(), None);
    }
}
