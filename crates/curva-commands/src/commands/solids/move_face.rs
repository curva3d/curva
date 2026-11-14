//! The `MoveFace` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// `MoveFace` (and `PushPull`, its Serpentine3D name): moves picked faces of a solid in or out
/// along their normals by a distance, the solid following.
#[derive(Default)]
pub struct MoveFace<const PUSH: bool>;

impl<const PUSH: bool> Recipe for MoveFace<PUSH> {
    const NAME: &'static str = if PUSH { "pushpull" } else { "moveface" };

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::faces("prompt.moveface.faces")];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.moveface.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Faces(picked)), Some(distance)) = (cx.values.first(), number(cx, 1))
        else {
            return Ok(Made::default());
        };
        if distance.abs() < 1e-9 {
            return Err(Message::new("error.bad_distance"));
        }
        let mut owners: Vec<u64> = picked.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, &owners) {
            let faces: Vec<usize> = picked
                .iter()
                .filter(|(o, _)| *o == id)
                .map(|(_, f)| *f)
                .collect();
            let moved = body
                .faces_offset(&faces, distance)
                .map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Body(moved)));
        }
        made.message = Some(Message::with(
            "done.moveface",
            vec![picked.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    #[test]
    fn move_faces_shear_rebuild_and_cap() {
        let mut doc = Document::default();
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let mesh = cube.mesh().unwrap();
        let top = mesh
            .triangle_faces
            .iter()
            .zip(&mesh.triangles)
            .find(|(_, t)| {
                t.iter()
                    .all(|i| (mesh.positions[*i as usize][2] - 1.0).abs() < 1e-9)
            })
            .map(|(f, _)| *f)
            .unwrap();
        let id = doc.add_body(cube);
        doc.select_faces(&[(id, top)]);
        run(&mut doc, &["pushpull", "", "0.5"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        doc.select(&[id]);
        run(&mut doc, &["shear", "0,0", "1,0", "30"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        let mut doc = Document::default();
        let a = Curve::control_point(vec![[0.0; 3], [2.0, 0.0, 1.0], [4.0, 0.0, 0.0]], 2, false)
            .unwrap();
        let b = Curve::control_point(
            vec![[0.0, 3.0, 0.0], [2.0, 3.0, 1.0], [4.0, 3.0, 0.0]],
            2,
            false,
        )
        .unwrap();
        let sheet = doc.add_body(Body::ruled(&a, &b, false).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "rebuilduv");
        for typed in ["PointCountU", "8", "PointCountV", "8"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[sheet]);
        processor.submit(&mut doc, "");
        let (rows, cols, _) = last_body(&doc).control_net().unwrap();
        assert_eq!((rows, cols), (8, 8));
        let mut doc = Document::default();
        let square = Curve::polyline(
            vec![[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            true,
        )
        .unwrap();
        let tube = doc.add_body(Body::extruded(&[square], [0.0, 0.0, 1.0], 3.0, false).unwrap());
        doc.select(&[tube]);
        run(&mut doc, &["cap"]);
        assert!(last_body(&doc).is_solid());
    }
}
