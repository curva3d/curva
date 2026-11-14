//! The `Zebra` command.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// `Zebra`: shows black and white stripes on the shaded faces of the last-used viewport, so
/// breaks in smoothness show as kinks in them; run again to stop.
#[derive(Default)]
pub struct Zebra;

impl Command for Zebra {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Zebra)
    }
}
