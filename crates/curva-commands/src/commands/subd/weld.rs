//! Rhino's `UnweldEdge` and `WeldEdge` for SubDs: the picked cage edges cut open into creased
//! borders, or border edges at the same place joined back and made smooth. Measured in Rhino 8
//! (RhinoCommon picks, 22 November 2026): unwelding two edges in a row of a 2 × 2 × 2 box adds one
//! point (the middle one; the ends stay whole) and four creased border edges, with no message;
//! welding those borders gives the box back. Edges that cannot be used are left alone.

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use std::collections::BTreeMap;

/// `WeldEdge` when `WELD`, else `UnweldEdge`.
#[derive(Default)]
pub struct Welding<const WELD: bool>;

/// Rhino's `WeldEdge`.
pub type WeldEdge = Welding<true>;
/// Rhino's `UnweldEdge`.
pub type UnweldEdge = Welding<false>;

impl<const WELD: bool> Recipe for Welding<WELD> {
    const NAME: &'static str = if WELD { "weldedge" } else { "unweldedge" };

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_parts(if WELD {
            "prompt.weldedge.edges"
        } else {
            "prompt.unweldedge.edges"
        })]
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
            let Some(subd) = cx
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.subd())
            else {
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
}
