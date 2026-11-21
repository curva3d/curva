//! Rhino's `ToNURBS` for SubDs, meshes and curves (spec `docs/specs/mesh-and-subd.md` §3.3), on
//! `curva-mesh`'s `SubD::to_nurbs`. Measured in Rhino 8's `-ToNURBS` with Enter at every option: the SubD stays,
//! and a closed SubD becomes a closed polysurface (a 4 m cube's cage of one face a side holds
//! 20.963 m³; of 2 × 2 a side, 50.3587 m³). `PackFaces` (on) joins the patches into larger faces as
//! Rhino does: 6 for either cube. Meshes become one surface per face, as MeshToNURB makes
//! them (Rhino 8: a 2 × 2 × 2 box mesh gives a solid of 24 faces), and curves NURBS curves.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Curve;

const DELETE_INPUT: &str = "DeleteInput";
const PACK_FACES: &str = "PackFaces";

/// The recipe.
#[derive(Default)]
pub struct ToNurbs;

/// One object as NURBS, as Rhino's ToNURBS turns it: a SubD's smooth surface, a mesh's faces (as
/// MeshToNURB's defaults, one surface per face), a curve as one NURBS curve. `None` for objects
/// it leaves alone, and for curves the kernel cannot turn into NURBS yet.
fn converted(shape: &Shape, pack: bool) -> Option<Result<Shape, Message>> {
    let failed = |e: String| Message::with("error.geometry", vec![e]);
    match shape {
        Shape::SubD(subd) => Some(
            subd.to_nurbs(1, pack)
                .map(|(body, _)| Shape::Body(body))
                .map_err(|e| failed(e.to_string())),
        ),
        Shape::Mesh(mesh) => Some(mesh.to_nurbs().map(Shape::Body).map_err(|e| failed(e.0))),
        Shape::Curve(curve) if !matches!(curve, Curve::Point { .. } | Curve::Nurbs { .. }) => {
            curve.to_nurbs().ok().map(|c| Ok(Shape::Curve(c)))
        }
        _ => None,
    }
}

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
            let Some(shape) = converted(&object.shape, cx.toggle(PACK_FACES)) else {
                continue;
            };
            made.derived.push(Derived {
                shape: shape?,
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
    use curva_kernel::Curve;
    use curva_mesh::{Frame, Mesh, SubD};

    #[test]
    fn meshes_and_curves_become_nurbs_and_stay() {
        // Rhino 8's ToNURBS with Enter (21 November 2026): a 2 × 2 × 2 box mesh becomes a solid of
        // 24 faces holding 8 m³, and a curve becomes a NURBS curve; the originals stay.
        let mut doc = Document::default();
        let mesh = doc.add_shape(Shape::Mesh(Mesh::mesh_box(
            &Frame::world(),
            [2.0; 3],
            [2, 2, 2],
        )));
        let curve = Curve::interpolated(
            vec![
                [0.0, 5.0, 0.0],
                [1.0, 6.0, 0.0],
                [2.0, 5.0, 0.0],
                [3.0, 6.0, 0.0],
            ],
            false,
        )
        .unwrap();
        let curve = doc.add_shape(Shape::Curve(curve));
        doc.select(&[mesh, curve]);
        run(&mut doc, &["tonurbs"]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 4, "the originals stay");
        let body = objects
            .iter()
            .find_map(|o| o.shape.body())
            .expect("a polysurface");
        assert_eq!(body.face_count().unwrap(), 24);
        assert!(body.is_solid());
        assert!((body.volume().unwrap() - 8.0).abs() < 1e-6);
        assert!(
            objects
                .iter()
                .any(|o| matches!(&o.shape, Shape::Curve(Curve::Nurbs { .. })))
        );
    }

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
