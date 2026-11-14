//! The `DraftAnalysis` command.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// `DraftAnalysis`: colours the shaded faces of the last-used viewport by their angle to the
/// construction plane's normal: green where they have draft, red where they undercut, yellow
/// between. Run again to stop.
#[derive(Default)]
pub struct DraftAnalysis;

impl Command for DraftAnalysis {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::DraftAnalysis)
    }
}
