//! `Make2D`, as in Rhino 8 (spec `make2d.md`; `rhino-reference.md`): the chosen objects drawn as
//! flat curves seen from the last-used view or straight down on the construction plane, with the
//! lines other objects cover hidden or left out. The curves are plain, with no link back.

use crate::commands::{bodies_of, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::{Document, Linetype};
use curva_kernel::hidden::{DrawnLine, LineKind, Options, View, hidden_lines};
use curva_kernel::{Curve, Point};

const PROJECTION: &str = "Projection";
const HIDDEN: &str = "HiddenLines";
const TANGENTS: &str = "TangentEdges";

/// Where the drawing is seen from.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Projection {
    /// The last-used viewport's view; the drawing's lower left corner goes to the world origin.
    #[default]
    View,
    /// Straight down on the construction plane; the drawing lies on that plane.
    CPlane,
}

/// Where a point of the drawing goes in the model.
type Placer = Box<dyn Fn([f64; 2]) -> Point>;

/// The running command.
#[derive(Default)]
pub struct Make2D {
    projection: Projection,
    without_hidden: bool,
    tangent_edges: bool,
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A drawn curve, from the view plane into the model: moved by `shift` in the plane, then put
/// where `place` puts the plane's points. Lines stay exact lines, other curves are mapped
/// exactly, as `place` is affine.
fn placed(curve: &Curve, shift: [f64; 2], place: &Placer) -> Option<Curve> {
    let at = |p: Point| place([p[0] + shift[0], p[1] + shift[1]]);
    if let Curve::Line { start, end } = curve {
        return Curve::line(at(*start), at(*end)).ok();
    }
    // The affine map `place` stands for, from where it puts the plane's origin and axes.
    let o = at([0.0; 3]);
    let x: Point = std::array::from_fn(|i| at([1.0, 0.0, 0.0])[i] - o[i]);
    let y: Point = std::array::from_fn(|i| at([0.0, 1.0, 0.0])[i] - o[i]);
    curve
        .mapped_affine([
            [x[0], x[1], x[2], 0.0],
            [y[0], y[1], y[2], 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [o[0], o[1], o[2], 1.0],
        ])
        .ok()
}

/// The layer at `path` (`Make2D::Hidden::Curves`), made with its parents if missing; new hidden
/// layers draw dashed.
fn layer_named(doc: &mut Document, path: &str, hidden: bool) -> u64 {
    let existed = doc.state().layer_named(path).is_some();
    let id = doc.layer_at_path(path);
    if hidden && !existed {
        doc.set_layer_linetype(id, Linetype::Hidden);
    }
    id
}

impl Make2D {
    fn ask(&self) -> Step {
        Step::Ask(Request {
            options: vec![PROJECTION, HIDDEN, TANGENTS],
            option_labels: vec![
                format!(
                    "{PROJECTION}={}",
                    match self.projection {
                        Projection::View => "View",
                        Projection::CPlane => "CPlane",
                    }
                ),
                format!(
                    "{HIDDEN}={}",
                    if self.without_hidden { "No" } else { "Yes" }
                ),
                format!(
                    "{TANGENTS}={}",
                    if self.tangent_edges { "Yes" } else { "No" }
                ),
            ],
            ..Request::new("prompt.make2d.objects", Kind::Objects)
        })
    }

    /// The view to draw from, and where a point of the drawing goes in the model.
    fn view(&self, doc: &Document) -> (View, Placer) {
        let (origin, [x, y]) = doc.work_frame();
        let normal = cross(x, y);
        let plan = View {
            eye: std::array::from_fn(|i| origin[i] + normal[i] * 1e9),
            forward: normal.map(|c| -c),
            right: x,
            up: y,
            parallel: true,
        };
        match (self.projection, doc.view()) {
            (Projection::View, Some(view)) => (view, Box::new(|p| [p[0], p[1], 0.0])),
            // Plan views measure from the plane's origin, so the drawing lies on the plane.
            _ => (
                plan,
                Box::new(move |p| std::array::from_fn(|i| origin[i] + x[i] * p[0] + y[i] * p[1])),
            ),
        }
    }

    fn make(&self, doc: &mut Document, ids: &[u64]) -> Step {
        let bodies: Vec<_> = bodies_of(doc, ids).into_iter().map(|(_, b)| b).collect();
        let curves: Vec<_> = curves_of(doc, ids).into_iter().map(|(_, c)| c).collect();
        if bodies.is_empty() && curves.is_empty() {
            return Step::Failed(Message::new("error.make2d_nothing"));
        }
        let (view, place) = self.view(doc);
        let options = Options {
            tangent_edges: self.tangent_edges,
        };
        let lines = match hidden_lines(&bodies, &curves, &view, options) {
            Ok(lines) => lines,
            Err(error) => return geometry_failed(&error),
        };
        let lines: Vec<DrawnLine> = lines
            .into_iter()
            .filter(|l| l.visible || !self.without_hidden)
            .collect();
        // From a view, the drawing's lower left corner goes to the world origin, as in Rhino.
        let mut shift = [0.0, 0.0];
        if self.projection == Projection::View && doc.view().is_some() {
            let low = lines
                .iter()
                .filter_map(|l| l.curve.sample().ok())
                .flatten()
                .fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
            if low.iter().all(|v| v.is_finite()) {
                shift = [-low[0], -low[1]];
            }
        }
        let mut made = 0;
        for line in lines {
            let Some(curve) = placed(&line.curve, shift, &place) else {
                continue;
            };
            let name = match (line.visible, line.kind) {
                (true, LineKind::Curve) => "Make2D::Visible::Curves",
                (true, LineKind::Silhouette) => "Make2D::Visible::Silhouettes",
                (true, LineKind::Tangent) => "Make2D::Visible::Tangents",
                (false, LineKind::Curve) => "Make2D::Hidden::Curves",
                (false, LineKind::Silhouette) => "Make2D::Hidden::Silhouettes",
                (false, LineKind::Tangent) => "Make2D::Hidden::Tangents",
            };
            let layer = layer_named(doc, name, !line.visible);
            let id = doc.add(curve);
            doc.move_to_layer(&[id], layer);
            made += 1;
        }
        Step::Done(Some(Message::with("done.make2d", vec![made.to_string()])))
    }
}

impl Command for Make2D {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(PROJECTION)) => {
                self.projection = match self.projection {
                    Projection::View => Projection::CPlane,
                    Projection::CPlane => Projection::View,
                };
                self.ask()
            }
            Some(Answer::Option(TANGENTS)) => {
                self.tangent_edges = !self.tangent_edges;
                self.ask()
            }
            Some(Answer::Option(HIDDEN)) => {
                self.without_hidden = !self.without_hidden;
                self.ask()
            }
            Some(Answer::Objects(ids)) => self.make(doc, &ids),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Body;

    fn layer_lines(doc: &Document, name: &str) -> usize {
        let Some(layer) = doc.state().layer_named(name) else {
            return 0;
        };
        doc.state()
            .objects
            .iter()
            .filter(|o| o.layer == layer && o.curve().is_some())
            .count()
    }

    /// The curves Make2D made.
    fn drawn(doc: &Document) -> Vec<curva_kernel::Curve> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| o.curve().cloned())
            .collect()
    }

    /// A view looking down at the corner of a box at `[0, 0, 0]`–`[10, 10, 10]`.
    fn at_an_angle() -> curva_kernel::hidden::View {
        let d = [-1.0_f64, 1.2, -0.8];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let forward = d.map(|c| c / l);
        let r = [forward[1], -forward[0], 0.0];
        let rl = (r[0] * r[0] + r[1] * r[1]).sqrt();
        let right = r.map(|c| c / rl);
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        curva_kernel::hidden::View {
            eye: std::array::from_fn(|i| 5.0 - forward[i] * 1000.0),
            forward,
            right,
            up,
            parallel: true,
        }
    }

    #[test]
    fn a_box_seen_from_above_is_four_clean_lines() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", "Projection", ""]);
        // Its outline only: the bottom lies under the top and the upright edges are points.
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Curves"), 4);
        assert_eq!(layer_lines(&doc, "Make2D::Hidden::Curves"), 0);
        let curves = drawn(&doc);
        assert!(
            curves
                .iter()
                .all(|c| matches!(c, curva_kernel::Curve::Line { .. }))
        );
        // Flat on the construction plane, and closed: every end meets another.
        let ends: Vec<[f64; 3]> = curves.iter().flat_map(|c| c.sample().unwrap()).collect();
        assert!(ends.iter().all(|p| p[2].abs() < 1e-12));
        for p in &ends {
            let meeting = ends
                .iter()
                .filter(|q| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-9))
                .count();
            assert_eq!(meeting, 2, "{p:?}");
        }
    }

    #[test]
    fn a_square_is_four_lines() {
        let mut doc = Document::default();
        let square = doc.add(
            curva_kernel::Curve::polyline(
                vec![
                    [0.0; 3],
                    [10.0, 0.0, 0.0],
                    [10.0, 10.0, 0.0],
                    [0.0, 10.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[square]);
        let _ = run(&mut doc, &["Make2D", "Projection", ""]);
        // The square itself is the first object; the drawing is the rest.
        let made: Vec<_> = drawn(&doc).into_iter().skip(1).collect();
        assert_eq!(made.len(), 4);
        assert!(
            made.iter()
                .all(|c| matches!(c, curva_kernel::Curve::Line { .. }))
        );
    }

    #[test]
    fn a_box_at_an_angle_has_its_hidden_edges_dashed() {
        let mut doc = Document::default();
        doc.set_view(at_an_angle());
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", ""]);
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Curves"), 9);
        assert_eq!(layer_lines(&doc, "Make2D::Hidden::Curves"), 3);
        // A tree: Make2D > Hidden > Curves, the hidden lines dashed.
        let hidden = doc.state().layer_named("Make2D::Hidden::Curves").unwrap();
        let hidden = doc.state().layer(hidden).unwrap();
        assert_eq!(hidden.name, "Curves");
        assert_eq!(hidden.linetype, curva_document::Linetype::Hidden);
        let parent = doc.state().layer(hidden.parent.unwrap()).unwrap();
        assert_eq!(parent.name, "Hidden");
    }

    #[test]
    fn hidden_lines_can_be_left_out() {
        let mut doc = Document::default();
        doc.set_view(at_an_angle());
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", "HiddenLines", ""]);
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Curves"), 9);
        assert_eq!(layer_lines(&doc, "Make2D::Hidden::Curves"), 0);
    }

    #[test]
    fn a_sphere_seen_at_an_angle_draws_its_outline() {
        let mut doc = Document::default();
        let eye = [100.0_f64, -120.0, 80.0];
        let l = (eye[0] * eye[0] + eye[1] * eye[1] + eye[2] * eye[2]).sqrt();
        let forward = eye.map(|c| -c / l);
        let r = [forward[1], -forward[0], 0.0];
        let rl = (r[0] * r[0] + r[1] * r[1]).sqrt();
        let right = r.map(|c| c / rl);
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        doc.set_view(curva_kernel::hidden::View {
            eye,
            forward,
            right,
            up,
            parallel: false,
        });
        let _ = run(&mut doc, &["Sphere", "0,0,0", "10", "SelAll", "Make2D", ""]);
        // One clean outline, nothing hidden.
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Silhouettes"), 1);
        assert_eq!(layer_lines(&doc, "Make2D::Hidden::Silhouettes"), 0);
    }

    #[test]
    fn from_the_last_view_the_drawing_starts_at_the_origin() {
        let mut doc = Document::default();
        doc.set_view(curva_kernel::hidden::View {
            eye: [0.0, -1000.0, 0.0],
            forward: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            parallel: true,
        });
        let cube = doc.add_body(Body::cuboid([50.0, 0.0, 20.0], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", ""]);
        let points: Vec<_> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.curve())
            .filter_map(|c| c.sample().ok())
            .flatten()
            .collect();
        let low = points
            .iter()
            .fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        assert!(low[0].abs() < 1e-9 && low[1].abs() < 1e-9, "{low:?}");
    }
}
