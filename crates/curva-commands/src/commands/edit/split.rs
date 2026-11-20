//! `Split`, as in Rhino: cuts curves where they cross the cutting curves, keeping every piece.
//! Rhino's `Point`, `Isocurve` and `Shrink`, and splitting surfaces, are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::{curves_of, cut_by};
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// The recipe.
#[derive(Default)]
pub struct Split;

impl Recipe for Split {
    const NAME: &'static str = "split";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.split.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.split.cutters").curves());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(targets)), Some(Value::Objects(cutter_ids))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, targets) {
            let others: Vec<u64> = cutter_ids.iter().copied().filter(|c| *c != id).collect();
            let cutters: Vec<_> = curves_of(cx.doc, &others)
                .into_iter()
                .map(|(_, c)| c)
                .collect();
            let pieces = cut_by(&curve, &cutters)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            if pieces.len() >= 2 {
                made.remove.push(id);
                made.derived.extend(pieces.into_iter().map(|piece| Derived {
                    shape: Shape::Curve(piece),
                    from: id,
                    history: None,
                }));
            }
        }
        if made.derived.is_empty() {
            return Err(Message::new("error.nothing_to_cut"));
        }
        made.message = Some(Message::with(
            "done.split",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn crossing_lines() -> (Document, u64, u64) {
        let mut doc = Document::default();
        let target = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let cutter = doc.add(Curve::line([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        (doc, target, cutter)
    }

    fn split(doc: &mut Document, target: u64, cutter: u64) -> Processor {
        let mut processor = Processor::default();
        doc.select(&[target]);
        processor.submit(doc, "split");
        doc.select(&[cutter]);
        processor.submit(doc, "");
        processor
    }

    #[test]
    fn a_curve_is_cut_where_it_crosses_and_the_cutter_stays() {
        let (mut doc, target, cutter) = crossing_lines();
        let processor = split(&mut doc, target, cutter);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 3);
        assert!(doc.state().objects.iter().any(|o| o.id == cutter));
        assert!(doc.state().objects.iter().all(|o| o.id != target));
    }

    #[test]
    fn it_is_one_undo_step() {
        let (mut doc, target, cutter) = crossing_lines();
        let mut processor = split(&mut doc, target, cutter);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
        assert!(doc.state().objects.iter().any(|o| o.id == target));
    }

    #[test]
    fn a_cutter_that_misses_changes_nothing() {
        let mut doc = Document::default();
        let target = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let cutter = doc.add(Curve::line([3.0, 2.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let processor = split(&mut doc, target, cutter);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(!processor.is_running());
    }
}
