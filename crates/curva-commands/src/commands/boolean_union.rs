//! `BooleanUnion`, as in Rhino: joins solids into one. With `DeleteInput` (on, as in Rhino) the
//! originals are replaced by the result. Rhino's `MergeCoplanarFaces` is still to come
//! (`docs/commands/rhino-reference.md`).

use super::bodies_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Combine, KernelError};

pub(super) const DELETE_INPUT: &str = "DeleteInput";

/// The recipe.
#[derive(Default)]
pub struct BooleanUnion;

/// The solids joined into one, or `None` if nothing is left.
pub(super) fn united(bodies: &[(u64, Body)]) -> Result<Option<Body>, KernelError> {
    let Some((_, first)) = bodies.first() else {
        return Ok(None);
    };
    let mut whole = first.clone();
    for (_, body) in &bodies[1..] {
        match whole.combined(body, Combine::Union)? {
            Some(made) => whole = made,
            None => return Ok(None),
        }
    }
    Ok(Some(whole))
}

/// The result of a Boolean command: `body` on the layer of `from`, with the inputs `used` taken
/// away when `DeleteInput` is on.
pub(super) fn result(
    cx: &Context<'_>,
    body: Option<Body>,
    from: u64,
    used: Vec<u64>,
    message: Message,
) -> Made {
    Made {
        remove: if cx.toggle(DELETE_INPUT) {
            used
        } else {
            Vec::new()
        },
        derived: body
            .map(|body| Derived {
                shape: Shape::Body(body),
                from,
                history: None,
            })
            .into_iter()
            .collect(),
        message: Some(message),
        ..Made::default()
    }
}

/// An engine failure as a message.
pub(super) fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

impl Recipe for BooleanUnion {
    const NAME: &'static str = "booleanunion";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DELETE_INPUT, true)];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.booleanunion.objects")
                .bodies()
                .at_least(2),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let bodies = bodies_of(cx.doc, chosen);
        let Some(whole) = united(&bodies).map_err(|e| failed(&e))? else {
            return Err(Message::new("error.boolean.empty"));
        };
        let ids: Vec<u64> = bodies.iter().map(|(id, _)| *id).collect();
        let message = Message::with("done.booleanunion", vec![ids.len().to_string()]);
        Ok(result(cx, Some(whole), ids[0], ids, message))
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

    #[test]
    fn delete_input_off_keeps_the_originals() {
        let mut doc = Document::default();
        doc.remember("booleanunion.deleteinput", 0.0);
        let a = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let b = doc.add_body(Body::cuboid([1.0, 0.0, 0.0], [2.0, 2.0, 2.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["booleanunion"]);
        assert_eq!(doc.state().objects.len(), 3);
    }
}
