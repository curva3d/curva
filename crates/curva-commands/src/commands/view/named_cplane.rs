//! `NamedCPlane`: saves the last-used viewport's construction plane under a name, puts a viewport
//! on one, or forgets it. Rhino files carry these too (spec `docs/specs/3dm-files.md` §2).

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::Document;

const SAVE: &str = "Save";
const RESTORE: &str = "Restore";
const DELETE: &str = "Delete";

/// The running command.
#[derive(Default)]
pub struct NamedCPlane {
    action: Option<&'static str>,
}

impl Command for NamedCPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(action) = self.action else {
            return match answer {
                None => {
                    let names: Vec<&str> = doc.cplanes().iter().map(|p| p.name.as_str()).collect();
                    let key = if names.is_empty() {
                        "prompt.namedcplane.action_none"
                    } else {
                        "prompt.namedcplane.action"
                    };
                    Step::Ask(Request {
                        prompt: Message::with(key, vec![names.join(", ")]),
                        options: vec![SAVE, RESTORE, DELETE],
                        ..Request::new(key, Kind::Choice)
                    })
                }
                Some(Answer::Option(word)) => {
                    self.action = Some(word);
                    Step::Ask(Request::new("prompt.namedcplane.name", Kind::Path))
                }
                _ => unexpected(),
            };
        };
        let Some(Answer::Path(name)) = answer else {
            return unexpected();
        };
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Step::Failed(Message::new("error.no_view_name"));
        }
        match action {
            SAVE => Step::View(ViewRequest::SaveCPlane(name)),
            RESTORE => match doc.saved_cplane(&name) {
                Some(plane) => Step::View(ViewRequest::SetCPlane {
                    origin: plane.origin,
                    x: plane.x,
                    y: plane.y,
                }),
                None => Step::Failed(Message::with("error.no_cplane", vec![name])),
            },
            DELETE if doc.delete_cplane(&name) => {
                Step::Done(Some(Message::with("done.namedcplane.delete", vec![name])))
            }
            _ => Step::Failed(Message::with("error.no_cplane", vec![name])),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::{Document, SavedCPlane};

    fn with_plane(name: &str) -> Document {
        let mut doc = Document::default();
        doc.save_cplane(SavedCPlane {
            name: name.to_owned(),
            origin: [0.0, 0.0, 3.0],
            x: [0.0, 1.0, 0.0],
            y: [-1.0, 0.0, 0.0],
        });
        doc
    }

    #[test]
    fn save_asks_the_viewport_for_its_plane() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["namedcplane", "Save", "Level 2"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SaveCPlane("Level 2".to_owned()))
        );
    }

    #[test]
    fn restore_puts_the_viewport_on_the_saved_plane() {
        let mut doc = with_plane("Level 2");
        let mut processor = run(&mut doc, &["namedcplane", "Restore", "level 2"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SetCPlane {
                origin: [0.0, 0.0, 3.0],
                x: [0.0, 1.0, 0.0],
                y: [-1.0, 0.0, 0.0],
            })
        );
    }

    #[test]
    fn delete_forgets_it_and_an_unknown_name_fails() {
        let mut doc = with_plane("Level 2");
        run(&mut doc, &["namedcplane", "Delete", "Level 2"]);
        assert!(doc.cplanes().is_empty());
        let mut processor = run(&mut doc, &["namedcplane", "Restore", "Level 2"]);
        assert_eq!(processor.take_view_request(), None);
    }
}
