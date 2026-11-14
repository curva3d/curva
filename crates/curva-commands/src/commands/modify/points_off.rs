//! `PointsOff`: hides every control point.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct PointsOff;

impl Command for PointsOff {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        doc.points_off();
        Step::Done(Some(Message::new("done.pointsoff")))
    }
}
