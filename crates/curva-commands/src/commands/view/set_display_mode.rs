//! `SetDisplayMode`: draws the last-used viewport in wireframe, shaded, ghosted, hidden or
//! technical, as in Rhino, or massing, a white study model.

use crate::commands::unexpected;
use crate::{Answer, Command, DisplayMode, Kind, Request, Step, ViewRequest};
use curva_document::Document;

const WIREFRAME: &str = "Wireframe";
const SHADED: &str = "Shaded";
const GHOSTED: &str = "Ghosted";
const HIDDEN: &str = "Hidden";
const TECHNICAL: &str = "Technical";
const MASSING: &str = "Massing";

/// The running command.
#[derive(Default)]
pub struct SetDisplayMode;

impl Command for SetDisplayMode {
    fn step(&mut self, _doc: &mut Document, answer: Option<Answer>) -> Step {
        let mode = match answer {
            None => {
                return Step::Ask(Request {
                    options: vec![WIREFRAME, SHADED, GHOSTED, HIDDEN, TECHNICAL, MASSING],
                    ..Request::new("prompt.setdisplaymode.mode", Kind::Choice)
                });
            }
            Some(Answer::Option(WIREFRAME)) => DisplayMode::Wireframe,
            Some(Answer::Option(SHADED)) => DisplayMode::Shaded,
            Some(Answer::Option(GHOSTED)) => DisplayMode::Ghosted,
            Some(Answer::Option(HIDDEN)) => DisplayMode::Hidden,
            Some(Answer::Option(TECHNICAL)) => DisplayMode::Technical,
            Some(Answer::Option(MASSING)) => DisplayMode::Massing,
            Some(_) => return unexpected(),
        };
        Step::View(ViewRequest::Display(mode))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
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

    #[test]
    fn ghosted_is_a_choice() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["setdisplaymode", "g"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Display(DisplayMode::Ghosted))
        );
    }

    #[test]
    fn hidden_is_a_choice() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["setdisplaymode", "h"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Display(DisplayMode::Hidden))
        );
    }

    #[test]
    fn technical_is_a_choice() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["setdisplaymode", "t"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Display(DisplayMode::Technical))
        );
    }

    // Massing was first called Clay; the test keeps its first name.
    #[test]
    fn clay_is_a_choice() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["setdisplaymode", "m"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Display(DisplayMode::Massing))
        );
    }
}
