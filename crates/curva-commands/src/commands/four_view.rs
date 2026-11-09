//! `4View`: returns to four viewports in their standard arrangement.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct FourView;

impl Command for FourView {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::FourViews)
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
        let mut processor = run(&mut doc, &["4View"]);
        assert!(!processor.is_running());
        assert_eq!(processor.take_view_request(), Some(ViewRequest::FourViews));
        assert_eq!(processor.take_view_request(), None);
    }
}
