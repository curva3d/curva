//! The `DupEdge` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// `DupEdge`: copies picked edges of surfaces and solids as curves.
#[derive(Default)]
pub struct DupEdge;

impl Recipe for DupEdge {
    const NAME: &'static str = "dupedge";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::edges("prompt.dupedge.edges")]
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
            // The exact edge picked: a line, arc or NURBS, not the drawn polyline.
            let curve = body.edge_curve(*edge).map_err(|e| failed(&e))?;
            {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: *id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupedge",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
