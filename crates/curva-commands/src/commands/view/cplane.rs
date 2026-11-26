//! `CPlane`: sets the last-used viewport's construction plane, as Rhino 8's does: from three
//! points (the default, or `3Point`), back to `World`, raised by an `Elevation`, turned by
//! `Rotate` about its Z, facing the `View`, or back and forward through the viewport's planes with
//! `Undo` and `Redo`, on a picked `Surface`, square to a `Curve`, or moved `Through` a point (#776).

use crate::commands::{ask_point, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step, ViewRequest};
use curva_document::Document;
use curva_kernel::{Curve, Point, cross, sub, unit};

const WORLD: &str = "World";
const ELEVATION: &str = "Elevation";
const ROTATE: &str = "Rotate";
const VIEW: &str = "View";
const THREE_POINT: &str = "3Point";
const UNDO: &str = "Undo";
const REDO: &str = "Redo";
const SURFACE: &str = "Surface";
const THROUGH: &str = "Through";
const CURVE: &str = "Curve";

/// The plane square to the curve picked at `at`: its origin there, its normal along the curve,
/// its X the current plane's Z laid flat on it (or its X), so a plane across a level curve
/// stands upright (Rhino's CPlane Curve, #776).
fn across_curve(doc: &Document, at: Point) -> Step {
    let Ok(Some((_, curve))) = crate::commands::curve_at(doc, at, crate::recipe::ON_CURVE) else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let Ok(spline) = curva_kernel::Spline::of(&curve) else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let t = spline.closest(at);
    let (origin, Some(normal)) = (spline.point(t), unit(spline.derivative(t))) else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let (_, [x, y]) = doc.work_frame();
    let flat = |d: Point| {
        let along = curva_kernel::dot(d, normal);
        unit(std::array::from_fn(|k| d[k] - normal[k] * along))
    };
    match flat(cross(x, y)).or_else(|| flat(x)) {
        Some(up) => plane(origin, cross(up, normal), up),
        None => Step::Failed(Message::new("error.bad_plane")),
    }
}

/// The running command.
#[derive(Default)]
pub struct SetCPlane {
    origin: Option<Point>,
    along_x: Option<Point>,
    /// The option chosen at the first prompt, while it asks for its value.
    asking: Option<&'static str>,
    /// For `Surface`: the face picked, while it asks where on it the plane's origin goes.
    face: Option<(u64, usize)>,
}

/// The plane on face `face` of object `id` at the point of it nearest `at`: its normal the
/// face's, its X the current plane's X laid flat on it (Rhino's CPlane Surface, #776).
fn on_surface(doc: &Document, (id, face): (u64, usize), at: Point) -> Step {
    let Some(body) = doc
        .state()
        .objects
        .iter()
        .find(|o| o.id == id)
        .and_then(|o| o.shape.body())
    else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let (Ok((_, _, origin)), Ok(normal)) = (body.nearest_face(at), body.face_normal_near(face, at))
    else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let Some(normal) = unit(normal) else {
        return Step::Failed(Message::new("error.bad_plane"));
    };
    let (_, [x, y]) = doc.work_frame();
    let flat = |d: Point| {
        let along = curva_kernel::dot(d, normal);
        unit(std::array::from_fn(|k| d[k] - normal[k] * along))
    };
    match flat(x).or_else(|| flat(y)) {
        Some(x) => plane(origin, x, cross(normal, x)),
        None => Step::Failed(Message::new("error.bad_plane")),
    }
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
            UNDO => Step::View(ViewRequest::UndoCPlane),
            REDO => Step::View(ViewRequest::RedoCPlane),
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
            SURFACE => {
                self.asking = Some(SURFACE);
                Step::Ask(Request::new("prompt.cplane.surface", Kind::Faces))
            }
            THROUGH => {
                self.asking = Some(THROUGH);
                Step::Ask(Request::new("prompt.cplane.through", Kind::Point))
            }
            CURVE => {
                self.asking = Some(CURVE);
                Step::Ask(Request::new("prompt.cplane.curve", Kind::PickPart))
            }
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
            return match (option, answer) {
                (SURFACE, Some(Answer::Faces(picked))) => match picked.first() {
                    Some(face) => {
                        self.face = Some(*face);
                        ask_point("prompt.cplane.surface_origin", Vec::new())
                    }
                    None => Step::Failed(Message::new("error.bad_plane")),
                },
                (SURFACE, Some(Answer::Point(at))) => match self.face {
                    Some(face) => on_surface(doc, face, at),
                    None => unexpected(),
                },
                // Through: the same plane moved to the point.
                (THROUGH, Some(Answer::Point(at))) => {
                    let (_, [x, y]) = doc.work_frame();
                    plane(at, x, y)
                }
                (CURVE, Some(Answer::Point(at))) => across_curve(doc, at),
                (_, Some(Answer::Number(value))) => Self::valued(doc, option, value),
                _ => unexpected(),
            };
        }
        match (self.origin, self.along_x, answer) {
            (None, None, None) => Step::Ask(Request {
                options: vec![
                    ELEVATION,
                    ROTATE,
                    SURFACE,
                    CURVE,
                    THROUGH,
                    VIEW,
                    WORLD,
                    THREE_POINT,
                    UNDO,
                    REDO,
                ],
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

    #[test]
    fn undo_and_redo_step_through_the_viewports_planes() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "Undo"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::UndoCPlane));
        let mut processor = run(&mut doc, &["cplane", "Redo"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::RedoCPlane));
    }

    #[test]
    fn surface_puts_the_plane_on_a_picked_face_as_in_rhino() {
        let mut doc = Document::default();
        let cube = curva_kernel::Body::cuboid([0.0; 3], [10.0, 10.0, 5.0]).unwrap();
        let top = (0..6)
            .find(|f| {
                cube.face_normal_near(*f, [5.0, 5.0, 5.0])
                    .is_ok_and(|n| n[2] > 0.9)
            })
            .unwrap();
        let id = doc.add_body(cube);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "cplane");
        processor.submit(&mut doc, "Surface");
        doc.select_faces(&[(id, top)]);
        processor.submit(&mut doc, "");
        processor.click(&mut doc, [3.0, 4.0, 5.0]);
        let Some(ViewRequest::SetCPlane { origin, x, y }) = processor.take_view_request() else {
            panic!("no plane");
        };
        let near = |a: [f64; 3], b: [f64; 3]| curva_kernel::gap(a, b) < 1e-9;
        assert!(
            near(origin, [3.0, 4.0, 5.0]) && near(x, [1.0, 0.0, 0.0]) && near(y, [0.0, 1.0, 0.0]),
            "{origin:?} {x:?} {y:?}"
        );
    }

    #[test]
    fn through_moves_the_plane_and_curve_stands_it_across_a_curve() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["cplane", "Through", "3,4,5"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::SetCPlane {
                origin: [3.0, 4.0, 5.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
            })
        );
        doc.add(curva_kernel::Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "cplane");
        processor.submit(&mut doc, "Curve");
        processor.click(&mut doc, [5.0, 0.0, 0.0]);
        let Some(ViewRequest::SetCPlane { origin, x, y }) = processor.take_view_request() else {
            panic!("no plane");
        };
        // Square to the line at its middle, standing upright.
        let near = |a: [f64; 3], b: [f64; 3]| curva_kernel::gap(a, b) < 1e-9;
        assert!(
            near(origin, [5.0, 0.0, 0.0]) && near(x, [0.0, 1.0, 0.0]) && near(y, [0.0, 0.0, 1.0]),
            "{origin:?} {x:?} {y:?}"
        );
    }
}
