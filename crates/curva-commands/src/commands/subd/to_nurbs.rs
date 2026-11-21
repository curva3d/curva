//! Rhino's `ToNURBS` for SubDs (spec `docs/specs/mesh-and-subd.md` §3.3), on `curva-mesh`'s
//! `SubD::to_nurbs`. Measured in Rhino 8's `-ToNURBS` with Enter at every option: the SubD stays,
//! and a closed SubD becomes a closed polysurface (a 4 m cube's cage of one face a side holds
//! 20.963 m³; of 2 × 2 a side, 50.3587 m³). `PackFaces` (on) joins the patches into larger faces as
//! Rhino does: 6 for either cube.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

const DELETE_INPUT: &str = "DeleteInput";
const PACK_FACES: &str = "PackFaces";

/// The recipe.
#[derive(Default)]
pub struct ToNurbs;

impl Recipe for ToNurbs {
    const NAME: &'static str = "tonurbs";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DELETE_INPUT, false),
        OptionDef::toggle(PACK_FACES, true),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.tonurbs.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        let mut made = Made::default();
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
        {
            let Some(subd) = object.shape.subd() else {
                continue;
            };
            let (body, _) = subd
                .to_nurbs(1, cx.toggle(PACK_FACES))
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: object.id,
                history: None,
            });
            if cx.toggle(DELETE_INPUT) {
                made.remove.push(object.id);
            }
        }
        if made.derived.is_empty() {
            return Err(Message::new("error.tonurbs_none"));
        }
        made.message = Some(Message::with(
            "done.tonurbs",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, SubD};

    fn converted(counts: u32) -> Document {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(SubD::subd_box(
            &Frame::world(),
            [4.0; 3],
            [counts; 3],
        )));
        doc.select(&[id]);
        run(&mut doc, &["tonurbs"]);
        doc
    }

    #[test]
    fn a_cube_becomes_a_solid_of_rhinos_volume_and_the_subd_stays() {
        // Rhino 8's volumes for the same two cubes; Curva's are 20.9673 and 50.3594.
        for (counts, rhino) in [(1, 20.963), (2, 50.3587)] {
            let doc = converted(counts);
            let objects = &doc.state().objects;
            assert_eq!(objects.len(), 2, "the SubD stays");
            let body = objects[1].shape.body().expect("a polysurface");
            let volume = body.volume().unwrap();
            assert!((volume / rhino - 1.0).abs() < 0.001, "{counts}: {volume}");
            // PackFaces is on, as in Rhino: either cube comes out as 6 faces.
            assert_eq!(body.face_count().unwrap(), 6, "{counts}");
        }
    }
}
