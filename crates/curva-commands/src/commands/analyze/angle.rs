//! The `Angle` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Curve, dot, sub};

/// `Angle`: the angle at a corner, from the corner and a point along each side.
#[derive(Default)]
pub struct Angle;

impl Recipe for Angle {
    const NAME: &'static str = "angle";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.angle.corner")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.angle.first").from(c));
            if cx.values.len() >= 2 {
                steps.push(StepDef::point("prompt.angle.second").from(c));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(a)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let mut guides: Vec<Curve> = Curve::line(c, a).ok().into_iter().collect();
        let Some(b) = point(cx, 2) else {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        };
        guides.extend(Curve::line(c, b).ok());
        let (u, v) = (sub(a, c), sub(b, c));
        let lengths = dot(u, u).sqrt() * dot(v, v).sqrt();
        if lengths < 1e-12 {
            return Err(Message::new("error.reference_point"));
        }
        let degrees = (dot(u, v) / lengths).clamp(-1.0, 1.0).acos().to_degrees();
        Ok(Made {
            guides,
            message: Some(Message::with("done.angle", vec![format!("{degrees:.4}")])),
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
    fn measuring_and_selecting() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["angle", "0,0", "10,0", "0,10"]);
        assert!(processor.log().last().unwrap().contains("90"));
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::circle([5.0, 0.0, 0.0], 1.0).unwrap());
        run(&mut doc, &["selline"]);
        assert_eq!(doc.selection(), vec![line]);
        run(&mut doc, &["selclosedcrv"]);
        assert_eq!(doc.selection().len(), 1);
        assert_ne!(doc.selection(), vec![line]);
    }
}
