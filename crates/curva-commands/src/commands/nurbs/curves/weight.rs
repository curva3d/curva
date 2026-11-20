//! The `Weight` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Spline};

/// `Weight`: sets the weight of the control points picked (with `PointsOn`): more than 1 pulls
/// the curve towards a point, less lets it go.
#[derive(Default)]
pub struct Weight;

impl Recipe for Weight {
    const NAME: &'static str = "weight";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::number("prompt.weight.value").remembered("Weight", 1.0)]
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.doc.grip_selection().is_empty() {
            return Err(Message::new("error.weight_points"));
        }
        match number(cx, 0) {
            Some(w) if w <= 0.0 => Err(Message::new("error.weight_positive")),
            _ => Ok(()),
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(weight) = number(cx, 0) else {
            return Ok(Made::default());
        };
        let grips = cx.doc.grip_selection();
        let mut owners: Vec<u64> = grips.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, &owners) {
            // Only control-point curves: their grips are their control points, one for one.
            if !matches!(curve, Curve::Nurbs { .. }) || curve.is_closed().unwrap_or(false) {
                continue;
            }
            let mut spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            for (_, index) in grips.iter().filter(|(owner, _)| *owner == id) {
                if let Some(q) = spline.points.get_mut(*index) {
                    let at = [q[0] / q[3], q[1] / q[3], q[2] / q[3]];
                    *q = [at[0] * weight, at[1] * weight, at[2] * weight, weight];
                }
            }
            let curve = spline.to_curve().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.weight",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::{Curve, Spline};

    #[test]
    fn weight_pulls_the_curve_to_the_point() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(vec![[0.0; 3], [5.0, 5.0, 0.0], [10.0, 0.0, 0.0]], 2, false)
                .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["pointson"]);
        doc.select_grips(&[(id, 1)]);
        run(&mut doc, &["weight", "5"]);
        let curve = doc.state().objects[0].curve().unwrap().clone();
        let peak = Spline::of(&curve).unwrap().point(0.5);
        // Unweighted the middle is at y = 2.5; weighted 5 it is pulled to 5 × 5 / 6.
        assert!((peak[1] - 25.0 / 6.0).abs() < 1e-9, "{peak:?}");
        let _ = Shape::Curve(curve);
    }
}
