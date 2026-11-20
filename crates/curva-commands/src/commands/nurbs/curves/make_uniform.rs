//! The `MakeUniform` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Spline};

/// `MakeUniform`: puts curves' knots evenly apart, keeping their control points (the shape
/// changes a little).
#[derive(Default)]
pub struct MakeUniform;

impl Recipe for MakeUniform {
    const NAME: &'static str = "makeuniform";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.makeuniform.objects").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            if matches!(curve, Curve::Line { .. } | Curve::Point { .. }) {
                continue;
            }
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            let curve = spline.uniform().to_curve().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.makeuniform",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}
