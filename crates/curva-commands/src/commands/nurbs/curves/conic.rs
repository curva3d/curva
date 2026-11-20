//! The `Conic` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Curve;

/// `Conic`: a conic section from its start, its end, the apex where its end tangents meet, and
/// its rho (0 to 1: under 0.5 an ellipse's arc, 0.5 a parabola, over 0.5 a hyperbola). Exact,
/// as a weighted quadratic.
#[derive(Default)]
pub struct Conic;

impl Recipe for Conic {
    const NAME: &'static str = "conic";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.conic.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.conic.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::point("prompt.conic.apex").from(b));
                if point(cx, 2).is_some() {
                    steps.push(StepDef::number("prompt.conic.rho").remembered("Rho", 0.5));
                }
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        match number(cx, 3) {
            Some(rho) if !(rho > 0.0 && rho < 1.0) => Err(Message::new("error.conic_rho")),
            _ => Ok(()),
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(apex) = point(cx, 2) else {
            return Ok(Made {
                guides: Curve::line(a, b).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let rho = number(cx, 3).unwrap_or(0.5).clamp(1e-6, 1.0 - 1e-6);
        let weight = rho / (1.0 - rho);
        let curve = bezier(&[a, apex, b], Some(&[1.0, weight, 1.0]))
            .to_curve()
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            guides: [Curve::line(a, apex), Curve::line(apex, b)]
                .into_iter()
                .filter_map(Result::ok)
                .collect(),
            message: Some(Message::new("done.conic")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Spline};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn a_conic_with_rho_a_half_is_a_parabola() {
        let mut doc = Document::default();
        run(&mut doc, &["conic", "-2,1", "2,1", "0,-1", "0.5"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        for i in 0..=10 {
            let p = spline.point(f64::from(i) / 10.0);
            // y = x² / 4 passes through the ends and has its apex tangents meeting at (0, -1).
            assert!((p[1] - p[0] * p[0] / 4.0).abs() < 1e-9, "{p:?}");
        }
        // A rho of 1 is refused.
        let mut doc = Document::default();
        let processor = run(&mut doc, &["conic", "-2,1", "2,1", "0,-1", "1"]);
        assert!(processor.log().iter().any(|line| line.contains("Rho")));
    }
}
