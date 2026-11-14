//! The `SetUserText` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `SetUserText`: keeps a key and its text on objects (an empty text removes the key).
#[derive(Default)]
pub struct SetUserText {
    objects: Vec<u64>,
    key: Option<String>,
}

impl Command for SetUserText {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) if self.key.is_none() && !self.objects.is_empty() => {
                self.key = Some(text.trim().to_owned());
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.setusertext.value", Kind::Path)
                })
            }
            Some(Answer::Path(_) | Answer::Empty) if self.key.is_some() => {
                let value = match answer {
                    Some(Answer::Path(v)) => v,
                    _ => String::new(),
                };
                let key = self.key.clone().unwrap_or_default();
                doc.set_user_text(&self.objects, &key, &value);
                Step::Done(Some(Message::with(
                    "done.setusertext",
                    vec![key, self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.setusertext.objects");
                }
                Step::Ask(Request::new("prompt.setusertext.key", Kind::Path))
            }
        }
    }
}
