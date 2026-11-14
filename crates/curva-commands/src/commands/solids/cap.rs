//! The `Cap` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;

/// `Cap`: closes the flat open borders of surfaces and polysurfaces with flat faces; closed ones
/// become solids.
#[derive(Default)]
pub struct Cap;

impl Recipe for Cap {
    const NAME: &'static str = "cap";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.cap.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            if body.is_solid() {
                continue;
            }
            let capped = body.capped().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Body(capped)));
        }
        made.message = Some(Message::with(
            "done.cap",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}
