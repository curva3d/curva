//! `ProjectToCPlane`: flattens objects onto the plane being drawn on.

use super::{ask_objects, curves_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, cross, dot, sub, unit};

/// How far apart two heights may be and still count as the same.
const SAME_HEIGHT: f64 = 1e-9;

/// `curve` with every point moved along the unit vector `axis` to the plane through `onto`.
///
/// A curve already parallel to that plane is moved rather than rebuilt, so a circle stays a
/// circle.
pub(super) fn flattened_onto(
    curve: &Curve,
    onto: Point,
    axis: Point,
) -> Result<Curve, KernelError> {
    let heights: Vec<f64> = curve
        .sample()?
        .iter()
        .map(|p| dot(sub(*p, onto), axis))
        .collect();
    let first = heights.first().copied().unwrap_or(0.0);
    if heights.iter().all(|h| (h - first).abs() <= SAME_HEIGHT) {
        return Ok(curve.translated(axis.map(|value| -value * first)));
    }
    curve.stretched(onto, axis, 0.0)
}

/// The running command.
#[derive(Default)]
pub struct ProjectToCPlane;

impl Command for ProjectToCPlane {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.projecttocplane.objects");
        }
        let (origin, [x, y]) = doc.work_frame();
        let Some(normal) = unit(cross(x, y)) else {
            return Step::Failed(Message::new("error.bad_plane"));
        };
        for (id, curve) in curves_of(doc, &chosen) {
            match flattened_onto(&curve, origin, normal) {
                Ok(flat) => doc.replace(id, flat),
                Err(error) => return geometry_failed(&error),
            }
        }
        Step::Done(Some(Message::with(
            "done.projecttocplane",
            vec![chosen.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_sloping_line_lands_on_the_plane() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0, 0.0, 2.0], [3.0, 0.0, 5.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["projecttocplane"]);
        assert_eq!(
            doc.state().objects[0].curve.sample().unwrap(),
            [[0.0; 3], [3.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn a_raised_circle_stays_a_circle() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([1.0, 1.0, 4.0], 2.0).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["projecttocplane"]);
        assert_eq!(
            doc.state().objects[0].curve,
            Curve::circle([1.0, 1.0, 0.0], 2.0).unwrap()
        );
    }

    #[test]
    fn a_line_straight_up_cannot_be_flattened_and_nothing_changes() {
        let mut doc = Document::default();
        let line = Curve::line([0.0; 3], [0.0, 0.0, 3.0]).unwrap();
        let id = doc.add(line.clone());
        doc.select(&[id]);
        let processor = run(&mut doc, &["projecttocplane"]);
        assert_eq!(doc.state().objects[0].curve, line);
        assert_eq!(processor.log().len(), 1);
    }
}
