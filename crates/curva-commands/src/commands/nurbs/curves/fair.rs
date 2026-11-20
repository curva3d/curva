//! The `Fair` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, gap};

/// The furthest the curve `b` strays from `a`, by points along `b`.
fn deviation(a: &Curve, b: &Curve) -> f64 {
    let Ok(points) = b.divided(64) else {
        return f64::INFINITY;
    };
    points
        .iter()
        .map(|p| a.closest_point(*p).map_or(f64::INFINITY, |q| gap(*p, q)))
        .fold(0.0, f64::max)
}

/// `Fair`: smooths curves' bends, moving their inner control points no further than the
/// tolerance from the shape, their ends fixed.
#[derive(Default)]
pub struct Fair;

// CUSTOM KERNEL OPERATION: curve-fairing
impl Recipe for Fair {
    const NAME: &'static str = "fair";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.fair.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.fair.tolerance").remembered("Tolerance", 0.01));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(tolerance) = number(cx, 1) else {
            return Ok(Made::default());
        };
        if tolerance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let original = match curve {
                Curve::Nurbs { .. } | Curve::Interpolated { .. } | Curve::Polyline { .. } => {
                    curve.clone()
                }
                _ => continue,
            };
            let count = original.control_points().map(|p| p.len()).unwrap_or(0);
            if count < 3 {
                continue;
            }
            let inner: Vec<usize> = (1..count - 1).collect();
            let mut fair = original.clone();
            for _ in 0..200 {
                let Ok(next) = fair.smoothed(&inner, 0.25) else {
                    break;
                };
                if deviation(&original, &next) > tolerance {
                    break;
                }
                fair = next;
            }
            if fair != original {
                made.replace.push((id, Shape::Curve(fair)));
            }
        }
        made.message = Some(Message::with(
            "done.fair",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    fn ends(curve: &Curve) -> (curva_kernel::Point, curva_kernel::Point) {
        let points = curve.sample().unwrap();
        (points[0], points[points.len() - 1])
    }

    #[test]
    fn fair_stays_within_its_tolerance() {
        let mut doc = Document::default();
        let wiggly = Curve::control_point(
            vec![
                [0.0; 3],
                [1.0, 0.3, 0.0],
                [2.0, -0.3, 0.0],
                [3.0, 0.3, 0.0],
                [4.0, -0.3, 0.0],
                [5.0, 0.0, 0.0],
            ],
            3,
            false,
        )
        .unwrap();
        let id = doc.add(wiggly.clone());
        doc.select(&[id]);
        run(&mut doc, &["fair", "0.05"]);
        let fair = only_curve(&doc);
        assert_ne!(fair, wiggly);
        assert!(super::deviation(&wiggly, &fair) <= 0.05 + 1e-9);
        let (start, end) = ends(&fair);
        assert!(gap(start, [0.0; 3]) < 1e-9 && gap(end, [5.0, 0.0, 0.0]) < 1e-9);
    }
}
