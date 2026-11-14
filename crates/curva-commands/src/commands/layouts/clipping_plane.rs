//! `ClippingPlane`, `EnableClippingPlane` and `DisableClippingPlane`, as in Rhino 8 (spec
//! `layouts.md` §3; `rhino-reference.md`).

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::Document;
use curva_document::clipping::{ClipView, ClippingPlane as Record, SectionFill};
use curva_kernel::{Curve, Point};

const FLIP: &str = "FlipDirection";

/// `ClippingPlane`: a rectangle by two opposite corners on the construction plane, which cuts
/// away what is behind it in the model's viewports. `FlipDirection` keeps the other side.
#[derive(Default)]
pub struct ClippingPlane {
    first: Option<Point>,
    flipped: bool,
}

impl ClippingPlane {
    fn ask(&self) -> Step {
        let request = match self.first {
            None => Request::new("prompt.clippingplane.corner", Kind::Point),
            Some(a) => Request {
                from: Some(a),
                corner: true,
                preview: vec![a],
                ..Request::new("prompt.clippingplane.other_corner", Kind::Point)
            },
        };
        Step::Ask(Request {
            options: vec![FLIP],
            option_labels: vec![format!(
                "{FLIP}={}",
                if self.flipped { "Yes" } else { "No" }
            )],
            ..request
        })
    }
}

impl Command for ClippingPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.first, answer) {
            (_, None) => self.ask(),
            (_, Some(Answer::Option(FLIP))) => {
                self.flipped = !self.flipped;
                self.ask()
            }
            (None, Some(Answer::Point(a))) => {
                self.first = Some(a);
                self.ask()
            }
            (Some(a), Some(Answer::Point(b))) => {
                // The corners on the construction plane, counter-clockwise seen from its front,
                // so the plane keeps the side the construction plane faces.
                let (origin, [x, y]) = doc.work_frame();
                let along = |p: Point, axis: Point| {
                    (0..3).map(|i| (p[i] - origin[i]) * axis[i]).sum::<f64>()
                };
                let (u0, u1) = (along(a, x).min(along(b, x)), along(a, x).max(along(b, x)));
                let (v0, v1) = (along(a, y).min(along(b, y)), along(a, y).max(along(b, y)));
                if u1 - u0 < 1e-9 || v1 - v0 < 1e-9 {
                    return Step::Failed(Message::new("error.clippingplane_corners"));
                }
                let at = |u: f64, v: f64| std::array::from_fn(|i| origin[i] + x[i] * u + y[i] * v);
                let corners = vec![at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)];
                let Ok(rectangle) = Curve::polyline(corners, true) else {
                    return Step::Failed(Message::new("error.clippingplane_corners"));
                };
                let frame = doc.add(rectangle);
                doc.add_clipping_plane(Record {
                    frame,
                    flipped: self.flipped,
                    views: (0..4).map(ClipView::Viewport).collect(),
                    section: SectionFill::default(),
                });
                Step::Done(Some(Message::new("done.clippingplane")))
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
}
