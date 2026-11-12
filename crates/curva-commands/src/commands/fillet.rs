//! `Fillet`, as in Rhino: rounds the corner between two curves, keeping the sides clicked. A
//! number typed at either prompt sets `Radius`. Rhino's `Join`, `Trim`, `ExtendArcsBy`,
//! `ExtendOtherCurvesBy` and `Dynamic` are still to come (`docs/commands/rhino-reference.md`).

use super::curve_at;
use crate::Message;
use crate::recipe::{Context, Derived, Made, ON_CURVE, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{fillet_curves, gap};

const RADIUS: &str = "Radius";

/// The recipe.
#[derive(Default)]
pub struct Fillet;

impl Recipe for Fillet {
    const NAME: &'static str = "fillet";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::distance(RADIUS, 1.0, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::pick("prompt.fillet.first").number_sets(RADIUS)];
        if !cx.values.is_empty() {
            steps.push(StepDef::pick("prompt.fillet.second").number_sets(RADIUS));
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
        let same = first_id == second_id;
        if same && gap(a, b) <= ON_CURVE {
            return Ok(Made::default());
        }
        let made = fillet_curves(&first, a, &second, b, cx.number(RADIUS), same).map_err(failed)?;
        Ok(Made {
            remove: if same {
                vec![first_id]
            } else {
                vec![first_id, second_id]
            },
            derived: made
                .into_iter()
                .map(|curve| Derived {
                    shape: Shape::Curve(curve),
                    from: first_id,
                    history: None,
                })
                .collect(),
            message: Some(Message::new("done.fillet")),
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
        processor.submit(&mut doc, "fillet");
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
    fn two_clicks_round_the_corner_at_the_current_radius() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 3);
        let expected = 18.0 + std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
    }

    #[test]
    fn a_typed_radius_is_used_and_remembered() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "3");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        let expected = 14.0 + 3.0 * std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
        assert_eq!(doc.remembered("fillet.radius"), Some(3.0));
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
    fn two_segments_of_one_polyline_round_into_one_curve() {
        let mut doc = Document::default();
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        doc.add(Curve::polyline(path, false).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "fillet");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
        let expected = 18.0 + std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
    }

    #[test]
    fn a_line_and_an_arc_round_together() {
        let mut doc = Document::default();
        doc.add(Curve::line([30.0, 0.0, 0.0], [10.0, 0.0, 0.0]).unwrap());
        let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        doc.add(Curve::arc([0.0; 3], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], x, y).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "fillet");
        processor.click(&mut doc, [28.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0 * 0.5_f64.cos(), 10.0 * 0.5_f64.sin(), 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
    }
    #[test]
    fn a_click_on_nothing_keeps_asking() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [50.0, 50.0, 0.0]);
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn a_radius_too_large_changes_nothing() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "50");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!((total(&doc) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn the_preview_shows_the_rounded_corner_before_the_second_click() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        let shown = processor.preview(&doc, Some([10.0, 9.0, 0.0]), "");
        assert_eq!(shown.len(), 3);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
