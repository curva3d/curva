//! The `SelBlockInstanceNamed` command.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `SelBlockInstanceNamed`: selects the instances of a definition by its name.
#[derive(Default)]
pub struct SelBlockInstanceNamed;

impl Command for SelBlockInstanceNamed {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.selblock.name", Kind::Path)),
            Some(Answer::Path(name)) => {
                let Some(definition) = doc.definition_named(name.trim()).map(|d| d.id) else {
                    return Step::Failed(Message::with("error.no_block", vec![name]));
                };
                let open = doc.selectable();
                let found: Vec<u64> = doc
                    .instances_of(definition)
                    .into_iter()
                    .filter(|id| open.contains(id))
                    .collect();
                doc.select(&found);
                Step::Done(Some(Message::with(
                    "done.select",
                    vec![found.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}
