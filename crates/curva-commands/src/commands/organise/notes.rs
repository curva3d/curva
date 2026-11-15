//! The `Notes` command.

use super::set_document_user_text::typed_text;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `Notes`: the model's notes, kept with the file and read by the `Notes` field. Rhino edits them
/// in a panel; here they are typed at the prompt (`\n` for a new line, `""` to clear), and
/// Enter alone keeps them.
#[derive(Default)]
pub struct Notes;

impl Command for Notes {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) => {
                let notes = typed_text(Some(Answer::Path(text)));
                doc.set_notes(&notes);
                Step::Done(Some(Message::new("done.notes")))
            }
            Some(Answer::Empty) => Step::Done(None),
            _ => {
                let now = doc.state().notes.replace('\n', "\\n");
                Step::Ask(Request {
                    prompt: Message::with("prompt.notes", vec![now]),
                    allow_empty: true,
                    ..Request::new("prompt.notes", Kind::Path)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, fields};

    #[test]
    fn notes_are_typed_kept_and_read_by_their_field() {
        let mut doc = Document::default();
        run(&mut doc, &["Notes", "Issued for planning\\nSecond line"]);
        assert_eq!(doc.state().notes, "Issued for planning\nSecond line");
        let shown = fields::resolve(
            "%<Notes()>%",
            doc.state(),
            None,
            &fields::FieldContext::default(),
        );
        assert_eq!(shown, "Issued for planning\nSecond line");
        run(&mut doc, &["Notes", ""]);
        assert_eq!(
            doc.state().notes,
            "Issued for planning\nSecond line",
            "Enter keeps them"
        );
        run(&mut doc, &["Notes", "\"\""]);
        assert!(doc.state().notes.is_empty());
    }
}
