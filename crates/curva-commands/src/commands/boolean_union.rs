//! `BooleanUnion`: joins solids into one, as in Rhino. The originals are replaced by the result.

use super::{ask_objects, bodies_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::Combine;

/// The running command.
#[derive(Default)]
pub struct BooleanUnion;

impl Command for BooleanUnion {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let bodies = bodies_of(doc, &chosen);
        if bodies.len() < 2 {
            return ask_objects("prompt.booleanunion.objects");
        }
        let mut whole = bodies[0].1.clone();
        for (_, body) in &bodies[1..] {
            match whole.combined(body, Combine::Union) {
                Ok(Some(made)) => whole = made,
                Ok(None) => return Step::Failed(Message::new("error.boolean.empty")),
                Err(error) => return geometry_failed(&error),
            }
        }
        let layer = doc.layer_of(bodies[0].0);
        let ids: Vec<u64> = bodies.iter().map(|(id, _)| *id).collect();
        doc.remove(&ids);
        doc.add_shape_on(Shape::Body(whole), layer);
        Step::Done(Some(Message::with(
            "done.booleanunion",
            vec![ids.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn overlapping_cubes_become_one_solid() {
        let mut doc = Document::default();
        let a = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let b = doc.add_body(Body::cuboid([1.0, 0.0, 0.0], [2.0, 2.0, 2.0]).unwrap());
        doc.select(&[a, b]);
        let mut processor = run(&mut doc, &["booleanunion"]);
        assert_eq!(doc.state().objects.len(), 1);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 12.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }
}
