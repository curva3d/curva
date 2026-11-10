//! `PlanarSrf`: fills closed flat curves with a surface, as in Rhino. A curve inside another
//! makes a hole; curves apart from each other make separate surfaces in one go.

use super::{ask_objects, curves_of, geometry_failed, plane_normal};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, nested_groups};

/// The running command.
#[derive(Default)]
pub struct PlanarSrf;

impl Command for PlanarSrf {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let closed: Vec<(u64, curva_kernel::Curve)> = curves_of(doc, &chosen)
            .into_iter()
            .filter(|(_, curve)| curve.is_closed().unwrap_or(false))
            .collect();
        if closed.is_empty() {
            if chosen.is_empty() {
                return ask_objects("prompt.planarsrf.objects");
            }
            return Step::Failed(Message::new("error.planarsrf.none_closed"));
        }
        let curves: Vec<curva_kernel::Curve> =
            closed.iter().map(|(_, curve)| curve.clone()).collect();
        let mut made = Vec::new();
        for group in nested_groups(&curves, plane_normal(doc)) {
            let members: Vec<curva_kernel::Curve> =
                group.iter().map(|&i| curves[i].clone()).collect();
            match Body::planar(&members) {
                Ok(body) => made.push((closed[group[0]].0, body)),
                Err(error) => return geometry_failed(&error),
            }
        }
        let count = made.len();
        for (source, body) in made {
            let layer = doc.layer_of(source);
            doc.add_shape_on(Shape::Body(body), layer);
        }
        Step::Done(Some(Message::with(
            "done.planarsrf",
            vec![count.to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_square_with_a_round_hole_becomes_one_surface() {
        let mut doc = Document::default();
        let square = doc.add(
            Curve::polyline(
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
        let hole = doc.add(Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap());
        doc.select(&[square, hole]);
        let mut processor = run(&mut doc, &["planarsrf"]);
        assert_eq!(doc.state().objects.len(), 3);
        let area = doc.state().objects[2].shape.body().unwrap().area().unwrap();
        assert!((area - (100.0 - std::f64::consts::PI * 4.0)).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn separate_outlines_make_separate_surfaces() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let b = doc.add(Curve::circle([10.0, 0.0, 0.0], 1.0).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["planarsrf"]);
        assert_eq!(doc.state().objects.len(), 4);
    }

    #[test]
    fn open_curves_are_refused() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        let processor = run(&mut doc, &["planarsrf"]);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
