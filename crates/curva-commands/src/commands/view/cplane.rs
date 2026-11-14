//! `CPlane`: sets the last-used viewport's construction plane from three points.

use crate::commands::{ask_point, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step, ViewRequest};
use curva_document::Document;
use curva_kernel::{Curve, Point, cross, sub, unit};

const WORLD: &str = "World";

/// The running command.
#[derive(Default)]
pub struct SetCPlane {
    origin: Option<Point>,
    along_x: Option<Point>,
}

impl Command for SetCPlane {
    fn step(&mut self, _doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.origin, self.along_x, answer) {
            (None, None, None) => Step::Ask(Request {
                options: vec![WORLD],
                ..Request::new("prompt.cplane.origin", Kind::Point)
            }),
            (None, None, Some(Answer::Option(WORLD))) => Step::View(ViewRequest::ResetCPlane),
            (None, None, Some(Answer::Point(origin))) => {
                self.origin = Some(origin);
                ask_point("prompt.cplane.x", vec![origin])
            }
            (Some(origin), None, Some(Answer::Point(along))) => {
                self.along_x = Some(along);
                ask_point("prompt.cplane.y", vec![origin])
            }
            (Some(origin), Some(along), Some(Answer::Point(toward))) => {
                let Some(x) = unit(sub(along, origin)) else {
                    return Step::Failed(Message::new("error.bad_plane"));
                };
                let Some(normal) = unit(cross(x, sub(toward, origin))) else {
                    return Step::Failed(Message::new("error.bad_plane"));
                };
                Step::View(ViewRequest::SetCPlane {
                    origin,
                    x,
                    y: cross(normal, x),
                })
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(origin), Some(cursor)) = (self.origin, live.cursor) else {
            return Vec::new();
        };
        let mut shown = guide(origin, cursor);
        if let Some(along) = self.along_x {
            shown.extend(guide(origin, along));
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use crate::ViewRequest;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn three_points_make_a_plane_with_square_axes() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "1,1,1", "4,1,1", "9,7,1"]);
        assert!(!processor.is_running());
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SetCPlane {
                origin: [1.0, 1.0, 1.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
            })
        );
    }

    #[test]
    fn world_puts_the_plane_back() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "world"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::ResetCPlane)
        );
    }

    #[test]
    fn points_in_a_line_make_no_plane() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "0,0", "1,0", "2,0"]);
        assert!(!processor.is_running());
        assert_eq!(processor.take_view_request(), None);
    }
}
