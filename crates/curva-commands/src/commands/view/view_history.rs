//! `UndoView` and `RedoView`, as in Rhino: step back and forward through the last-used
//! viewport's recent views. Camera moves are kept apart from the model's undo, so Ctrl+Z never
//! moves the camera; these are on Home and End, as in Rhino, and on Alt+Z and Alt+Shift+Z.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command: back, or with `FORWARD`, forward.
#[derive(Default)]
pub struct ViewHistory<const FORWARD: bool>;

impl<const FORWARD: bool> Command for ViewHistory<FORWARD> {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(if FORWARD {
            ViewRequest::RedoView
        } else {
            ViewRequest::UndoView
        })
    }
}

/// `UndoView`.
pub type UndoView = ViewHistory<false>;
/// `RedoView`.
pub type RedoView = ViewHistory<true>;

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn ask_for_the_view_history() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["UndoView"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::UndoView));
        let mut processor = run(&mut doc, &["RedoView"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::RedoView));
    }
}
