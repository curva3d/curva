//! `SelNone`: clears the selection.

use crate::{Answer, Command, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelNone;

impl Command for SelNone {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        doc.select(&[]);
        Step::Done(None)
    }
}
