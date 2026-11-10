//! `SetDisplayMode`: draws the last-used viewport in wireframe or shaded, as in Rhino.

use super::unexpected;
use crate::{Answer, Command, DisplayMode, Kind, Request, Step, ViewRequest};
use curva_document::Document;

const WIREFRAME: &str = "Wireframe";
const SHADED: &str = "Shaded";

/// The running command.
#[derive(Default)]
pub struct SetDisplayMode;

impl Command for SetDisplayMode {
    fn step(&mut self, _doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request {
                options: vec![WIREFRAME, SHADED],
                ..Request::new("prompt.setdisplaymode.mode", Kind::Choice)
            }),
            Some(Answer::Option(WIREFRAME)) => {
                Step::View(ViewRequest::Display(DisplayMode::Wireframe))
            }
            Some(Answer::Option(SHADED)) => Step::View(ViewRequest::Display(DisplayMode::Shaded)),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::{DisplayMode, ViewRequest};
    use curva_document::Document;

    #[test]
    fn asks_which_mode_and_hands_it_to_the_viewport() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["setdisplaymode", "s"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Display(DisplayMode::Shaded))
        );
        run(&mut doc, &["setdisplaymode", "Wireframe"]);
    }
}
