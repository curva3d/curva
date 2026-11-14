//! `Loft`: stretches a skin through curves in the order they were made, as in Rhino. The ends
//! are left open. Rhino's seams, `Style`, `Closed`, tangents and `Rebuild` are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// The recipe.
#[derive(Default)]
pub struct Loft;

impl Recipe for Loft {
    const NAME: &'static str = "loft";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.loft.objects").curves().at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let curves: Vec<(u64, Curve)> = curves_of(cx.doc, chosen)
            .into_iter()
            .filter(|(_, curve)| !matches!(curve, Curve::Point { .. }))
            .collect();
        if curves.len() < 2 {
            return Err(Message::new("error.loft.two"));
        }
        let sections: Vec<Curve> = curves.iter().map(|(_, curve)| curve.clone()).collect();
        let body = Body::lofted(&sections, false)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(body),
                from: curves[0].0,
                history: None,
            }],
            message: Some(Message::new("done.loft")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn two_circles_make_a_tube() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        let b = doc.add(Curve::circle([0.0, 0.0, 3.0], 2.0).unwrap());
        doc.select(&[a, b]);
        let mut processor = run(&mut doc, &["loft"]);
        let area = doc.state().objects[2].shape.body().unwrap().area().unwrap();
        assert!((area - std::f64::consts::TAU * 2.0 * 3.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn one_curve_is_not_enough() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[a]);
        let processor = run(&mut doc, &["loft"]);
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
