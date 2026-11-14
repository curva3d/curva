//! The `DupBorder` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `DupBorder`: copies the open border of surfaces and polysurfaces as curves, joined where they
/// meet.
#[derive(Default)]
pub struct DupBorder;

impl Recipe for DupBorder {
    const NAME: &'static str = "dupborder";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.dupborder.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let edges = body.naked_edges().map_err(|e| failed(&e))?;
            let curves = edges
                .into_iter()
                .map(|edge| body.edge_curve(edge))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| failed(&e))?;
            let joined = curva_kernel::join_all(&curves).map_err(|e| failed(&e))?;
            for curve in joined {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupborder",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn dup_border_copies_a_surface_outline() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["dupborder"]);
        let border = only_curve(&doc);
        assert!(border.is_closed().unwrap());
        assert!((border.length().unwrap() - 14.0).abs() < 1e-6);
    }
}
