//! The `InterpCrvOnSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value, run_points};
use curva_document::Shape;

/// `InterpCrvOnSrf`: a curve through points on a surface, lying on it.
#[derive(Default)]
pub struct InterpCrvOnSrf;

impl Recipe for InterpCrvOnSrf {
    const NAME: &'static str = "interpcrvonsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.interpcrvonsrf.surface")
                .bodies()
                .one(),
        ];
        if cx.values.is_empty() {
            return steps;
        }
        let clicks = &cx.values[1..];
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point("prompt.interpcrvonsrf.point");
            steps.push(if i >= 2 { step.optional() } else { step });
        }
        if clicks.last() != Some(&Value::Empty) {
            let step = StepDef::point("prompt.interpcrvonsrf.point");
            steps.push(if clicks.len() >= 2 {
                step.optional()
            } else {
                step
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 0) else {
            return Ok(Made::default());
        };
        let points = run_points(cx);
        if points.len() < 2 {
            return Ok(Made::default());
        }
        let (face, _, _) = body.nearest_face(points[0]).map_err(|e| failed(&e))?;
        let curve = body.curve_on_face(face, &points).map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Curve(curve),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.interpcrvonsrf")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn sheet(doc: &mut Document, y0: f64, y1: f64, z: f64) -> u64 {
        doc.add_body(
            Body::ruled(
                &line([0.0, y0, z], [4.0, y0, z]),
                &line([0.0, y1, z], [4.0, y1, z]),
                false,
            )
            .unwrap(),
        )
    }

    fn last_curve(doc: &Document) -> Curve {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.curve().cloned())
            .unwrap()
    }

    #[test]
    fn curves_on_surfaces() {
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 2.0);
        let wire = doc.add(line([1.0, 1.0, 5.0], [3.0, 3.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[wire]);
        processor.submit(&mut doc, "pull");
        doc.select(&[s]);
        processor.submit(&mut doc, "");
        assert!(
            last_curve(&doc)
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        doc.select(&[s]);
        run(&mut doc, &["interpcrvonsrf", "1,1,9", "2,3,9", "3,1,9", ""]);
        assert!(
            last_curve(&doc)
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let base = doc.add(line([1.0, 1.0, 2.0], [3.0, 1.0, 2.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[base]);
        processor.submit(&mut doc, "offsetcrvonsrf");
        doc.select(&[s]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "1");
        processor.submit(&mut doc, "2,4,2");
        let p = last_curve(&doc).sample().unwrap()[0];
        assert!((p[1] - 2.0).abs() < 1e-3, "{p:?}");
    }
}
