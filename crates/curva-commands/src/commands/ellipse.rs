//! `Ellipse`: an ellipse from its centre, the end of one axis and the end of the other.

use super::{ask_point, geometry_failed, guide, plane_normal, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, cross, dot, flattened, sub, unit};

/// The running command.
#[derive(Default)]
pub struct Ellipse {
    centre: Option<Point>,
    first: Option<Point>,
    normal: Point,
}

impl Ellipse {
    /// The ellipse whose second axis reaches as far as `toward` does, at a right angle.
    fn through(&self, centre: Point, first: Point, toward: Point) -> Result<Curve, KernelError> {
        let a = flattened(sub(first, centre), self.normal);
        let side = unit(cross(self.normal, a))
            .ok_or_else(|| KernelError("the axes need some length".to_owned()))?;
        let reach = dot(sub(toward, centre), side).abs();
        Curve::ellipse(
            centre,
            a,
            [side[0] * reach, side[1] * reach, side[2] * reach],
        )
    }
}

impl Command for Ellipse {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.centre, self.first, answer) {
            (None, None, None) => ask_point("prompt.ellipse.centre", Vec::new()),
            (None, None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                self.normal = plane_normal(doc);
                ask_point("prompt.ellipse.first", vec![centre])
            }
            (Some(centre), None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                ask_point("prompt.ellipse.second", vec![centre])
            }
            (Some(centre), Some(first), Some(Answer::Point(toward))) => {
                match self.through(centre, first, toward) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::new("done.ellipse")))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(centre), Some(cursor)) = (self.centre, live.cursor) else {
            return Vec::new();
        };
        let mut shown = guide(centre, cursor);
        if let Some(first) = self.first {
            shown.extend(self.through(centre, first, cursor));
            shown.extend(guide(centre, first));
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn makes_an_ellipse_with_the_axes_picked() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "0,0", "5,0", "9,3"]);
        for point in doc.state().objects[0].curve().unwrap().sample().unwrap() {
            let value = (point[0] / 5.0).powi(2) + (point[1] / 3.0).powi(2);
            assert!((value - 1.0).abs() < 1e-6, "{point:?}");
        }
    }

    #[test]
    fn it_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["ellipse", "0,0", "5,0", "0,3"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
    }

    #[test]
    fn axes_with_no_length_make_nothing() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "0,0", "0,0", "1,1"]);
        assert!(doc.state().objects.is_empty());
    }
}
