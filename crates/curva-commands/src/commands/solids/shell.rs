//! `Shell`: hollows a solid to a wall thickness, opening the picked faces, as in Rhino; Enter uses
//! the last thickness. The faces opened all belong to one solid, the first one picked. With
//! `DeleteInput` off, the hollowed solid is added and the original kept. Rhino's `EditThickness`
//! is still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;

const DELETE_INPUT: &str = "DeleteInput";

/// The recipe.
#[derive(Default)]
pub struct Shell;

impl Recipe for Shell {
    const NAME: &'static str = "shell";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DELETE_INPUT, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::faces("prompt.shell.faces")];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.shell.thickness").remembered("Thickness", 1.0));
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(thickness) = cx.values.get(1).and_then(Value::number)
            && thickness <= 0.0
        {
            return Err(Message::new("error.bad_distance"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Faces(picked)), Some(thickness)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::number))
        else {
            return Ok(Made::default());
        };
        let Some(&(id, _)) = picked.first() else {
            return Ok(Made::default());
        };
        if thickness <= 0.0 {
            return Ok(Made::default());
        }
        let faces: Vec<usize> = picked
            .iter()
            .filter(|(owner, _)| *owner == id)
            .map(|(_, face)| *face)
            .collect();
        let Some(body) = cx
            .doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| o.shape.body())
        else {
            return Ok(Made::default());
        };
        let hollow = body
            .shelled(&faces, thickness)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        let mut made = Made {
            message: Some(Message::new("done.shell")),
            ..Made::default()
        };
        if cx.toggle(DELETE_INPUT) {
            made.replace.push((id, Shape::Body(hollow)));
        } else {
            made.derived.push(Derived {
                shape: Shape::Body(hollow),
                from: id,
                history: None,
            });
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    /// A 10 mm cube and the number of its top face.
    fn cube(doc: &mut Document) -> (u64, usize) {
        let body = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let mesh = body.mesh().unwrap();
        let top = (0..mesh.triangles.len())
            .find(|&t| {
                mesh.triangles[t]
                    .iter()
                    .all(|&c| (mesh.positions[c as usize][2] - 10.0).abs() < 1e-9)
            })
            .map(|t| mesh.triangle_faces[t])
            .unwrap();
        (doc.add_body(body), top)
    }

    #[test]
    fn a_cube_opened_at_the_top_keeps_walls_of_the_thickness() {
        let mut doc = Document::default();
        let (id, top) = cube(&mut doc);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "shell");
        doc.select_faces(&[(id, top)]);
        processor.submit(&mut doc, "");
        assert!(!processor.preview(&doc, None, "1").is_empty());
        processor.submit(&mut doc, "1");
        assert!(!processor.is_running());
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - (1000.0 - 8.0 * 8.0 * 9.0)).abs() < 1e-3);
        processor.submit(&mut doc, "undo");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 1000.0).abs() < 1e-3);
    }
}
