//! `Layouts`: opens the Layouts panel (spec `layouts.md` §1).

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Layouts;

impl Command for Layouts {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowLayouts)
    }
}
