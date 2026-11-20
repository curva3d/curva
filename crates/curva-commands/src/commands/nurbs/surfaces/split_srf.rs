//! The `SplitSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Curve;

/// `SplitSrf`: cuts a surface with curves lying on it.
#[derive(Default)]
pub struct SplitSrf;

impl Recipe for SplitSrf {
    const NAME: &'static str = "splitsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.splitsrf.surface").bodies().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.splitsrf.curves").curves());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 0) else {
            return Ok(Made::default());
        };
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 1))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        let faces = body.face_count().map_err(|e| failed(&e))?;
        let mut cut = body.clone();
        let mut any = false;
        for face in 0..faces {
            if let Ok(next) = cut.split_by_curves(face, &curves) {
                cut = next;
                any = true;
                break;
            }
        }
        if !any {
            return Err(Message::new("error.splitsrf_missed"));
        }
        // Each piece its own surface.
        let mut made = Made::default();
        let count = cut.face_count().map_err(|e| failed(&e))?;
        for face in 0..count {
            let piece = cut.faces_only(&[face]).map_err(|e| failed(&e))?;
            if face == 0 {
                made.replace.push((id, Shape::Body(piece)));
            } else {
                made.derived.push(Derived {
                    shape: Shape::Body(piece),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with("done.splitsrf", vec![count.to_string()]));
        Ok(made)
    }
}
