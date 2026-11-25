//! The `Hyperbola` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, cross, dot, gap, sub, unit};

/// `Hyperbola`: one branch of a hyperbola, exact. As in Rhino, its centre, a focus and a point the
/// branch reaches, the vertex following from them (#707); with `FromVertex`, its centre, the vertex,
/// the focus's distance beyond it, then how far the branch reaches.
/// Rhino's option to give the vertex before the focus.
const FROM_VERTEX: &str = "FromVertex";

#[derive(Default)]
pub struct Hyperbola;

impl Recipe for Hyperbola {
    const NAME: &'static str = "hyperbola";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(FROM_VERTEX)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let first = StepDef::point("prompt.hyperbola.centre").offering(&[FROM_VERTEX]);
        if cx.mode != Some(FROM_VERTEX) {
            let mut steps = vec![first];
            if let Some(c) = point(cx, 0) {
                steps.push(StepDef::point("prompt.hyperbola.focus_point").from(c));
                if point(cx, 1).is_some() {
                    steps.push(StepDef::point("prompt.hyperbola.end").from(c));
                }
            }
            return steps;
        }
        let mut steps = vec![first];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.hyperbola.vertex").from(c));
            if let Some(v) = point(cx, 1) {
                steps.push(StepDef::along("prompt.hyperbola.focus", c, sub(v, c)));
                if number(cx, 2).is_some() {
                    steps.push(StepDef::point("prompt.hyperbola.end").from(v));
                }
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(second)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let axis = unit(sub(second, c)).ok_or_else(|| Message::new("error.reference_point"))?;
        let guide = || Made {
            guides: Curve::line(c, second).ok().into_iter().collect(),
            ..Made::default()
        };
        // From a focus and a point on the branch, as Rhino: the difference of the point's distances
        // to the two foci is twice the distance to the vertex.
        let (a, focal, end) = if cx.mode == Some(FROM_VERTEX) {
            let Some(focal) = number(cx, 2) else {
                return Ok(guide());
            };
            (gap(c, second), focal, point(cx, 3))
        } else {
            let Some(end) = point(cx, 2) else {
                return Ok(guide());
            };
            let focal = gap(c, second);
            let far = plus(c, axis, -focal);
            ((gap(end, far) - gap(end, second)) / 2.0, focal, Some(end))
        };
        if a <= 1e-9 {
            return Err(Message::new("error.hyperbola_focus"));
        }
        if focal.abs() <= a {
            return Err(Message::new("error.hyperbola_focus"));
        }
        let b = (focal * focal - a * a).sqrt();
        let side =
            unit(cross(up(cx), axis)).ok_or_else(|| Message::new("error.reference_point"))?;
        // How far the branch reaches across its axis: at least a little.
        let reach = end
            .map(|end| dot(sub(end, c), side).abs())
            .unwrap_or(b)
            .max(1e-6 * a);
        let x_end = a * (1.0 + reach * reach / (b * b)).sqrt();
        let at = |x: f64, y: f64| plus(plus(c, axis, x), side, y);
        // The end tangents meet on the axis at a² / x; the vertex is the curve's middle, which
        // makes the middle weight x / a.
        let curve = bezier(
            &[at(x_end, -reach), at(a * a / x_end, 0.0), at(x_end, reach)],
            Some(&[1.0, x_end / a, 1.0]),
        )
        .to_curve()
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.hyperbola")),
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
    fn a_hyperbola_is_exact() {
        // a = 2, c = 3: b² = 5; every point satisfies x²/4 − y²/5 = 1.
        let mut doc = Document::default();
        run(
            &mut doc,
            &["hyperbola", "fromvertex", "0,0", "2,0", "3", "5,4"],
        );
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        for i in 0..=20 {
            let p = spline.point(f64::from(i) / 20.0);
            assert!(
                (p[0] * p[0] / 4.0 - p[1] * p[1] / 5.0 - 1.0).abs() < 1e-9,
                "{p:?}"
            );
        }
    }
}
