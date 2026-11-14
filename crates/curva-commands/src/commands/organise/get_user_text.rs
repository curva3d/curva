//! The `GetUserText` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `GetUserText`: shows an object's keys and texts.
#[derive(Default)]
pub struct GetUserText;

impl Command for GetUserText {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        let Some(object) = ids
            .first()
            .and_then(|id| doc.state().objects.iter().find(|o| o.id == *id))
        else {
            return ask_objects("prompt.getusertext.object");
        };
        if object.user_text.is_empty() {
            return Step::Done(Some(Message::new("done.getusertext_none")));
        }
        let shown: Vec<String> = object
            .user_text
            .iter()
            .map(|(k, v)| format!("{k} = {v}"))
            .collect();
        Step::Done(Some(Message::with(
            "done.getusertext",
            vec![shown.join("; ")],
        )))
    }
}
