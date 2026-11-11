//! `BooleanDifference`: cuts solids away from others, as in Rhino. First the solids to cut
//! from, then the solids to cut with; the cutters are used up.

use super::{ask_objects, bodies_of, geometry_failed, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::Combine;

/// The running command.
#[derive(Default)]
pub struct BooleanDifference {
    from: Option<Vec<u64>>,
}

impl Command for BooleanDifference {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(from) = self.from.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            let solids: Vec<u64> = bodies_of(doc, &chosen).iter().map(|(id, _)| *id).collect();
            if solids.is_empty() {
                return ask_objects("prompt.booleandifference.from");
            }
            self.from = Some(solids);
            doc.select(&[]);
            return ask_objects("prompt.booleandifference.with");
        };
        let Some(Answer::Objects(chosen)) = answer else {
            return unexpected();
        };
        let cutters: Vec<_> = bodies_of(doc, &chosen)
            .into_iter()
            .filter(|(id, _)| !from.contains(id))
            .collect();
        if cutters.is_empty() {
            return ask_objects("prompt.booleandifference.with");
        }
        let mut results = Vec::new();
        for (id, body) in bodies_of(doc, &from) {
            let mut left = Some(body);
            for (_, cutter) in &cutters {
                let Some(current) = left else { break };
                left = match current.combined(cutter, Combine::Difference) {
                    Ok(made) => made,
                    Err(error) => return geometry_failed(&error),
                };
            }
            results.push((id, left));
        }
        let cutter_ids: Vec<u64> = cutters.iter().map(|(id, _)| *id).collect();
        doc.remove(&cutter_ids);
        for (id, left) in results {
            let layer = doc.layer_of(id);
            doc.remove(&[id]);
            // A solid cut away entirely is gone, as in Rhino.
            if let Some(body) = left {
                doc.add_shape_on(Shape::Body(body), layer);
            }
        }
        Step::Done(Some(Message::with(
            "done.booleandifference",
            vec![from.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn a_cube_cut_by_another_loses_the_overlap() {
        let mut doc = Document::default();
        let a = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let b = doc.add_body(Body::cuboid([1.0, 0.0, 0.0], [2.0, 2.0, 2.0]).unwrap());
        let mut processor = Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "booleandifference");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 4.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }
}
