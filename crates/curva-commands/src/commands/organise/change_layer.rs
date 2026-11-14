//! The `ChangeLayer` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `ChangeLayer`: moves objects to a layer, by its name.
#[derive(Default)]
pub struct ChangeLayer {
    objects: Vec<u64>,
}

impl Command for ChangeLayer {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.objects.is_empty() => {
                let Some(layer) = doc
                    .layers()
                    .iter()
                    .find(|l| l.name.eq_ignore_ascii_case(name.trim()))
                    .map(|l| l.id)
                else {
                    return Step::Failed(Message::with("error.no_layer", vec![name]));
                };
                doc.move_to_layer(&self.objects, layer);
                Step::Done(Some(Message::with(
                    "done.changelayer",
                    vec![self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.changelayer.objects");
                }
                Step::Ask(Request::new("prompt.changelayer.name", Kind::Path))
            }
        }
    }
}
