//! The `UnrollSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `UnrollSrf`: lays developable surfaces flat on the world XY plane.
#[derive(Default)]
pub struct UnrollSrf;

impl Recipe for UnrollSrf {
    const NAME: &'static str = "unrollsrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.unrollsrf.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let flat = body.unrolled().map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(flat),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.unrollsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
