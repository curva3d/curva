//! `Loft`: stretches a skin through curves in the order they were made, as in Rhino. The ends
//! are left open; closing them is a job for `Cap`, which comes later.

use super::{ask_objects, curves_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve};

/// The running command.
#[derive(Default)]
pub struct Loft;

impl Command for Loft {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let curves: Vec<(u64, Curve)> = curves_of(doc, &chosen)
            .into_iter()
            .filter(|(_, curve)| !matches!(curve, Curve::Point { .. }))
            .collect();
        if curves.len() < 2 {
            return ask_objects("prompt.loft.objects");
        }
        let sections: Vec<Curve> = curves.iter().map(|(_, curve)| curve.clone()).collect();
        match Body::lofted(&sections, false) {
            Ok(body) => {
                let layer = doc.layer_of(curves[0].0);
                doc.add_shape_on(Shape::Body(body), layer);
                Step::Done(Some(Message::new("done.loft")))
            }
            Err(error) => geometry_failed(&error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn two_circles_make_a_tube() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        let b = doc.add(Curve::circle([0.0, 0.0, 3.0], 2.0).unwrap());
        doc.select(&[a, b]);
        let mut processor = run(&mut doc, &["loft"]);
        let area = doc.state().objects[2].shape.body().unwrap().area().unwrap();
        assert!((area - std::f64::consts::TAU * 2.0 * 3.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn one_curve_is_not_enough() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[a]);
        let processor = run(&mut doc, &["loft"]);
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
