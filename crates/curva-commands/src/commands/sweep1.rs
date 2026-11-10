//! `Sweep1`: sweeps cross-section curves along one rail, as in Rhino. The rail is chosen first,
//! then the cross sections.

use super::{ask_objects, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve};

/// The running command.
#[derive(Default)]
pub struct Sweep1 {
    rail: Option<(u64, Curve)>,
}

impl Command for Sweep1 {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some((rail_id, rail)) = self.rail.clone() else {
            // One curve selected beforehand is taken as the rail, as in Rhino.
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            return match curves_of(doc, &chosen).as_slice() {
                [only] => {
                    self.rail = Some(only.clone());
                    doc.select(&[]);
                    ask_objects("prompt.sweep1.sections")
                }
                _ => ask_objects("prompt.sweep1.rail"),
            };
        };
        let Some(Answer::Objects(chosen)) = answer else {
            return unexpected();
        };
        let sections: Vec<Curve> = curves_of(doc, &chosen)
            .into_iter()
            .filter(|(id, _)| *id != rail_id)
            .map(|(_, curve)| curve)
            .collect();
        if sections.is_empty() {
            return ask_objects("prompt.sweep1.sections");
        }
        match Body::swept(&sections, &rail, false) {
            Ok(body) => {
                let layer = doc.layer_of(rail_id);
                doc.add_shape_on(Shape::Body(body), layer);
                Step::Done(Some(Message::new("done.sweep1")))
            }
            Err(error) => geometry_failed(&error),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn one_curve_selected_beforehand_is_the_rail() {
        let mut doc = Document::default();
        let section = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let rail = doc.add(Curve::line([0.0; 3], [0.0, 0.0, 6.0]).unwrap());
        doc.select(&[rail]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "sweep1");
        doc.select(&[section]);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn a_circle_swept_along_a_line_makes_a_tube() {
        let mut doc = Document::default();
        let rail = doc.add(Curve::line([0.0; 3], [0.0, 0.0, 6.0]).unwrap());
        let section = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "sweep1");
        doc.select(&[rail]);
        processor.submit(&mut doc, "");
        doc.select(&[section]);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        let area = doc.state().objects[2].shape.body().unwrap().area().unwrap();
        assert!((area - std::f64::consts::TAU * 6.0).abs() < 1e-2);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }
}
