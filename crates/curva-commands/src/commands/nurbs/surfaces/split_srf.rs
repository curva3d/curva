//! The `SplitSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// `SplitSrf`: cuts a surface with curves lying on it.
/// The pieces `body` splits into along `curves` on one of its faces, each a surface of its own;
/// `None` when no face is split. Shared with `Split`, which splits surfaces too, as Rhino's does.
pub(crate) fn split_pieces(body: &Body, curves: &[Curve]) -> Result<Option<Vec<Body>>, Message> {
    let faces = body.face_count().map_err(|e| failed(&e))?;
    let Some(cut) = (0..faces).find_map(|face| body.split_by_curves(face, curves).ok()) else {
        return Ok(None);
    };
    let count = cut.face_count().map_err(|e| failed(&e))?;
    (0..count)
        .map(|face| cut.faces_only(&[face]).map_err(|e| failed(&e)))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

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
        let Some(pieces) = split_pieces(&body, &curves)? else {
            return Err(Message::new("error.splitsrf_missed"));
        };
        // Each piece its own surface.
        let mut made = Made::default();
        let count = pieces.len();
        for (face, piece) in pieces.into_iter().enumerate() {
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
