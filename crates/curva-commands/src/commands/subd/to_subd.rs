//! Rhino's `ToSubD` for meshes (spec `docs/specs/mesh-and-subd.md` §1, Stage B), on `curva-mesh`'s
//! `SubD::from_mesh`. Measured in Rhino 8's `-ToSubD` with Enter at every option: the mesh's
//! points become the cage (`UseMesh` ControlPoints), its unwelded edges are not creased
//! (`MeshCreases` off), an open border is creased with no corners (`MeshCorners` off), and the mesh
//! stays. A 2 × 2 × 2 box mesh gives a cage of 24 faces and 26 points, welded or not.
//!
//! Surfaces and polysurfaces go through `SubD::from_body`, Rhino's fit of a SubD to a surface;
//! Rhino takes only surfaces and extrusions, and Curva's polysurfaces stand in for its extrusions
//! (owner's choice, 21 November 2026).

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::{SubD, ToSubdOptions};

const USE_MESH: &str = "UseMesh";
const CREASES: &str = "MeshCreases";
const CORNERS: &str = "MeshCorners";

/// The recipe.
#[derive(Default)]
pub struct ToSubD;

impl Recipe for ToSubD {
    const NAME: &'static str = "tosubd";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::name(USE_MESH, "ControlPoints"),
        OptionDef::toggle(CREASES, false),
        OptionDef::toggle(CORNERS, false),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == USE_MESH).then(|| vec!["ControlPoints".to_owned(), "Location".to_owned()])
    }

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.tosubd.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        let options = ToSubdOptions {
            through_points: cx.name(USE_MESH) == "Location",
            creases: cx.toggle(CREASES),
            corners: cx.toggle(CORNERS),
        };
        let mut made = Made::default();
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
        {
            let subd = match &object.shape {
                Shape::Mesh(mesh) => SubD::from_mesh(mesh, options),
                Shape::Body(body) => SubD::from_body(body),
                _ => continue,
            }
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::SubD(subd),
                from: object.id,
                history: None,
            });
        }
        if made.derived.is_empty() {
            return Err(Message::new("error.tosubd_none"));
        }
        made.message = Some(Message::with(
            "done.tosubd",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, Mesh};

    fn converted(mesh: Mesh, steps: &[&str]) -> Document {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::Mesh(mesh));
        let mut all = vec!["tosubd"];
        all.extend_from_slice(steps);
        doc.select(&[id]);
        run(&mut doc, &all);
        doc
    }

    #[test]
    fn a_box_mesh_becomes_rhinos_cage_and_the_mesh_stays() {
        // Rhino 8: a 2 × 2 × 2 box mesh gives 24 cage faces and 26 points, nothing creased.
        let doc = converted(Mesh::mesh_box(&Frame::world(), [4.0; 3], [2, 2, 2]), &[]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        assert!(objects[0].shape.mesh().is_some(), "the mesh stays");
        let subd = objects[1].shape.subd().unwrap();
        assert_eq!(subd.cage().face_count(), 24);
        assert_eq!(subd.cage().vertex_count(), 26);
        assert_eq!(subd.creases().count(), 0);
    }

    #[test]
    fn an_open_plane_keeps_its_border_creased_as_in_rhino() {
        // Rhino 8: a 2 × 2 plane mesh gives 4 faces with its 8 border edges creased.
        let doc = converted(Mesh::mesh_plane(&Frame::world(), [4.0; 2], [2, 2]), &[]);
        let subd = doc.state().objects[1].shape.subd().unwrap();
        assert_eq!(subd.cage().face_count(), 4);
        assert_eq!(subd.creases().count(), 8);
    }
}

#[cfg(test)]
mod surface_tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::{Body, Curve};

    #[test]
    fn an_extruded_circle_becomes_rhinos_subd_and_stays() {
        // Rhino 8's ToSubD on a capped extrusion: 8 sides and two eight-sided caps.
        let mut doc = Document::default();
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let body = Body::extruded(&[circle], [0.0, 0.0, 1.0], 4.0, true).unwrap();
        let id = doc.add_shape(Shape::Body(body));
        doc.select(&[id]);
        run(&mut doc, &["tosubd"]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        assert!(objects[0].shape.body().is_some(), "the polysurface stays");
        let subd = objects[1].shape.subd().unwrap();
        assert_eq!(subd.cage().face_count(), 10);
        assert_eq!(subd.creases().count(), 16);
    }
}
