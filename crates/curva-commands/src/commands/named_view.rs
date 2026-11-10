//! `NamedView`: saves the last-used viewport's view under a name, brings one back, or forgets it.

use super::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::Document;

const SAVE: &str = "Save";
const RESTORE: &str = "Restore";
const DELETE: &str = "Delete";

/// The running command.
#[derive(Default)]
pub struct NamedView {
    action: Option<&'static str>,
}

impl Command for NamedView {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(action) = self.action else {
            return match answer {
                None => {
                    let names: Vec<&str> = doc.views().iter().map(|v| v.name.as_str()).collect();
                    let key = if names.is_empty() {
                        "prompt.namedview.action_none"
                    } else {
                        "prompt.namedview.action"
                    };
                    Step::Ask(Request {
                        prompt: Message::with(key, vec![names.join(", ")]),
                        options: vec![SAVE, RESTORE, DELETE],
                        ..Request::new(key, Kind::Choice)
                    })
                }
                Some(Answer::Option(word)) => {
                    self.action = Some(word);
                    Step::Ask(Request::new("prompt.namedview.name", Kind::Path))
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
            SAVE => Step::View(ViewRequest::SaveView(name)),
            RESTORE if doc.saved_view(&name).is_some() => {
                Step::View(ViewRequest::RestoreView(name))
            }
            DELETE if doc.delete_view(&name) => {
                Step::Done(Some(Message::with("done.namedview.delete", vec![name])))
            }
            _ => Step::Failed(Message::with("error.no_view", vec![name])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::ViewRequest;
    use curva_document::{Document, SavedView};

    fn with_view(name: &str) -> Document {
        let mut doc = Document::default();
        doc.save_view(SavedView {
            name: name.to_owned(),
            kind: "Top".to_owned(),
            target: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            distance: 10.0,
            axes: None,
            parallel: true,
        });
        doc
    }

    #[test]
    fn save_hands_the_name_to_the_view() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["namedview", "save", "Front door"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SaveView("Front door".to_owned()))
        );
    }

    #[test]
    fn only_a_saved_name_can_be_restored() {
        let mut doc = with_view("Plan");
        let mut processor = run(&mut doc, &["namedview", "restore", "plan"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::RestoreView("plan".to_owned()))
        );
        let mut processor = run(&mut doc, &["namedview", "restore", "elsewhere"]);
        assert_eq!(processor.take_view_request(), None);
        assert!(processor.log().last().unwrap().contains("elsewhere"));
    }

    #[test]
    fn delete_forgets_the_view_and_the_prompt_lists_what_is_saved() {
        let mut doc = with_view("Plan");
        let processor = run(&mut doc, &["namedview"]);
        assert!(processor.prompt().unwrap().contains("Plan"));
        run(&mut doc, &["namedview", "delete", "Plan"]);
        assert!(doc.views().is_empty());
    }
}
