//! Rhino's `QuadRemesh` (spec `docs/specs/mesh-and-subd.md` §3.5 and §10, #329): meshes, SubDs,
//! surfaces and polysurfaces turned into an even grid of quads near `TargetQuadCount`, on
//! `curva-mesh`'s `quad_remesh.rs`. As measured in Rhino 8's `-QuadRemesh`: the input stays, the
//! result is a new mesh on its layer, SubDs and NURBS are meshed first, and a 40 × 40 sphere comes
//! out as 1,750 quads at the default 2,000. `ConvertToSubD` makes a SubD instead, as Rhino's
//! dialog offers.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_document::meshing::{mesh_of, remembered_settings};
use curva_mesh::{QuadRemeshOptions, QuadTarget, SubD};

const TARGET: &str = "TargetQuadCount";
const EDGE_LENGTH: &str = "TargetEdgeLength";
const HARD_EDGES: &str = "DetectHardEdges";
const SUBD: &str = "ConvertToSubD";

/// Quads about `TargetEdgeLength` long when it is set, else about `TargetQuadCount` of them, as
/// in Rhino.
fn target(cx: &Context<'_>) -> QuadTarget {
    let length = cx.number(EDGE_LENGTH);
    if length > 0.0 {
        QuadTarget::EdgeLength(length)
    } else {
        QuadTarget::Count(cx.number(TARGET).round().max(1.0) as usize)
    }
}

/// The recipe.
#[derive(Default)]
pub struct QuadRemesh;

impl Recipe for QuadRemesh {
    const NAME: &'static str = "quadremesh";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(TARGET, 2000, 1, 1_000_000),
        // 0, as in Rhino, aims at the quad count instead.
        OptionDef::distance(EDGE_LENGTH, 0.0, true),
        OptionDef::toggle(HARD_EDGES, true),
        OptionDef::toggle(SUBD, false),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.quadremesh.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        let settings = remembered_settings(cx.doc);
        let found: Vec<_> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .filter_map(|o| mesh_of(&o.shape, &settings).map(|m| (o.id, m)))
            .collect();
        if found.is_empty() {
            return Err(Message::new("error.quadremesh_nothing"));
        }
        let options = QuadRemeshOptions {
            target: target(cx),
            hard_edges: cx.toggle(HARD_EDGES),
            ..QuadRemeshOptions::count(2000)
        };
        let mut made = Made::default();
        for (id, mesh) in found {
            let quads = mesh
                .quad_remeshed(&options)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            let shape = if cx.toggle(SUBD) {
                Shape::SubD(SubD::new(quads))
            } else {
                Shape::Mesh(quads)
            };
            made.derived.push(Derived {
                shape,
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.quadremesh",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, Mesh, SphereStyle};

    /// Rhino's measuring sphere: radius 5, 40 × 40 faces.
    fn sphere_40() -> Mesh {
        Mesh::mesh_sphere(
            &Frame::world(),
            5.0,
            SphereStyle::Uv {
                vertical: 40,
                around: 40,
            },
        )
    }

    fn remeshed(options: &[&str]) -> Document {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::Mesh(sphere_40()));
        let mut processor = Processor::default();
        processor.submit(&mut doc, "quadremesh");
        for option in options {
            processor.submit(&mut doc, option);
        }
        doc.select(&[id]);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        doc
    }

    #[test]
    fn a_sphere_comes_out_near_rhinos_count_and_the_input_stays() {
        let doc = remeshed(&[]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].shape.mesh().unwrap().face_count(), 1600);
        let quads = objects[1].shape.mesh().unwrap();
        // Rhino 8 makes 1,750 quads here (with or without its adaptive size) and Curva 1,990.
        // Within 15 % of Rhino's.
        let count = quads.face_count();
        assert!((1490..=2010).contains(&count), "{count}");
        assert!(quads.is_closed());
    }

    #[test]
    fn target_edge_length_sets_the_quads_size_as_in_rhino() {
        let doc = remeshed(&["TargetEdgeLength", "1"]);
        let count = doc.state().objects[1].shape.mesh().unwrap().face_count();
        // Rhino 8 makes 244 quads here.
        assert!((220..=270).contains(&count), "{count}");
    }

    #[test]
    fn convert_to_subd_makes_a_subd() {
        let doc = remeshed(&["ConvertToSubD"]);
        assert!(matches!(doc.state().objects[1].shape, Shape::SubD(_)));
    }
}
