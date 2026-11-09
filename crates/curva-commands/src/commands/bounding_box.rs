//! `BoundingBox`: reports the size of the box around objects.

use super::{ask_objects, curves_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct BoundingBox;

impl Command for BoundingBox {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.boundingbox.objects");
        }
        let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (_, curve) in curves_of(doc, &chosen) {
            let points = match curve.sample() {
                Ok(points) => points,
                Err(error) => return geometry_failed(&error),
            };
            for point in points {
                for axis in 0..3 {
                    low[axis] = low[axis].min(point[axis]);
                    high[axis] = high[axis].max(point[axis]);
                }
            }
        }
        let size = |axis: usize| format!("{:.3}", high[axis] - low[axis]);
        Step::Done(Some(Message::with(
            "done.boundingbox",
            vec![size(0), size(1), size(2)],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_reports_the_size_and_changes_nothing() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([1.0, 2.0, 0.0], [4.0, 2.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([1.0, 2.0, 0.0], [1.0, 7.0, 2.0]).unwrap());
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["boundingbox"]);
        assert!(processor.log()[0].contains("3.000 x 5.000 x 2.000"));
        assert_eq!(doc.state().objects.len(), 2);
    }
}
