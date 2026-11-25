//! The `Angle` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Curve, dot, sub};

/// `Angle`, as in Rhino: the angle between two lines, each given by its start and end, so lines
/// that do not meet (two walls in plan) can be measured too (#694).
#[derive(Default)]
pub struct Angle;

impl Recipe for Angle {
    const NAME: &'static str = "angle";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let prompts = [
            "prompt.angle.first_start",
            "prompt.angle.first_end",
            "prompt.angle.second_start",
            "prompt.angle.second_end",
        ];
        let mut steps = Vec::new();
        for (k, prompt) in prompts.into_iter().enumerate() {
            if k > cx.values.len() {
                break;
            }
            let step = StepDef::point(prompt);
            // Each end measures from its line's start.
            let start = (k % 2 == 1).then(|| point(cx, k - 1)).flatten();
            steps.push(match start {
                Some(start) => step.from(start),
                None => step,
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let line = |k: usize| Some((point(cx, k)?, point(cx, k + 1)?));
        let guides: Vec<Curve> = [line(0), line(2)]
            .into_iter()
            .flatten()
            .filter_map(|(a, b)| Curve::line(a, b).ok())
            .collect();
        let (Some((a0, a1)), Some((b0, b1))) = (line(0), line(2)) else {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        };
        let (u, v) = (sub(a1, a0), sub(b1, b0));
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
        let processor = run(&mut doc, &["angle", "0,0", "10,0", "0,0", "0,10"]);
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
