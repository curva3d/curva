//! `BooleanIntersection`, as in Rhino: keeps only where two sets of solids overlap. With
//! `DeleteInput` (on, as in Rhino) the originals are replaced by what is left.

use super::bodies_of;
use super::boolean_union::{DELETE_INPUT, failed, result, united};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_kernel::Combine;

/// The recipe.
#[derive(Default)]
pub struct BooleanIntersection;

impl Recipe for BooleanIntersection {
    const NAME: &'static str = "booleanintersection";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DELETE_INPUT, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.booleanintersection.first").bodies()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.booleanintersection.second")
                    .bodies()
                    .apart(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(first)), Some(Value::Objects(second))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let (firsts, seconds) = (bodies_of(cx.doc, first), bodies_of(cx.doc, second));
        let left = match (
            united(&firsts).map_err(|e| failed(&e))?,
            united(&seconds).map_err(|e| failed(&e))?,
        ) {
            (Some(a), Some(b)) => a
                .combined(&b, Combine::Intersection)
                .map_err(|e| failed(&e))?,
            _ => None,
        };
        if left.is_none() {
            return Err(Message::new("error.boolean.empty"));
        }
        let used: Vec<u64> = firsts.iter().chain(&seconds).map(|(id, _)| *id).collect();
        let message = Message::new("done.booleanintersection");
        Ok(result(cx, left, used[0], used, message))
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
