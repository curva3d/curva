//! The `DupFaceBorder` command.

use crate::Message;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// `DupFaceBorder`: copies the outlines of picked faces as curves.
#[derive(Default)]
pub struct DupFaceBorder;

impl Recipe for DupFaceBorder {
    const NAME: &'static str = "dupfaceborder";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::faces("prompt.dupfaceborder.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Faces(picked)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, face) in picked {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let fail =
                |e: curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
            let lone = body.faces_only(&[*face]).map_err(fail)?;
            let curves = lone
                .naked_edges()
                .map_err(fail)?
                .into_iter()
                .map(|e| lone.edge_curve(e))
                .collect::<Result<Vec<_>, _>>()
                .map_err(fail)?;
            for curve in curva_kernel::join_all(&curves).map_err(fail)? {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: *id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupfaceborder",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn face_borders_and_text() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap());
        doc.select_faces(&[(cube, 0)]);
        run(&mut doc, &["dupfaceborder", ""]);
        let border = doc.state().objects.last().unwrap().curve().unwrap().clone();
        assert!(border.is_closed().unwrap());
        let mut doc = Document::default();
        run(&mut doc, &["textobject", "Hi", "0,0", "10"]);
        assert!(doc.state().objects.len() >= 2);
        let mut doc = Document::default();
        run(&mut doc, &["textobject", "Solids", "O", "0,0", "10"]);
        assert!(
            doc.state()
                .objects
                .iter()
                .all(|o| o.shape.body().is_some_and(curva_kernel::Body::is_solid))
        );
    }
}
