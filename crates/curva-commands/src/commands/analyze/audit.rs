//! The `Audit` command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `Audit`: checks every object and reports broken geometry, selecting it.
#[derive(Default)]
pub struct Audit;

impl Command for Audit {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let found = doc.audit();
        if found.is_empty() {
            return Step::Done(Some(Message::new("done.audit_clean")));
        }
        let ids: Vec<u64> = found.iter().map(|(id, _)| *id).collect();
        doc.select(&ids);
        let first = found
            .first()
            .map(|(_, why)| why.clone())
            .unwrap_or_default();
        Step::Done(Some(Message::with(
            "done.audit",
            vec![found.len().to_string(), first],
        )))
    }
}
