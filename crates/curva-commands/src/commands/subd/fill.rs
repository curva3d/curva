//! Rhino's `Fill` for SubDs and meshes: the holes the picked border edges run round closed with
//! new faces. A mesh hole fills only when all its open edges are picked, as a piece of its own
//! joined to the mesh (Rhino 8, measured).
//! Measured in Rhino 8 (RhinoCommon picks, `-Fill`, 22 November 2026): it asks "Select SubD and
//! mesh boundary edges.", then "Patch style <Automatic> ( Automatic  SingleFace  TriangleFan
//! Triangulated )"; Automatic closes an open box's square hole of eight edges with four quads
//! round a new middle point, and the filled border becomes smooth. Curva offers the patch style
//! while the edges are picked, as there is no step for a choice of words yet.

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use curva_mesh::FillStyle;
use std::collections::BTreeMap;

const PATCH_STYLE: &str = "PatchStyle";
const STYLES: [&str; 4] = ["Automatic", "SingleFace", "TriangleFan", "Triangulated"];

/// The recipe.
#[derive(Default)]
pub struct Fill;

fn style(cx: &Context<'_>) -> FillStyle {
    match cx.name(PATCH_STYLE).as_str() {
        "SingleFace" => FillStyle::SingleFace,
        "TriangleFan" => FillStyle::TriangleFan,
        "Triangulated" => FillStyle::Triangulated,
        _ => FillStyle::Automatic,
    }
}

impl Recipe for Fill {
    const NAME: &'static str = "fill";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::name(PATCH_STYLE, "Automatic")];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == PATCH_STYLE).then(|| STYLES.map(str::to_owned).to_vec())
    }

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_parts("prompt.fill.edges").offering(&[PATCH_STYLE])]
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
            let failed =
                |e: curva_mesh::MeshError| Message::with("error.geometry", vec![e.to_string()]);
            let Some(object) = cx.doc.state().objects.iter().find(|o| o.id == id) else {
                continue;
            };
            match &object.shape {
                Shape::SubD(subd) => {
                    let filled = subd.filled_at(&edges, style(cx)).map_err(failed)?;
                    if filled.cage().face_count() > subd.cage().face_count() {
                        made.replace.push((id, Shape::SubD(filled)));
                    }
                }
                // A mesh hole fills only when all its open edges are picked, as in Rhino 8.
                Shape::Mesh(mesh) => {
                    let (filled, holes) = mesh.filled_at(&edges, style(cx)).map_err(failed)?;
                    if holes > 0 {
                        made.replace.push((id, Shape::Mesh(filled)));
                    }
                }
                _ => {}
            }
        }
        if made.replace.is_empty() {
            return Err(Message::new("error.fill_none"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, Mesh, SubD};

    #[test]
    fn a_mesh_hole_fills_when_all_its_edges_are_picked_as_in_rhino() {
        // Rhino 8: an open 4 m box mesh of 2 × 2 faces a side, all eight top edges picked, gets
        // four quads on nine new points; one edge alone fills nothing.
        let whole = Mesh::mesh_box(&Frame::world(), [4.0; 3], [2, 2, 2]);
        let rest: Vec<Vec<u32>> = whole
            .faces()
            .filter(|f| !f.iter().all(|&v| whole.vertices()[v as usize][2] > 3.99))
            .map(<[u32]>::to_vec)
            .collect();
        let open = Mesh::new(whole.vertices().to_vec(), &rest).unwrap();
        let open = open.without_unused_vertices().0;
        let top: Vec<MeshPart> = open
            .faces()
            .flat_map(|f| {
                let n = f.len();
                (0..n)
                    .map(move |k| (f[k], f[(k + 1) % n]))
                    .collect::<Vec<_>>()
            })
            .filter(|&(a, b)| {
                let at = |v: u32| open.vertices()[v as usize][2];
                at(a) > 3.99 && at(b) > 3.99
            })
            .map(|(a, b)| MeshPart::edge(a, b))
            .collect();
        assert_eq!(top.len(), 8);
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::Mesh(open));
        doc.select_mesh_parts(&[(id, top[0])]);
        run(&mut doc, &["fill", ""]);
        let mesh = |doc: &Document| doc.state().objects[0].shape.mesh().unwrap().clone();
        assert_eq!(mesh(&doc).face_count(), 20, "one edge fills nothing");
        let all: Vec<(u64, MeshPart)> = top.iter().map(|&p| (id, p)).collect();
        doc.select_mesh_parts(&all);
        run(&mut doc, &["fill", ""]);
        let filled = mesh(&doc);
        assert_eq!((filled.face_count(), filled.vertex_count()), (24, 34));
        assert!(filled.is_closed());
    }

    /// A 4 m box SubD of 2 × 2 faces a side, without its four top faces.
    fn open_box(doc: &mut Document) -> u64 {
        let cage = SubD::subd_box(&Frame::world(), [4.0; 3], [2, 2, 2]);
        let top: Vec<usize> = cage
            .cage()
            .faces()
            .enumerate()
            .filter(|(_, f)| {
                f.iter()
                    .all(|&v| cage.cage().vertices()[v as usize][2] > 3.99)
            })
            .map(|(k, _)| k)
            .collect();
        doc.add_shape(Shape::SubD(cage.without_faces(&top).unwrap()))
    }

    fn subd(doc: &Document, id: u64) -> SubD {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.shape.subd().unwrap().clone()
    }

    fn a_border_edge(subd: &SubD) -> MeshPart {
        let [a, b] = subd.creases().next().unwrap();
        MeshPart::edge(a, b)
    }

    #[test]
    fn automatic_fills_the_hole_with_four_quads_as_in_rhino() {
        // Rhino 8: 20 quads become 24, one new point in the middle, no crease left.
        let mut doc = Document::default();
        let id = open_box(&mut doc);
        let before = subd(&doc, id);
        assert_eq!(before.cage().face_count(), 20);
        doc.select_mesh_parts(&[(id, a_border_edge(&before))]);
        run(&mut doc, &["fill", ""]);
        let after = subd(&doc, id);
        assert_eq!(after.cage().face_count(), 24);
        assert!(after.cage().faces().all(|f| f.len() == 4));
        assert_eq!(
            after.cage().vertex_count(),
            before.cage().vertex_count() + 1
        );
        assert_eq!(after.creases().count(), 0);
        assert!(after.cage().vertices().contains(&[2.0, 2.0, 4.0]));
    }

    #[test]
    fn single_face_closes_the_hole_with_one_face() {
        let mut doc = Document::default();
        let id = open_box(&mut doc);
        let edge = a_border_edge(&subd(&doc, id));
        let mut processor = Processor::default();
        processor.submit(&mut doc, "fill");
        processor.submit(&mut doc, "PatchStyle");
        processor.submit(&mut doc, "SingleFace");
        doc.select_mesh_parts(&[(id, edge)]);
        processor.submit(&mut doc, "");
        let after = subd(&doc, id);
        assert_eq!(after.cage().face_count(), 21);
        assert!(after.cage().faces().any(|f| f.len() == 8));
    }
}
