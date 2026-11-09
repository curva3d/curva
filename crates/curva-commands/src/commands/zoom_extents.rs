//! `ZoomExtents`: fits the model in every viewport.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct ZoomExtents;

impl Command for ZoomExtents {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ZoomExtents)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::ViewRequest;
    use curva_document::Document;

    #[test]
    fn asks_for_the_view_and_finishes() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["ZoomExtents"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ZoomExtents)
        );
        assert_eq!(processor.take_view_request(), None);
    }
}
