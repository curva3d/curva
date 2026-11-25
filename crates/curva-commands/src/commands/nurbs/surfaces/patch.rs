//! The `Patch` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// `Patch`: a surface filling a closed chain of curves.
#[derive(Default)]
pub struct Patch;

impl Recipe for Patch {
    const NAME: &'static str = "patch";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.patch.curves").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        // One closed curve is filled through its pieces.
        if curves.len() == 1
            && let Some(pieces) = curves[0].exploded()
        {
            curves = pieces;
        }
        // Three or four sides make one surface through them, as Rhino's Patch gives one surface
        // there (#684); more sides are filled in pieces, for now.
        let one = (3..=4)
            .contains(&curves.len())
            .then(|| Body::from_edges(&curves).ok())
            .flatten();
        let (body, off) = match one {
            Some(body) => (body, 0.0),
            None => Body::patched(&curves).map_err(|e| failed(&e))?,
        };
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::with("done.patch", vec![format!("{off:.4}")])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn four_sides_make_one_surface_as_in_rhino() {
        let mut doc = Document::default();
        let corners = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 3.0],
            [0.0, 10.0, 0.0],
        ];
        let id = doc.add(Curve::polyline(corners, true).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["patch"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert_eq!(body.face_count().unwrap(), 1);
        // Rhino's fitted patch is 102.9385; a surface through the four sides is 102.9399.
        assert!((body.area().unwrap() - 102.9399).abs() < 1e-3);
    }
}
