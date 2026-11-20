//! The `Patch` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// `Patch`: a surface filling a closed chain of curves.
#[derive(Default)]
pub struct Patch;

impl Recipe for Patch {
    const NAME: &'static str = "patch";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.patch.curves").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        // One closed curve is filled through its pieces.
        if curves.len() == 1
            && let Some(pieces) = curves[0].exploded()
        {
            curves = pieces;
        }
        let (body, off) = Body::patched(&curves).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::with("done.patch", vec![format!("{off:.4}")])),
            ..Made::default()
        })
    }
}
