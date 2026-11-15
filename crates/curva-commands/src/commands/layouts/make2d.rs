//! `Make2D`, as in Rhino 8 (spec `make2d.md`; `rhino-reference.md`): the chosen objects drawn as
//! flat curves seen from the last-used view or straight down on the construction plane, with the
//! lines other objects cover hidden or left out. The curves are plain, with no link back.

use crate::commands::{bodies_of, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::{Document, Linetype};
use curva_kernel::hidden::{DrawnLine, LineKind, View, hidden_lines};
use curva_kernel::{Curve, Point};

const PROJECTION: &str = "Projection";
const HIDDEN: &str = "HiddenLines";

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
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The layer named `name`, made if there is none, with hidden lines dashed.
fn layer_named(doc: &mut Document, name: &str, hidden: bool) -> u64 {
    if let Some(layer) = doc.state().layers.iter().find(|l| l.name == name) {
        return layer.id;
    }
    let id = doc.add_layer(name.to_owned());
    if hidden {
        doc.set_layer_linetype(id, Linetype::Hidden);
    }
    id
}

impl Make2D {
    fn ask(&self) -> Step {
        Step::Ask(Request {
            options: vec![PROJECTION, HIDDEN],
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
        let lines = match hidden_lines(&bodies, &curves, &view) {
            Ok(lines) => lines,
            Err(error) => return geometry_failed(&error),
        };
        let mut lines: Vec<DrawnLine> = lines
            .into_iter()
            .filter(|l| l.visible || !self.without_hidden)
            .collect();
        // From a view, the drawing's lower left corner goes to the world origin, as in Rhino.
        if self.projection == Projection::View && doc.view().is_some() {
            let low = lines
                .iter()
                .flat_map(|l| l.points.iter())
                .fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
            if low.iter().all(|v| v.is_finite()) {
                for line in &mut lines {
                    for p in &mut line.points {
                        *p = [p[0] - low[0], p[1] - low[1]];
                    }
                }
            }
        }
        let mut made = 0;
        for line in lines {
            let points: Vec<Point> = line.points.iter().map(|p| place(*p)).collect();
            let curve = match points.as_slice() {
                [a, b] => Curve::line(*a, *b),
                _ => Curve::polyline(points, false),
            };
            let Ok(curve) = curve else {
                continue;
            };
            let name = match (line.visible, line.kind) {
                (true, LineKind::Curve) => "Make2D Visible Curves",
                (true, LineKind::Silhouette) => "Make2D Visible Silhouettes",
                (false, LineKind::Curve) => "Make2D Hidden Curves",
                (false, LineKind::Silhouette) => "Make2D Hidden Silhouettes",
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
        let Some(layer) = doc.state().layers.iter().find(|l| l.name == name) else {
            return 0;
        };
        doc.state()
            .objects
            .iter()
            .filter(|o| o.layer == layer.id && o.curve().is_some())
            .count()
    }

    #[test]
    fn a_box_seen_from_above_shows_its_top_and_hides_its_bottom() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", "Projection", ""]);
        // The top's four edges show, the bottom's four are hidden under them.
        assert_eq!(layer_lines(&doc, "Make2D Visible Curves"), 4);
        assert_eq!(layer_lines(&doc, "Make2D Hidden Curves"), 4);
        let hidden = doc
            .state()
            .layers
            .iter()
            .find(|l| l.name == "Make2D Hidden Curves")
            .unwrap();
        assert_eq!(hidden.linetype, curva_document::Linetype::Hidden);
        // On the construction plane, flat.
        let flat = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.curve())
            .filter_map(|c| c.sample().ok())
            .flatten()
            .all(|p| p[2].abs() < 1e-9);
        assert!(flat);
    }

    #[test]
    fn a_sphere_seen_at_an_angle_draws_its_outline() {
        let mut doc = Document::default();
        doc.set_view(curva_kernel::hidden::View {
            eye: [100.0, -120.0, 80.0],
            forward: {
                let d: [f64; 3] = [-100.0, 120.0, -80.0];
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                d.map(|c| c / l)
            },
            right: {
                let r: [f64; 3] = [120.0, 100.0, 0.0];
                let l = (r[0] * r[0] + r[1] * r[1]).sqrt();
                r.map(|c| c / l)
            },
            up: [0.0, 0.0, 1.0],
            parallel: false,
        });
        let _ = run(&mut doc, &["Sphere", "0,0,0", "10", "SelAll", "Make2D", ""]);
        assert!(layer_lines(&doc, "Make2D Visible Silhouettes") > 0, "no outline");
    }

    #[test]
    fn hidden_lines_can_be_left_out() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let _ = run(&mut doc, &["Make2D", "Projection", "HiddenLines", ""]);
        assert_eq!(layer_lines(&doc, "Make2D Visible Curves"), 4);
        assert_eq!(layer_lines(&doc, "Make2D Hidden Curves"), 0);
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
