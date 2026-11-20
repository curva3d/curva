//! The `MergeAllFaces` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;

/// `MergeAllFaces`: merges the faces of polysurfaces that lie on one surface and share an edge.
#[derive(Default)]
pub struct MergeAllFaces;

impl Recipe for MergeAllFaces {
    const NAME: &'static str = "mergeallfaces";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.mergeallfaces.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        let mut merged = 0;
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let before = body.face_count().map_err(|e| failed(&e))?;
            let after_body = body.faces_merged().map_err(|e| failed(&e))?;
            let after = after_body.face_count().map_err(|e| failed(&e))?;
            if after < before {
                merged += before - after;
                made.replace.push((id, Shape::Body(after_body)));
            }
        }
        made.message = Some(Message::with(
            "done.mergeallfaces",
            vec![merged.to_string()],
        ));
        Ok(made)
    }
}
