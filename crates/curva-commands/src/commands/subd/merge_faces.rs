//! Rhino's `MergeFaces` for SubDs: picked faces that touch along edges become one face. Measured
//! in Rhino 8 (RhinoCommon picks, Enter, 22 November 2026): two neighbouring quads on a side of a
//! 4 m SubD box of 2 × 2 a side become one six-sided face, every point kept, and Rhino says
//! "Changed 1 objects.".

use super::parts::by_object;
use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::{MeshPart, Shape};

/// The recipe.
#[derive(Default)]
pub struct MergeFaces;

impl Recipe for MergeFaces {
    const NAME: &'static str = "mergefaces";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_parts("prompt.mergefaces.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, parts) in by_object(cx, 0) {
            let faces: Vec<usize> = parts
                .iter()
                .filter_map(|p| match *p {
                    MeshPart::Face(f) => Some(f as usize),
                    _ => None,
                })
                .collect();
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
            let merged = subd
                .with_faces_merged(&faces)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            if merged.cage().face_count() < subd.cage().face_count() {
                made.replace.push((id, Shape::SubD(merged)));
            }
        }
        if made.replace.is_empty() {
            return Err(Message::new("error.mergefaces_none"));
        }
        // Rhino's own words, "objects" even for one.
        made.message = Some(Message::with(
            "done.mergefaces",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD};

    #[test]
    fn two_neighbouring_faces_merge_as_in_rhino() {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let at = subd.cage().vertices().to_vec();
        let face_at = |p: [f64; 3]| {
            let k = subd
                .cage()
                .faces()
                .position(|f| {
                    let n = f.len() as f64;
                    (0..3).all(|i| {
                        let mid = f.iter().map(|&v| at[v as usize][i]).sum::<f64>() / n;
                        (mid - p[i]).abs() < 1e-9
                    })
                })
                .unwrap();
            MeshPart::Face(u32::try_from(k).unwrap())
        };
        let (a, b) = (face_at([201.0, 0.0, 1.0]), face_at([203.0, 0.0, 1.0]));
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(subd));
        doc.select_mesh_parts(&[(id, a), (id, b)]);
        run(&mut doc, &["mergefaces", ""]);
        let merged = doc.state().objects[0].shape.subd().unwrap();
        assert_eq!(
            (merged.cage().face_count(), merged.cage().vertex_count()),
            (23, 26)
        );
        assert!(merged.cage().faces().any(|f| f.len() == 6));
    }
}
