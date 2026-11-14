//! `ZoomSelected`: fits the selection in every viewport.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct ZoomSelected;

impl Command for ZoomSelected {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ZoomSelected)
    }
}

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn asks_for_the_view() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["zs"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ZoomSelected)
        );
    }
}
