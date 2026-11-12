//! `PlanarSrf`: fills closed flat curves with a surface, as in Rhino. A curve inside another
//! makes a hole; curves apart from each other make separate surfaces in one go.

use super::curves_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, cross, nested_groups};

/// The recipe.
#[derive(Default)]
pub struct PlanarSrf;

impl Recipe for PlanarSrf {
    const NAME: &'static str = "planarsrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.planarsrf.objects").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let closed: Vec<(u64, Curve)> = curves_of(cx.doc, chosen)
            .into_iter()
            .filter(|(_, curve)| curve.is_closed().unwrap_or(false))
            .collect();
        if closed.is_empty() {
            return Err(Message::new("error.planarsrf.none_closed"));
        }
        let curves: Vec<Curve> = closed.iter().map(|(_, curve)| curve.clone()).collect();
        let [x, y] = cx.plane();
        let mut made = Made::default();
        for group in nested_groups(&curves, cross(x, y)) {
            let members: Vec<Curve> = group.iter().map(|&i| curves[i].clone()).collect();
            let body = Body::planar(&members)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: closed[group[0]].0,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.planarsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_square_with_a_round_hole_becomes_one_surface() {
        let mut doc = Document::default();
        let square = doc.add(
            Curve::polyline(
                vec![
                    [0.0; 3],
                    [10.0, 0.0, 0.0],
                    [10.0, 10.0, 0.0],
                    [0.0, 10.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        let hole = doc.add(Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap());
        doc.select(&[square, hole]);
        let mut processor = run(&mut doc, &["planarsrf"]);
        assert_eq!(doc.state().objects.len(), 3);
        let area = doc.state().objects[2].shape.body().unwrap().area().unwrap();
        assert!((area - (100.0 - std::f64::consts::PI * 4.0)).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn separate_outlines_make_separate_surfaces() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let b = doc.add(Curve::circle([10.0, 0.0, 0.0], 1.0).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["planarsrf"]);
        assert_eq!(doc.state().objects.len(), 4);
    }

    #[test]
    fn open_curves_are_refused() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        let processor = run(&mut doc, &["planarsrf"]);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
