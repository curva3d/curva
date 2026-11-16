//! `Make2D`, as in Rhino 8 (spec `make2d.md`; `rhino-reference.md`): the chosen objects drawn as
//! flat curves seen from the last-used view, straight down on the construction plane, or as a
//! four-view drawing (top, front, right and perspective, third or first angle), with the lines
//! other objects cover hidden or left out. The curves are plain, with no link back.

use crate::commands::{bodies_of, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::{Document, Linetype};
use curva_kernel::hidden::{DrawnLine, LineKind, Options, View, hidden_lines};
use curva_kernel::{Curve, Point};

const PROJECTION: &str = "Projection";
const HIDDEN: &str = "HiddenLines";
const TANGENTS: &str = "TangentEdges";
const GROUP: &str = "GroupOutput";
const SOURCE_LAYERS: &str = "MaintainSourceLayers";

/// Where the drawing is seen from.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Projection {
    /// The last-used viewport's view; the drawing's lower left corner goes to the world origin.
    #[default]
    View,
    /// Straight down on the construction plane; the drawing lies on that plane.
    CPlane,
    /// Top, front, right and perspective, in third angle projection (US): the top view above the
    /// front, the right view to its right. The lower left corner goes to the world origin.
    ThirdAngle,
    /// The same in first angle projection (European): the top view below the front, the right
    /// view to its left.
    FirstAngle,
}

impl Projection {
    fn name(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::CPlane => "CPlane",
            Self::ThirdAngle => "ThirdAngle",
            Self::FirstAngle => "FirstAngle",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::View => Self::CPlane,
            Self::CPlane => Self::ThirdAngle,
            Self::ThirdAngle => Self::FirstAngle,
            Self::FirstAngle => Self::View,
        }
    }
}

/// A flat view looking along `forward` through the world origin, so that its drawing measures
/// world coordinates along `right` and `up`.
fn standard_view(forward: Point, right: Point, up: Point) -> View {
    View {
        eye: forward.map(|c| -c * 1e9),
        forward,
        right,
        up,
        parallel: true,
    }
}

/// The four views of a four-view drawing: top, front, right, and Rhino 8's standard Perspective
/// direction, drawn flat.
fn four_views() -> [View; 4] {
    let forward: Point = [-0.433_012_701_892_219_3, 0.75, -0.5];
    let right = {
        let r = [forward[1], -forward[0], 0.0];
        let l = (r[0] * r[0] + r[1] * r[1]).sqrt();
        r.map(|c| c / l)
    };
    [
        standard_view([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        standard_view([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        standard_view([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        standard_view(forward, right, cross(right, forward)),
    ]
}

/// The lowest and highest corners of a view's drawing.
type Extent = ([f64; 2], [f64; 2]);

fn extent(lines: &[DrawnLine]) -> Option<Extent> {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for p in lines.iter().filter_map(|l| l.curve.sample().ok()).flatten() {
        for i in 0..2 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    low[0].is_finite().then_some((low, high))
}

/// How far to move each of the top, front, right and perspective drawings so they sit as a
/// four-view drawing: the top and right views lined up with the front (third angle: above and to
/// the right; first angle: below and to the left), the perspective at the upper right, with gaps
/// of a tenth of the largest view between them, and the whole drawing's lower left corner at the
/// origin. A view with nothing in it stays where it is.
fn four_view_shifts(extents: &[Option<Extent>; 4], first_angle: bool) -> [[f64; 2]; 4] {
    let [top, front, right, perspective] = *extents;
    let size = extents
        .iter()
        .flatten()
        .map(|(lo, hi)| (hi[0] - lo[0]).max(hi[1] - lo[1]))
        .fold(0.0, f64::max);
    let gap = size / 10.0;
    let (f_lo, f_hi) = front.unwrap_or(([0.0; 2], [0.0; 2]));
    let mut shifts = [[0.0; 2]; 4];
    if let Some((lo, hi)) = top {
        shifts[0][1] = if first_angle {
            f_lo[1] - gap - hi[1]
        } else {
            f_hi[1] + gap - lo[1]
        };
    }
    if let Some((lo, hi)) = right {
        shifts[2][0] = if first_angle {
            f_lo[0] - gap - hi[0]
        } else {
            f_hi[0] + gap - lo[0]
        };
    }
    if let Some((lo, _)) = perspective {
        // Beyond the front and the views beside and above it.
        let beside = [top, front, right]
            .iter()
            .zip(&shifts)
            .filter_map(|(e, s)| e.map(|(_, hi)| [hi[0] + s[0], hi[1] + s[1]]))
            .fold([f_hi[0], f_hi[1]], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
        shifts[3] = [beside[0] + gap - lo[0], beside[1] + gap - lo[1]];
    }
    // The whole drawing's lower left corner to the origin.
    let low = extents
        .iter()
        .zip(&shifts)
        .filter_map(|(e, s)| e.map(|(lo, _)| [lo[0] + s[0], lo[1] + s[1]]))
        .fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
    if low[0].is_finite() {
        for s in &mut shifts {
            *s = [s[0] - low[0], s[1] - low[1]];
        }
    }
    shifts
}

/// Where a point of the drawing goes in the model.
type Placer = Box<dyn Fn([f64; 2]) -> Point>;

/// A view's drawn lines, with how far they move on the plane.
type Drawing = (Vec<DrawnLine>, [f64; 2]);

/// The running command.
#[derive(Default)]
pub struct Make2D {
    projection: Projection,
    without_hidden: bool,
    tangent_edges: bool,
    /// Whether the drawing's curves go into one group.
    group: bool,
    /// Whether each curve goes on its source's layer tree, repeated under Make2D's layers.
    source_layers: bool,
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

fn yes(on: bool) -> &'static str {
    if on { "Yes" } else { "No" }
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
            options: vec![PROJECTION, HIDDEN, TANGENTS, GROUP, SOURCE_LAYERS],
            option_labels: vec![
                format!("{PROJECTION}={}", self.projection.name()),
                format!("{HIDDEN}={}", yes(!self.without_hidden)),
                format!("{TANGENTS}={}", yes(self.tangent_edges)),
                format!("{GROUP}={}", yes(self.group)),
                format!("{SOURCE_LAYERS}={}", yes(self.source_layers)),
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
        let (body_ids, bodies): (Vec<u64>, Vec<_>) = bodies_of(doc, ids).into_iter().unzip();
        let (curve_ids, curves): (Vec<u64>, Vec<_>) = curves_of(doc, ids).into_iter().unzip();
        if bodies.is_empty() && curves.is_empty() {
            return Step::Failed(Message::new("error.make2d_nothing"));
        }
        // Each input's layer path, in the order the drawn lines name their sources.
        let source_layers: Vec<String> = body_ids
            .iter()
            .chain(&curve_ids)
            .map(|id| {
                let layer = doc
                    .state()
                    .objects
                    .iter()
                    .find(|o| o.id == *id)
                    .map_or(0, |o| o.layer);
                doc.state().layer_path(layer)
            })
            .collect();
        let options = Options {
            tangent_edges: self.tangent_edges,
        };
        let draw = |view: &View| -> Result<Vec<DrawnLine>, curva_kernel::KernelError> {
            Ok(hidden_lines(&bodies, &curves, view, options)?
                .into_iter()
                .filter(|l| l.visible || !self.without_hidden)
                .collect())
        };
        // Each drawing with how far it moves on the plane, and where the plane goes.
        let (drawings, place): (Vec<Drawing>, Placer) = match self.projection {
            Projection::ThirdAngle | Projection::FirstAngle => {
                let mut sets = Vec::new();
                for view in four_views() {
                    match draw(&view) {
                        Ok(lines) => sets.push(lines),
                        Err(error) => return geometry_failed(&error),
                    }
                }
                let extents: [Option<Extent>; 4] = std::array::from_fn(|i| extent(&sets[i]));
                let shifts = four_view_shifts(&extents, self.projection == Projection::FirstAngle);
                (
                    sets.into_iter().zip(shifts).collect(),
                    Box::new(|p| [p[0], p[1], 0.0]),
                )
            }
            _ => {
                let (view, place) = self.view(doc);
                let lines = match draw(&view) {
                    Ok(lines) => lines,
                    Err(error) => return geometry_failed(&error),
                };
                // From a view, the drawing's lower left corner goes to the world origin, as
                // in Rhino.
                let shift = match extent(&lines) {
                    Some((low, _))
                        if self.projection == Projection::View && doc.view().is_some() =>
                    {
                        [-low[0], -low[1]]
                    }
                    _ => [0.0, 0.0],
                };
                (vec![(lines, shift)], place)
            }
        };
        let mut made: Vec<u64> = Vec::new();
        for (line, shift) in drawings
            .into_iter()
            .flat_map(|(lines, shift)| lines.into_iter().map(move |l| (l, shift)))
        {
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
            // With source layers kept, the source's layer tree is repeated under Make2D's.
            let path = match source_layers.get(line.source) {
                Some(source) if self.source_layers && !source.is_empty() => {
                    format!("{name}::{source}")
                }
                _ => name.to_owned(),
            };
            let layer = layer_named(doc, &path, !line.visible);
            let id = doc.add(curve);
            doc.move_to_layer(&[id], layer);
            made.push(id);
        }
        if self.group && !made.is_empty() {
            doc.group(&made);
        }
        Step::Done(Some(Message::with(
            "done.make2d",
            vec![made.len().to_string()],
        )))
    }
}

impl Command for Make2D {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(PROJECTION)) => {
                self.projection = self.projection.next();
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
            Some(Answer::Option(GROUP)) => {
                self.group = !self.group;
                self.ask()
            }
            Some(Answer::Option(SOURCE_LAYERS)) => {
                self.source_layers = !self.source_layers;
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

    #[test]
    fn a_four_view_drawing_lines_its_views_up_as_its_projection_says() {
        use super::four_view_shifts;
        // Top 10 × 20, front 10 × 30, right 20 × 30, perspective 25 × 25, each as drawn; the
        // largest is 30 across, so the gaps are 3.
        let extents = [
            Some(([0.0, 0.0], [10.0, 20.0])),
            Some(([0.0, 0.0], [10.0, 30.0])),
            Some(([0.0, 0.0], [20.0, 30.0])),
            Some(([-5.0, -5.0], [20.0, 20.0])),
        ];
        // Third angle: the front at the lower left, the top above it, the right to its right,
        // the perspective beyond both.
        let third = four_view_shifts(&extents, false);
        assert_eq!(third[1], [0.0, 0.0]);
        assert_eq!(third[0], [0.0, 33.0]);
        assert_eq!(third[2], [13.0, 0.0]);
        assert_eq!(third[3], [41.0, 61.0]);
        // First angle: the top below the front, the right to its left; the lower left corner
        // (the right view's, and the top's) at the origin.
        let first = four_view_shifts(&extents, true);
        let front = first[1];
        assert_eq!(first[0], [front[0], front[1] - 23.0]);
        assert_eq!(first[2], [front[0] - 23.0, front[1]]);
        assert_eq!(first[2][0], 0.0);
        assert_eq!(first[0][1], 0.0);
        assert!(first[3][0] > front[0] + 10.0 && first[3][1] > front[1] + 30.0);
    }

    #[test]
    fn make2d_draws_a_four_view_drawing_of_a_box() {
        // A box 10 wide (X), 20 deep (Y) and 30 high (Z).
        let mut doc = Document::default();
        let block = doc.add_body(Body::cuboid([0.0; 3], [10.0, 20.0, 30.0]).unwrap());
        doc.select(&[block]);
        let _ = run(
            &mut doc,
            &["Make2D", "Projection", "Projection", "HiddenLines", ""],
        );
        let curves = drawn(&doc);
        // The top, front and right views are four outline lines each; the perspective more.
        assert!(curves.len() > 12, "{}", curves.len());
        let boxes: Vec<([f64; 3], [f64; 3])> = curves
            .iter()
            .map(|c| {
                let points = c.sample().unwrap();
                let low = points.iter().fold([f64::INFINITY; 3], |m, p| {
                    std::array::from_fn(|i| m[i].min(p[i]))
                });
                let high = points.iter().fold([f64::NEG_INFINITY; 3], |m, p| {
                    std::array::from_fn(|i| m[i].max(p[i]))
                });
                (low, high)
            })
            .collect();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        // The front view's left edge: upright from the origin, the box's height.
        assert!(boxes.iter().any(|(lo, hi)| near(lo[0], 0.0)
            && near(hi[0], 0.0)
            && near(lo[1], 0.0)
            && near(hi[1], 30.0)));
        // The top view above it, the box's depth high and its width wide, lined up with it.
        assert!(boxes.iter().any(|(lo, hi)| near(lo[0], 0.0)
            && near(hi[0], 0.0)
            && lo[1] > 30.0
            && near(hi[1] - lo[1], 20.0)));
        // The right view to its right, the box's depth wide and its height high.
        assert!(boxes.iter().any(|(lo, hi)| lo[0] > 10.0
            && near(hi[0] - lo[0], 20.0)
            && near(lo[1], 0.0)
            && near(hi[1], 0.0)));
        assert!(
            boxes
                .iter()
                .all(|(lo, _)| lo[0] > -1e-9 && lo[1] > -1e-9 && near(lo[2], 0.0))
        );
    }

    #[test]
    fn the_drawing_can_keep_its_sources_layers_and_go_in_one_group() {
        let mut doc = Document::default();
        let wall = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        let door = doc.add_body(Body::cuboid([20.0, 0.0, 0.0], [5.0, 5.0, 5.0]).unwrap());
        let walls = doc.layer_at_path("Walls");
        let frames = doc.layer_at_path("Doors::Frames");
        doc.move_to_layer(&[wall], walls);
        doc.move_to_layer(&[door], frames);
        doc.select(&[wall, door]);
        let _ = run(
            &mut doc,
            &[
                "Make2D",
                "Projection",
                "MaintainSourceLayers",
                "GroupOutput",
                "",
            ],
        );
        // Each box's outline from above, under its own layer tree repeated under Make2D's.
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Curves::Walls"), 4);
        assert_eq!(
            layer_lines(&doc, "Make2D::Visible::Curves::Doors::Frames"),
            4
        );
        assert_eq!(layer_lines(&doc, "Make2D::Visible::Curves"), 0);
        // All eight in one group, which the boxes are not in.
        let groups: Vec<Option<u64>> = doc
            .state()
            .objects
            .iter()
            .filter(|o| o.curve().is_some())
            .map(|o| o.group)
            .collect();
        assert_eq!(groups.len(), 8);
        assert!(groups[0].is_some() && groups.iter().all(|g| *g == groups[0]));
        assert!(
            doc.state()
                .objects
                .iter()
                .filter(|o| o.curve().is_none())
                .all(|o| o.group.is_none())
        );
    }
}
