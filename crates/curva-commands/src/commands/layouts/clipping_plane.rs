//! `ClippingPlane`, `EnableClippingPlane` and `DisableClippingPlane`, as in Rhino 8 (spec
//! `layouts.md` §3; `rhino-reference.md`).

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::Document;
use curva_document::clipping::{ClipView, ClippingPlane as Record, SectionFill};
use curva_kernel::{Curve, Point, cross, dot, gap, sub, unit};

const FLIP: &str = "FlipDirection";
const VERTICAL: &str = "Vertical";
const THREE_POINT: &str = "3Point";

/// `ClippingPlane`, as in Rhino 8 (#773): a rectangle that cuts away what is behind it in the
/// model's viewports. By default two opposite corners on the construction plane; `Vertical`
/// stands it on a section line drawn in plan, a height tall (Enter: as tall as it is long);
/// `3Point` lays it along an edge, out to a width (Enter: as wide as it is long).
/// `FlipDirection` keeps the other side.
#[derive(Default)]
pub struct ClippingPlane {
    /// `Vertical` or `3Point`, once chosen.
    mode: Option<&'static str>,
    points: Vec<Point>,
    flipped: bool,
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

impl ClippingPlane {
    fn ask(&self) -> Step {
        let last = self.points.last().copied();
        let after = |key: &'static str, corner: bool| match last {
            Some(a) => Request {
                from: Some(a),
                corner,
                preview: vec![a],
                ..Request::new(key, Kind::Point)
            },
            None => Request::new(key, Kind::Point),
        };
        let request = match (self.mode, self.points.len()) {
            (Some(VERTICAL), 0) => Request::new("prompt.clippingplane.edge_start", Kind::Point),
            (Some(VERTICAL), 1) => after("prompt.clippingplane.edge_end", false),
            (Some(VERTICAL), _) => Request {
                allow_empty: true,
                ..Request::new("prompt.clippingplane.height", Kind::Number)
            },
            (Some(THREE_POINT), 0) => Request::new("prompt.clippingplane.first", Kind::Point),
            (Some(THREE_POINT), 1) => after("prompt.clippingplane.second", false),
            (Some(THREE_POINT), _) => Request {
                allow_empty: true,
                ..after("prompt.clippingplane.width", false)
            },
            (_, 0) => Request::new("prompt.clippingplane.corner", Kind::Point),
            _ => after("prompt.clippingplane.other_corner", true),
        };
        let mut options = vec![FLIP];
        if self.mode.is_none() && self.points.is_empty() {
            options.extend([THREE_POINT, VERTICAL]);
        }
        let mut labels = vec![format!(
            "{FLIP}={}",
            if self.flipped { "Yes" } else { "No" }
        )];
        labels.extend(options[1..].iter().map(|o| (*o).to_owned()));
        Step::Ask(Request {
            options,
            option_labels: labels,
            ..request
        })
    }

    /// The clipping plane on the rectangle through `corners`, in order.
    fn made(&self, doc: &mut Document, corners: Vec<Point>) -> Step {
        let Ok(rectangle) = Curve::polyline(corners, true) else {
            return Step::Failed(Message::new("error.clippingplane_corners"));
        };
        let frame = doc.add(rectangle);
        doc.add_clipping_plane(Record {
            frame,
            flipped: self.flipped,
            views: (0..4).map(ClipView::Viewport).collect(),
            section: SectionFill::default(),
            hatch: None,
        });
        Step::Done(Some(Message::new("done.clippingplane")))
    }

    /// The default: two opposite corners on the construction plane, counter-clockwise seen from
    /// its front, so the plane keeps the side the construction plane faces.
    fn by_corners(&self, doc: &mut Document, a: Point, b: Point) -> Step {
        let (origin, [x, y]) = doc.work_frame();
        let along =
            |p: Point, axis: Point| (0..3).map(|i| (p[i] - origin[i]) * axis[i]).sum::<f64>();
        let (u0, u1) = (along(a, x).min(along(b, x)), along(a, x).max(along(b, x)));
        let (v0, v1) = (along(a, y).min(along(b, y)), along(a, y).max(along(b, y)));
        if u1 - u0 < 1e-9 || v1 - v0 < 1e-9 {
            return Step::Failed(Message::new("error.clippingplane_corners"));
        }
        let at = |u: f64, v: f64| std::array::from_fn(|i| origin[i] + x[i] * u + y[i] * v);
        self.made(doc, vec![at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)])
    }

    /// `Vertical`: standing on the edge from `a` to `b`, `height` up the construction plane's Z.
    fn vertical(&self, doc: &mut Document, a: Point, b: Point, height: Option<f64>) -> Step {
        let (_, [x, y]) = doc.work_frame();
        let up = cross(x, y);
        let height = height.unwrap_or_else(|| gap(a, b));
        if gap(a, b) < 1e-9 || height <= 0.0 {
            return Step::Failed(Message::new("error.clippingplane_corners"));
        }
        self.made(doc, vec![a, b, plus(b, up, height), plus(a, up, height)])
    }

    /// `3Point`: along the edge from `a` to `b`, out across the construction plane to the side
    /// of `toward`, as far as it lies; without it, as wide as the edge is long.
    fn three_point(&self, doc: &mut Document, a: Point, b: Point, toward: Option<Point>) -> Step {
        let (_, [x, y]) = doc.work_frame();
        let normal = cross(x, y);
        let Some(along) = unit(sub(b, a)) else {
            return Step::Failed(Message::new("error.clippingplane_corners"));
        };
        let side = cross(normal, along);
        let width = toward.map_or(gap(a, b), |p| dot(sub(p, a), side));
        if width.abs() < 1e-9 {
            return Step::Failed(Message::new("error.clippingplane_corners"));
        }
        let (c, d) = (plus(b, side, width), plus(a, side, width));
        // Counter-clockwise seen from the construction plane's front, as the default is.
        let corners = if width > 0.0 {
            vec![a, b, c, d]
        } else {
            vec![a, d, c, b]
        };
        self.made(doc, corners)
    }
}

impl Command for ClippingPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let points = self.points.clone();
        match (self.mode, points.as_slice(), answer) {
            (_, _, None) => self.ask(),
            (_, _, Some(Answer::Option(FLIP))) => {
                self.flipped = !self.flipped;
                self.ask()
            }
            (None, [], Some(Answer::Option(mode @ (VERTICAL | THREE_POINT)))) => {
                self.mode = Some(mode);
                self.ask()
            }
            (None, [a], Some(Answer::Point(b))) => self.by_corners(doc, *a, b),
            (Some(VERTICAL), [a, b], Some(Answer::Number(h))) => {
                self.vertical(doc, *a, *b, Some(h))
            }
            (Some(VERTICAL), [a, b], Some(Answer::Empty)) => self.vertical(doc, *a, *b, None),
            (Some(THREE_POINT), [a, b], Some(Answer::Point(c))) => {
                self.three_point(doc, *a, *b, Some(c))
            }
            (Some(THREE_POINT), [a, b], Some(Answer::Empty)) => self.three_point(doc, *a, *b, None),
            (_, [] | [_], Some(Answer::Point(p))) => {
                self.points.push(p);
                self.ask()
            }
            _ => unexpected(),
        }
    }
}

/// The clipping planes among `ids`, by their rectangles.
fn planes_among(doc: &Document, ids: &[u64]) -> Vec<u64> {
    ids.iter()
        .copied()
        .filter(|id| doc.clipping_plane_of(*id).is_some())
        .collect()
}

/// `EnableClippingPlane` and `DisableClippingPlane`: picked clipping planes cut, or stop cutting,
/// the active viewport, or the detail being worked in.
#[derive(Default)]
pub struct EnableClippingPlane(bool);

/// `DisableClippingPlane`.
pub struct DisableClippingPlane(EnableClippingPlane);

impl Default for DisableClippingPlane {
    fn default() -> Self {
        Self(EnableClippingPlane(true))
    }
}

impl Command for DisableClippingPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        self.0.step(doc, answer)
    }
}

impl Command for EnableClippingPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let off = self.0;
        match answer {
            None => {
                let picked = planes_among(doc, &doc.selection());
                if picked.is_empty() {
                    Step::Ask(Request::new("prompt.clippingplane.which", Kind::Objects))
                } else {
                    Step::View(ViewRequest::ClipIn(picked, !off))
                }
            }
            Some(Answer::Objects(ids)) => {
                let picked = planes_among(doc, &ids);
                if picked.is_empty() {
                    Step::Failed(Message::new("error.clippingplane_none_picked"))
                } else {
                    Step::View(ViewRequest::ClipIn(picked, !off))
                }
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_document::clipping::ClipView;

    #[test]
    fn a_clipping_plane_lies_on_the_construction_plane_and_cuts_the_viewports() {
        let mut doc = Document::default();
        run(&mut doc, &["ClippingPlane", "0,0", "10,5"]);
        let cuts = doc.planes_cutting(ClipView::Viewport(1));
        assert_eq!(cuts.len(), 1);
        assert!((cuts[0].keep[2] - 1.0).abs() < 1e-9);
        assert_eq!(cuts[0].origin, [5.0, 2.5, 0.0]);
        run(&mut doc, &["ClippingPlane", "FlipDirection", "0,0", "10,5"]);
        let flipped = doc.planes_cutting(ClipView::Viewport(1));
        assert!((flipped[1].keep[2] + 1.0).abs() < 1e-9);
    }

    #[test]
    fn enabling_asks_the_interface_for_the_active_view() {
        let mut doc = Document::default();
        run(&mut doc, &["ClippingPlane", "0,0", "10,5"]);
        let frame = doc.state().objects[0].id;
        doc.select(&[frame]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "DisableClippingPlane");
        assert_eq!(
            processor.take_view_request(),
            Some(crate::ViewRequest::ClipIn(vec![frame], false))
        );
    }

    #[test]
    fn vertical_stands_on_a_section_line_and_3point_lays_along_an_edge_as_in_rhino() {
        // Rhino 8: Vertical from (0,0,0) to (10,0,0), 5 high, stands from z 0 to 5 on the line.
        let mut doc = Document::default();
        run(
            &mut doc,
            &["ClippingPlane", "Vertical", "0,0,0", "10,0,0", "5"],
        );
        let cuts = doc.planes_cutting(ClipView::Viewport(1));
        assert_eq!(cuts[0].origin, [5.0, 0.0, 2.5]);
        assert!(cuts[0].keep[2].abs() < 1e-9 && cuts[0].keep[1].abs() > 0.99);
        // Enter: as tall as it is long.
        run(
            &mut doc,
            &["ClippingPlane", "Vertical", "0,0,0", "10,0,0", ""],
        );
        assert_eq!(
            doc.planes_cutting(ClipView::Viewport(1))[1].origin,
            [5.0, 0.0, 5.0]
        );
        // Rhino 8: 3Point (0,0,0), (10,0,0), width to (0,5,0): corners (0,0) to (10,5).
        run(
            &mut doc,
            &["ClippingPlane", "3Point", "0,0,0", "10,0,0", "0,5,0"],
        );
        let cuts = doc.planes_cutting(ClipView::Viewport(1));
        assert_eq!(cuts[2].origin, [5.0, 2.5, 0.0]);
        assert!(
            (cuts[2].keep[2] - 1.0).abs() < 1e-9,
            "faces as the default does"
        );
        // Enter: as wide as it is long.
        run(
            &mut doc,
            &["ClippingPlane", "3Point", "0,0,0", "10,0,0", ""],
        );
        assert_eq!(
            doc.planes_cutting(ClipView::Viewport(1))[3].origin,
            [5.0, 5.0, 0.0]
        );
    }
}
