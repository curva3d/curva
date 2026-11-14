//! Helpers several commands in this tab share.

use crate::Answer;
use crate::Command;
use crate::Message;
use crate::Step;
use crate::commands::support::*;
use curva_document::Document;

/// Selects every selectable curve of one kind: `SelLine`, `SelPolyline`, `SelCircle`, `SelArc`,
/// `SelClosedCrv`, `SelOpenCrv`.
#[derive(Default)]
pub struct SelKind<const KIND: u8>;

impl<const KIND: u8> Command for SelKind<KIND> {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let kind = [
            CurveKind::Line,
            CurveKind::Polyline,
            CurveKind::Circle,
            CurveKind::Arc,
            CurveKind::Closed,
            CurveKind::Open,
        ][usize::from(KIND)];
        let open = doc.selectable();
        let found: Vec<u64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| open.contains(&o.id) && o.curve().is_some_and(|c| kind.takes(c)))
            .map(|o| o.id)
            .collect();
        doc.select(&found);
        Step::Done(Some(Message::with(
            "done.select",
            vec![found.len().to_string()],
        )))
    }
}
