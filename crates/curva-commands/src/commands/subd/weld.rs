//! Rhino's `UnweldEdge` and `WeldEdge` for SubDs: the picked cage edges cut open into creased
//! borders, or border edges at the same place joined back and made smooth. Measured in Rhino 8
//! (RhinoCommon picks, 22 November 2026): unwelding two edges in a row of a 2 × 2 × 2 box adds one
//! point (the middle one; the ends stay whole) and four creased border edges, with no message;
//! welding those borders gives the box back. Edges that cannot be used are left alone.
//!
//! On meshes UnweldEdge gives each point on the picked edges a vertex for each side, with
//! ModifyNormals (Yes, as Rhino) the normals of each side (measured on a mesh box the same day:
//! one folded edge, 26 vertices become 28; see `Mesh::with_edges_unwelded`). Rhino's
//! SharpCorners and WeldEdge on meshes are not here yet.

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use std::collections::BTreeMap;

const MODIFY_NORMALS: &str = "ModifyNormals";

/// `WeldEdge` when `WELD`, else `UnweldEdge`.
#[derive(Default)]
pub struct Welding<const WELD: bool>;

/// Rhino's `WeldEdge`.
pub type WeldEdge = Welding<true>;
/// Rhino's `UnweldEdge`.
pub type UnweldEdge = Welding<false>;

impl<const WELD: bool> Recipe for Welding<WELD> {
    const NAME: &'static str = if WELD { "weldedge" } else { "unweldedge" };
    const OPTIONS: &'static [OptionDef] = if WELD {
        &[]
    } else {
        &[OptionDef::toggle(MODIFY_NORMALS, true)]
    };

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        if WELD {
            vec![StepDef::mesh_parts("prompt.weldedge.edges")]
        } else {
            vec![StepDef::mesh_parts("prompt.unweldedge.edges").offering(&[MODIFY_NORMALS])]
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut picked: BTreeMap<u64, Vec<[u32; 2]>> = BTreeMap::new();
        if let Some(Value::MeshParts(parts)) = cx.values.first() {
            for &(id, part) in parts {
                if let MeshPart::Edge(a, b) = part {
                    picked.entry(id).or_default().push([a, b]);
                }
            }
        }
        let mut made = Made::default();
        for (id, edges) in picked {
            let Some(object) = cx.doc.state().objects.iter().find(|o| o.id == id) else {
                continue;
            };
            if let (false, Some(mesh)) = (WELD, object.shape.mesh()) {
                if let Ok(cut) = mesh.with_edges_unwelded(&edges, cx.toggle(MODIFY_NORMALS)) {
                    made.replace.push((id, Shape::Mesh(cut)));
                }
                continue;
            }
            let Some(subd) = object.shape.subd() else {
                continue;
            };
            let changed = if WELD {
                subd.welded(&edges)
            } else {
                subd.unwelded(&edges)
            };
            if let Ok(changed) = changed {
                made.replace.push((id, Shape::SubD(changed)));
            }
        }
        if made.replace.is_empty() {
            return Err(Message::new(if WELD {
                "error.weldedge_none"
            } else {
                "error.unweldedge_none"
            }));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD};

    fn subd(doc: &Document, id: u64) -> SubD {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.shape.subd().unwrap().clone()
    }

    /// The point of the cage at `p`.
    fn at(subd: &SubD, p: [f64; 3]) -> u32 {
        let k = subd.cage().vertices().iter().position(|&q| q == p).unwrap();
        u32::try_from(k).unwrap()
    }

    #[test]
    fn unweld_then_weld_as_in_rhino() {
        // Rhino 8: the two bottom-front edges of a 4 m box of 2 × 2 faces a side cut open into
        // 27 points and 4 creased border edges; WeldEdge on them gives the 26-point box back.
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(SubD::subd_box(
            &Frame::world(),
            [4.0; 3],
            [2, 2, 2],
        )));
        let before = subd(&doc, id);
        let [a, b, c] = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]].map(|p| at(&before, p));
        doc.select_mesh_parts(&[(id, MeshPart::edge(a, b)), (id, MeshPart::edge(b, c))]);
        run(&mut doc, &["unweldedge", ""]);
        let cut = subd(&doc, id);
        assert_eq!(cut.cage().vertex_count(), 27);
        assert_eq!(cut.creases().count(), 4);
        let border: Vec<(u64, MeshPart)> = cut
            .creases()
            .map(|[p, q]| (id, MeshPart::edge(p, q)))
            .collect();
        doc.select_mesh_parts(&border);
        run(&mut doc, &["weldedge", ""]);
        let welded = subd(&doc, id);
        assert_eq!(welded.cage().vertex_count(), 26);
        assert_eq!(welded.creases().count(), 0);
    }

    #[test]
    fn a_mesh_edge_unwelds_as_in_rhino() {
        // Rhino 8: one edge on a folded corner of a welded 4 m mesh box of 2 × 2 faces a side
        // gives each of its two points a second vertex: 26 become 28.
        let mut doc = Document::default();
        let mesh = SubD::subd_box(&Frame::world(), [4.0; 3], [2, 2, 2])
            .cage()
            .clone()
            .with_rebuilt_normals();
        let at = |p: [f64; 3]| {
            let k = mesh.vertices().iter().position(|&q| q == p).unwrap();
            u32::try_from(k).unwrap()
        };
        let edge = MeshPart::edge(at([0.0, 0.0, 0.0]), at([0.0, 0.0, 2.0]));
        let id = doc.add_shape(Shape::Mesh(mesh));
        doc.select_mesh_parts(&[(id, edge)]);
        run(&mut doc, &["unweldedge", ""]);
        let out = doc.state().objects[0].shape.mesh().unwrap();
        assert_eq!((out.vertex_count(), out.face_count()), (28, 24));
    }
}
