//! The `PrintWidthCommand` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::{Document, PrintWidth};

/// `PrintWidth`: sets objects' print width: millimetres, `ByLayer` or `Default`.
#[derive(Default)]
pub struct PrintWidthCommand {
    objects: Vec<u64>,
}

impl Command for PrintWidthCommand {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) if !self.objects.is_empty() => {
                let text = text.trim().trim_end_matches("mm").trim().to_owned();
                let width = if text.eq_ignore_ascii_case("bylayer") {
                    PrintWidth::ByLayer
                } else if text.eq_ignore_ascii_case("default") {
                    PrintWidth::Default
                } else if let Ok(mm) = text.parse::<f64>() {
                    PrintWidth::Millimetres(mm)
                } else {
                    return Step::Failed(Message::with("error.print_width", vec![text]));
                };
                doc.set_print_width(&self.objects, width);
                Step::Done(Some(Message::with(
                    "done.printwidth",
                    vec![self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.printwidth.objects");
                }
                Step::Ask(Request::new("prompt.printwidth.value", Kind::Path))
            }
        }
    }
}
