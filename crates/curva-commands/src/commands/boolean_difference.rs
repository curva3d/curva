//! `BooleanDifference`, as in Rhino: cuts solids away from others. First the solids to cut from,
//! then the solids to cut with. With `DeleteInput` (on, as in Rhino) the originals and the cutters
//! are used up. Rhino's `DeleteCutters` is still to come (`docs/commands/rhino-reference.md`).

use super::bodies_of;
use super::boolean_union::{DELETE_INPUT, failed};
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Combine;

/// The recipe.
#[derive(Default)]
pub struct BooleanDifference;

impl Recipe for BooleanDifference {
    const NAME: &'static str = "booleandifference";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DELETE_INPUT, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.booleandifference.from").bodies()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.booleandifference.with")
                    .bodies()
                    .apart(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(from)), Some(Value::Objects(with))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let cutters = bodies_of(cx.doc, with);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, from) {
            let mut left = Some(body);
            for (_, cutter) in &cutters {
                let Some(current) = left else { break };
                left = current
                    .combined(cutter, Combine::Difference)
                    .map_err(|e| failed(&e))?;
            }
            // A solid cut away entirely is gone, as in Rhino.
            made.derived.extend(left.map(|body| Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            }));
            made.remove.push(id);
        }
        if cx.toggle(DELETE_INPUT) {
            made.remove.extend(cutters.iter().map(|(id, _)| *id));
        } else {
            made.remove.clear();
        }
        // One solid cut: it and its cutters are kept inside the result, which follows them.
        if cx.toggle(DELETE_INPUT) && made.derived.len() == 1 && from.len() == 1 {
            made.remove.clear();
            made.live = Some((
                curva_document::LiveHow::Difference,
                from.clone(),
                cutters.iter().map(|(id, _)| *id).collect(),
            ));
        }
        made.message = Some(Message::with(
            "done.booleandifference",
            vec![from.len().to_string()],
        ));
        Ok(made)
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
        assert_eq!(doc.state().objects.iter().filter(|o| !o.hidden).count(), 1);
        let volume = doc
            .state()
            .objects
            .iter()
            .find(|o| !o.hidden)
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 4.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.iter().filter(|o| !o.hidden).count(), 2);
    }
}
