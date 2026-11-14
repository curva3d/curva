//! The `ShrinkTrimmedSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;

/// `ShrinkTrimmedSrf`: cuts trimmed surfaces down to the box of their trims.
#[derive(Default)]
pub struct ShrinkTrimmedSrf;

impl Recipe for ShrinkTrimmedSrf {
    const NAME: &'static str = "shrinktrimmedsrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.shrinktrimmedsrf.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            made.replace
                .push((id, Shape::Body(body.shrunk().map_err(|e| failed(&e))?)));
        }
        made.message = Some(Message::with(
            "done.shrinktrimmedsrf",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}
