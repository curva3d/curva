//! The `GetDocumentUserText` command.

use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `GetDocumentUserText`: shows the text kept with the file under a key; Enter alone lists
/// every key that is not hidden (hidden keys start with a full stop, as in Rhino).
#[derive(Default)]
pub struct GetDocumentUserText;

impl Command for GetDocumentUserText {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let text = &doc.state().document_text;
        match answer {
            Some(Answer::Path(key)) if !key.trim().is_empty() => match text.get(key.trim()) {
                Some(value) => Step::Done(Some(Message::with(
                    "done.getdocumentusertext",
                    vec![format!("{} = {value}", key.trim())],
                ))),
                None => Step::Done(Some(Message::with(
                    "done.getdocumentusertext.missing",
                    vec![key.trim().to_owned()],
                ))),
            },
            Some(Answer::Empty) => {
                let shown: Vec<String> = text
                    .iter()
                    .filter(|(k, _)| !k.starts_with('.'))
                    .map(|(k, v)| format!("{k} = {v}"))
                    .collect();
                if shown.is_empty() {
                    return Step::Done(Some(Message::new("done.getdocumentusertext.none")));
                }
                Step::Done(Some(Message::with(
                    "done.getdocumentusertext",
                    vec![shown.join("; ")],
                )))
            }
            _ => Step::Ask(Request {
                allow_empty: true,
                ..Request::new("prompt.getdocumentusertext.key", Kind::Path)
            }),
        }
    }
}
