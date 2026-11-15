//! The `SetDocumentUserText` command, and Curva's `SetLayoutUserText` beside it.

use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// A typed value: `""` (two double quotes) stands for nothing, as in Rhino, and `\n` for a new
/// line, as in Curva's other text prompts.
pub(crate) fn typed_text(answer: Option<Answer>) -> String {
    let text = match answer {
        Some(Answer::Path(text)) => text,
        _ => String::new(),
    };
    if text.trim() == "\"\"" {
        return String::new();
    }
    text.replace("\\n", "\n")
}

/// `SetDocumentUserText`: keeps a key and its text with the file, read by the `DocumentText`
/// field (Rhino 8's `commands/setdocumentusertext.htm`). An empty text removes the key.
/// `SetLayoutUserText` does the same on the current layout, for the `LayoutUserText` field;
/// Rhino sets that in the layout's properties.
#[derive(Default)]
pub struct SetDocumentUserText<const LAYOUT: bool> {
    key: Option<String>,
}

impl<const LAYOUT: bool> Command for SetDocumentUserText<LAYOUT> {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let layout = doc.space();
        if LAYOUT && layout.is_none() {
            return Step::Done(Some(Message::new("done.setlayoutusertext.no_layout")));
        }
        match (&self.key, answer) {
            (None, Some(Answer::Path(key))) if !key.trim().is_empty() => {
                self.key = Some(key.trim().to_owned());
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.setdocumentusertext.value", Kind::Path)
                })
            }
            (Some(key), answer) => {
                let key = key.clone();
                let value = typed_text(answer);
                match layout.filter(|_| LAYOUT) {
                    Some(layout) => {
                        doc.set_layout_text(layout, &key, &value);
                    }
                    None => doc.set_document_text(&key, &value),
                }
                let done = if value.is_empty() {
                    "done.setdocumentusertext.removed"
                } else {
                    "done.setdocumentusertext"
                };
                Step::Done(Some(Message::with(done, vec![key])))
            }
            _ => Step::Ask(Request::new("prompt.setdocumentusertext.key", Kind::Path)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn keys_are_kept_with_the_document_and_removed_by_empty_text() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["SetDocumentUserText", "Project", "Harbour Library"],
        );
        assert_eq!(doc.state().document_text["Project"], "Harbour Library");
        run(&mut doc, &["SetDocumentUserText", "Project", "\"\""]);
        assert!(doc.state().document_text.is_empty());
    }

    #[test]
    fn layout_keys_go_on_the_current_layout() {
        let mut doc = Document::default();
        let paper = curva_document::layouts::paper_size("A3").unwrap();
        let sheet = doc.add_layout("Plans".to_owned(), paper, true);
        run(&mut doc, &["SetLayoutUserText", "Revision", "B"]);
        assert!(
            doc.state().layouts[0].user_text.is_empty(),
            "not on a layout yet"
        );
        doc.set_space(Some(sheet));
        run(&mut doc, &["SetLayoutUserText", "Revision", "B"]);
        assert_eq!(doc.state().layouts[0].user_text["Revision"], "B");
    }
}
