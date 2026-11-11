//! `BooleanIntersection`: keeps only where two sets of solids overlap, as in Rhino. The
//! originals are replaced by what is left.

use super::{ask_objects, bodies_of, geometry_failed, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Combine, KernelError};

/// The running command.
#[derive(Default)]
pub struct BooleanIntersection {
    first: Option<Vec<u64>>,
}

/// The solids joined into one.
fn united(bodies: &[(u64, Body)]) -> Result<Option<Body>, KernelError> {
    let mut whole = bodies[0].1.clone();
    for (_, body) in &bodies[1..] {
        match whole.combined(body, Combine::Union)? {
            Some(made) => whole = made,
            None => return Ok(None),
        }
    }
    Ok(Some(whole))
}

impl Command for BooleanIntersection {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(first) = self.first.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            let solids: Vec<u64> = bodies_of(doc, &chosen).iter().map(|(id, _)| *id).collect();
            if solids.is_empty() {
                return ask_objects("prompt.booleanintersection.first");
            }
            self.first = Some(solids);
            doc.select(&[]);
            return ask_objects("prompt.booleanintersection.second");
        };
        let Some(Answer::Objects(chosen)) = answer else {
            return unexpected();
        };
        let second: Vec<_> = bodies_of(doc, &chosen)
            .into_iter()
            .filter(|(id, _)| !first.contains(id))
            .collect();
        if second.is_empty() {
            return ask_objects("prompt.booleanintersection.second");
        }
        let firsts = bodies_of(doc, &first);
        let made = match (united(&firsts), united(&second)) {
            (Ok(Some(a)), Ok(Some(b))) => a.combined(&b, Combine::Intersection),
            (Err(error), _) | (_, Err(error)) => return geometry_failed(&error),
            _ => Ok(None),
        };
        let left = match made {
            Ok(left) => left,
            Err(error) => return geometry_failed(&error),
        };
        let Some(body) = left else {
            return Step::Failed(Message::new("error.boolean.empty"));
        };
        let layer = doc.layer_of(firsts[0].0);
        let ids: Vec<u64> = firsts.iter().chain(&second).map(|(id, _)| *id).collect();
        doc.remove(&ids);
        doc.add_shape_on(Shape::Body(body), layer);
        Step::Done(Some(Message::new("done.booleanintersection")))
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    fn start(doc: &mut Document, offset: f64) -> Processor {
        let a = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let b = doc.add_body(Body::cuboid([offset, 0.0, 0.0], [2.0, 2.0, 2.0]).unwrap());
        let mut processor = Processor::default();
        doc.select(&[a]);
        processor.submit(doc, "booleanintersection");
        doc.select(&[b]);
        processor.submit(doc, "");
        processor
    }

    #[test]
    fn only_the_overlap_is_kept() {
        let mut doc = Document::default();
        let mut processor = start(&mut doc, 1.0);
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

    #[test]
    fn solids_apart_are_left_as_they_were() {
        let mut doc = Document::default();
        start(&mut doc, 10.0);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
