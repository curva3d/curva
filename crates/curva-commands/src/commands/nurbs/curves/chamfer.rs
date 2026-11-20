//! `Chamfer`, as in Rhino: cuts the corner between two lines with a straight line. A number typed
//! at either prompt sets `Distances`, used on both lines. Rhino's separate second distance, `Join`,
//! `Trim`, `ExtendArcsBy` and `ExtendOtherCurvesBy` are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::curve_at;
use crate::recipe::{Context, Derived, Made, ON_CURVE, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::chamfer_lines;

const DISTANCES: &str = "Distances";

/// The recipe.
#[derive(Default)]
pub struct Chamfer;

impl Recipe for Chamfer {
    const NAME: &'static str = "chamfer";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::distance(DISTANCES, 1.0, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::pick("prompt.chamfer.first").number_sets(DISTANCES)];
        if !cx.values.is_empty() {
            steps.push(StepDef::pick("prompt.chamfer.second").number_sets(DISTANCES));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (
            cx.values.first().and_then(Value::point),
            cx.values.get(1).and_then(Value::point),
        ) else {
            return Ok(Made::default());
        };
        let failed =
            |e: curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
        let (Some((first_id, first)), Some((second_id, second))) = (
            curve_at(cx.doc, a, ON_CURVE).map_err(failed)?,
            curve_at(cx.doc, b, ON_CURVE).map_err(failed)?,
        ) else {
            return Ok(Made::default());
        };
        if first_id == second_id {
            return Ok(Made::default());
        }
        let distance = cx.number(DISTANCES);
        let done = chamfer_lines(&first, a, &second, b, distance, distance).map_err(failed)?;
        Ok(Made {
            remove: vec![first_id, second_id],
            derived: [done.first, done.second, done.across]
                .into_iter()
                .map(|curve| Derived {
                    shape: Shape::Curve(curve),
                    from: first_id,
                    history: None,
                })
                .collect(),
            message: Some(Message::new("done.chamfer")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn start() -> (Document, Processor) {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "chamfer");
        (doc, processor)
    }

    fn total(doc: &Document) -> f64 {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve().unwrap().length().unwrap())
            .sum()
    }

    #[test]
    fn two_clicks_cut_the_corner_by_the_remembered_distance() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "3");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
        let expected = 14.0 + 18.0_f64.sqrt();
        assert!((total(&doc) - expected).abs() < 1e-6);
        assert_eq!(doc.remembered("chamfer.distances"), Some(3.0));
    }

    #[test]
    fn it_is_one_undo_step() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn the_preview_shows_the_cut_before_the_second_click() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        assert_eq!(processor.preview(&doc, Some([10.0, 9.0, 0.0]), "").len(), 3);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
