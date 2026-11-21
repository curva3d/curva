//! Rhino's `Crease` and `RemoveCrease` for SubDs: the picked cage edges made sharp, or smooth
//! again. Measured in Rhino 8 (RhinoCommon picks, Enter, 21 November 2026): only edges count (a
//! picked face changes nothing), a crease's ends become darts, and the message counts the SubDs
//! changed ("Added creases to 1 object.", "Removed creases from 2 objects.").

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use std::collections::BTreeMap;

/// `Crease` when `ADD`, else `RemoveCrease`.
#[derive(Default)]
pub struct Creasing<const ADD: bool>;

/// Rhino's `Crease`.
pub type Crease = Creasing<true>;
/// Rhino's `RemoveCrease`.
pub type RemoveCrease = Creasing<false>;

impl<const ADD: bool> Recipe for Creasing<ADD> {
    const NAME: &'static str = if ADD { "crease" } else { "removecrease" };

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_parts(if ADD {
            "prompt.crease.edges"
        } else {
            "prompt.removecrease.edges"
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
            let changed = if ADD {
                subd.clone().with_creases(&edges)
            } else {
                subd.clone().without_creases(&edges)
            };
            made.replace.push((id, Shape::SubD(changed)));
        }
        let count = made.replace.len();
        if count == 0 {
            return Err(Message::new("error.crease_none"));
        }
        let key = match (ADD, count) {
            (true, 1) => "done.crease_one",
            (true, _) => "done.crease",
            (false, 1) => "done.removecrease_one",
            (false, _) => "done.removecrease",
        };
        made.message = Some(Message::with(key, vec![count.to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD, VertexTag};

    fn subd_box(doc: &mut Document) -> u64 {
        doc.add_shape(Shape::SubD(SubD::subd_box(
            &Frame::world(),
            [4.0; 3],
            [2, 2, 2],
        )))
    }

    fn subd(doc: &Document, id: u64) -> SubD {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.shape.subd().unwrap().clone()
    }

    /// The first edge of the cage's first face.
    fn first_edge(subd: &SubD) -> (u32, u32) {
        let face = subd.cage().faces().next().unwrap().to_vec();
        (face[0], face[1])
    }

    #[test]
    fn crease_then_remove_crease_as_in_rhino() {
        // Rhino 8: one picked edge becomes a crease and its ends darts; RemoveCrease makes it
        // smooth again.
        let mut doc = Document::default();
        let id = subd_box(&mut doc);
        let (a, b) = first_edge(&subd(&doc, id));
        doc.select_mesh_parts(&[(id, MeshPart::edge(a, b))]);
        run(&mut doc, &["crease", ""]);
        let creased = subd(&doc, id);
        assert!(creased.sharpness(a, b).is_crease());
        assert_eq!(creased.vertex_tag(a), VertexTag::Dart);
        assert_eq!(creased.vertex_tag(b), VertexTag::Dart);
        doc.select_mesh_parts(&[(id, MeshPart::edge(a, b))]);
        run(&mut doc, &["removecrease", ""]);
        let smooth = subd(&doc, id);
        assert!(!smooth.sharpness(a, b).is_crease());
        assert_eq!(smooth.vertex_tag(a), VertexTag::Smooth);
    }

    #[test]
    fn a_picked_face_changes_nothing_as_in_rhino() {
        let mut doc = Document::default();
        let id = subd_box(&mut doc);
        doc.select_mesh_parts(&[(id, MeshPart::Face(0))]);
        run(&mut doc, &["crease", ""]);
        let after = subd(&doc, id);
        assert_eq!(after.creases().count(), 0);
    }
}
