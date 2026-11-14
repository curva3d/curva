//! `SelFilter` and `SelFilterToggle`: the selection filter, as in Rhino. `SelFilter` shows or
//! hides its row in the status bar; `SelFilterToggle` turns it off and on without losing the
//! kinds ticked. The interface keeps the filter, as the document knows nothing of picking.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running `SelFilter`.
#[derive(Default)]
pub struct SelFilter;

impl Command for SelFilter {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowSelectionFilter)
    }
}

/// The running `SelFilterToggle`.
#[derive(Default)]
pub struct SelFilterToggle;

impl Command for SelFilterToggle {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ToggleSelectionFilter)
    }
}

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn both_hand_the_filter_to_the_interface() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["selfilter"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ShowSelectionFilter)
        );
        let mut processor = run(&mut doc, &["selfiltertoggle"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ToggleSelectionFilter)
        );
    }
}
