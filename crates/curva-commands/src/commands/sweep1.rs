//! `Sweep1`: sweeps cross-section curves along one rail, as in Rhino. The rail is chosen first
//! (one curve selected beforehand is taken as it), then the cross sections. Rhino's frame
//! styles, `Closed`, shape blending and the other sweep options are still to come
//! (`docs/commands/rhino-reference.md`).

use super::curves_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// The recipe.
#[derive(Default)]
pub struct Sweep1;

impl Recipe for Sweep1 {
    const NAME: &'static str = "sweep1";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.sweep1.rail").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.sweep1.sections").curves().apart());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(rail)), Some(Value::Objects(chosen))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let Some((rail_id, rail)) = curves_of(cx.doc, rail).into_iter().next() else {
            return Ok(Made::default());
        };
        let sections: Vec<Curve> = curves_of(cx.doc, chosen)
            .into_iter()
            .map(|(_, curve)| curve)
            .collect();
        let body = Body::swept(&sections, &rail, false)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(body),
                from: rail_id,
                history: None,
            }],
            message: Some(Message::new("done.sweep1")),
            ..Made::default()
        })
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
