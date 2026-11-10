//! `Zoom`: changes how close the last-used viewport is: a window, in, out, a target, or a fit.

use super::{ask_point, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Request, Step, ViewRequest};
use curva_document::Document;
use curva_kernel::{Curve, Point};

const EXTENTS: &str = "Extents";
const SELECTED: &str = "Selected";
const IN: &str = "In";
const OUT: &str = "Out";
const TARGET: &str = "Target";

/// How much one step in or out changes the view, as in Rhino.
const STEP: f64 = 2.0;

/// The running command.
#[derive(Default)]
pub struct Zoom {
    corner: Option<Point>,
    targeting: bool,
}

impl Command for Zoom {
    fn step(&mut self, _doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.targeting {
            return match answer {
                Some(Answer::Point(at)) => Step::View(ViewRequest::ZoomTarget(at)),
                _ => unexpected(),
            };
        }
        if let Some(corner) = self.corner {
            return match answer {
                Some(Answer::Point(other)) => Step::View(ViewRequest::ZoomWindow(corner, other)),
                _ => unexpected(),
            };
        }
        match answer {
            None => Step::Ask(Request {
                options: vec![EXTENTS, SELECTED, IN, OUT, TARGET],
                ..Request::new("prompt.zoom.corner", Kind::Point)
            }),
            Some(Answer::Point(corner)) => {
                self.corner = Some(corner);
                ask_point("prompt.zoom.other_corner", vec![corner])
            }
            Some(Answer::Option(EXTENTS)) => Step::View(ViewRequest::ZoomExtents),
            Some(Answer::Option(SELECTED)) => Step::View(ViewRequest::ZoomSelected),
            Some(Answer::Option(IN)) => Step::View(ViewRequest::ZoomBy(STEP)),
            Some(Answer::Option(OUT)) => Step::View(ViewRequest::ZoomBy(1.0 / STEP)),
            Some(Answer::Option(TARGET)) => {
                self.targeting = true;
                ask_point("prompt.zoom.target", Vec::new())
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        match (self.corner, live.cursor) {
            (Some(corner), Some(cursor)) => guide(corner, cursor),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::ViewRequest;
    use curva_document::Document;

    fn asked(steps: &[&str]) -> Option<ViewRequest> {
        let mut doc = Document::default();
        run(&mut doc, steps).take_view_request()
    }

    #[test]
    fn several_view_changes_in_a_row_all_wait_their_turn() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["zoom", "in", "zoom", "out"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ZoomBy(2.0))
        );
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ZoomBy(0.5))
        );
        assert_eq!(processor.take_view_request(), None);
    }

    #[test]
    fn two_corners_make_a_window() {
        assert_eq!(
            asked(&["zoom", "0,0", "10,5"]),
            Some(ViewRequest::ZoomWindow([0.0; 3], [10.0, 5.0, 0.0]))
        );
    }

    #[test]
    fn in_and_out_are_opposite_steps() {
        assert_eq!(asked(&["zoom", "in"]), Some(ViewRequest::ZoomBy(2.0)));
        assert_eq!(asked(&["zoom", "out"]), Some(ViewRequest::ZoomBy(0.5)));
    }

    #[test]
    fn target_extents_and_selected_hand_on_to_the_view() {
        assert_eq!(
            asked(&["zoom", "target", "1,2,3"]),
            Some(ViewRequest::ZoomTarget([1.0, 2.0, 3.0]))
        );
        assert_eq!(asked(&["zoom", "extents"]), Some(ViewRequest::ZoomExtents));
        assert_eq!(
            asked(&["zoom", "selected"]),
            Some(ViewRequest::ZoomSelected)
        );
    }
}
