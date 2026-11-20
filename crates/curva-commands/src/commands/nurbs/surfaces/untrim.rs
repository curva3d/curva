//! The `Untrim` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// `Untrim`: takes a trim away: click a hole's edge to fill it, or an outer edge to bring the
/// face back to its whole surface.
#[derive(Default)]
pub struct Untrim;

impl Recipe for Untrim {
    const NAME: &'static str = "untrim";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::edges("prompt.untrim.edge").one()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Edges(edges)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, edge) in edges {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let whole = body.untrimmed_at(*edge).map_err(|e| failed(&e))?;
            made.replace.push((*id, Shape::Body(whole)));
        }
        made.message = Some(Message::with(
            "done.untrim",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}
