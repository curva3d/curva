//! `CPlane`: sets the last-used viewport's construction plane, as Rhino 8's does: from three
//! points (the default, or `3Point`), back to `World`, raised by an `Elevation`, turned by
//! `Rotate` about its Z, or facing the `View` (#776).

use crate::commands::{ask_point, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step, ViewRequest};
use curva_document::Document;
use curva_kernel::{Curve, Point, cross, sub, unit};

const WORLD: &str = "World";
const ELEVATION: &str = "Elevation";
const ROTATE: &str = "Rotate";
const VIEW: &str = "View";
const THREE_POINT: &str = "3Point";

/// The running command.
#[derive(Default)]
pub struct SetCPlane {
    origin: Option<Point>,
    along_x: Option<Point>,
    /// The option chosen at the first prompt, while it asks for its value.
    asking: Option<&'static str>,
}

/// The plane through `origin` with X along `x` and Y along `y`.
fn plane(origin: Point, x: Point, y: Point) -> Step {
    Step::View(ViewRequest::SetCPlane { origin, x, y })
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|k| p[k] + d[k] * by)
}

impl SetCPlane {
    /// The first prompt's options.
    fn chosen(&mut self, doc: &Document, option: &'static str) -> Step {
        let (origin, _) = doc.work_frame();
        match option {
            WORLD => Step::View(ViewRequest::ResetCPlane),
            VIEW => match doc.view() {
                // Facing the viewer, through the plane's origin, as Rhino's CPlane View.
                Some(view) => plane(origin, view.right, view.up),
                None => Step::Failed(Message::new("error.bad_plane")),
            },
            ELEVATION => {
                self.asking = Some(ELEVATION);
                Step::Ask(Request::new("prompt.cplane.elevation", Kind::Number))
            }
            ROTATE => {
                self.asking = Some(ROTATE);
                Step::Ask(Request::new("prompt.cplane.angle", Kind::Number))
            }
            THREE_POINT => Step::Ask(Request::new("prompt.cplane.origin3", Kind::Point)),
            _ => unexpected(),
        }
    }

    /// The value asked for by `option`.
    fn valued(doc: &Document, option: &'static str, value: f64) -> Step {
        let (origin, [x, y]) = doc.work_frame();
        match option {
            // Moved along its own Z by the elevation.
            ELEVATION => plane(plus(origin, cross(x, y), value), x, y),
            // Turned about its own Z, counter-clockwise seen from above.
            ROTATE => {
                let (s, c) = value.to_radians().sin_cos();
                let turned_x: Point = std::array::from_fn(|k| x[k] * c + y[k] * s);
                let turned_y: Point = std::array::from_fn(|k| y[k] * c - x[k] * s);
                plane(origin, turned_x, turned_y)
            }
            _ => unexpected(),
        }
    }
}

impl Command for SetCPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if let Some(option) = self.asking {
            return match answer {
                Some(Answer::Number(value)) => Self::valued(doc, option, value),
                _ => unexpected(),
            };
        }
        match (self.origin, self.along_x, answer) {
            (None, None, None) => Step::Ask(Request {
                options: vec![ELEVATION, ROTATE, VIEW, WORLD, THREE_POINT],
                ..Request::new("prompt.cplane.origin", Kind::Point)
            }),
            (None, None, Some(Answer::Option(option))) => self.chosen(doc, option),
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
                plane(origin, x, cross(normal, x))
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

    #[test]
    fn elevation_raises_the_plane_and_rotate_turns_it_as_in_rhino() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "Elevation", "3"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SetCPlane {
                origin: [0.0, 0.0, 3.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
            })
        );
        let mut processor = run(&mut doc, &["cplane", "Rotate", "90"]);
        let Some(ViewRequest::SetCPlane { origin, x, y }) = processor.take_view_request() else {
            panic!("no plane");
        };
        let near = |a: [f64; 3], b: [f64; 3]| curva_kernel::gap(a, b) < 1e-12;
        assert!(near(origin, [0.0; 3]) && near(x, [0.0, 1.0, 0.0]) && near(y, [-1.0, 0.0, 0.0]));
    }
}
