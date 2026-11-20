//! The `ExtractSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;

/// `ExtractSrf`: takes picked faces out of polysurfaces as surfaces of their own; with `Copy`,
/// copies them and leaves the polysurface whole.
#[derive(Default)]
pub struct ExtractSrf;

impl Recipe for ExtractSrf {
    const NAME: &'static str = "extractsrf";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Copy", false)];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::faces("prompt.extractsrf.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Faces(picked)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut owners: Vec<u64> = picked.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, &owners) {
            let faces: Vec<usize> = picked
                .iter()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, face)| *face)
                .collect();
            let count = body.face_count().map_err(|e| failed(&e))?;
            let taken = body.faces_only(&faces).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(taken),
                from: id,
                history: None,
            });
            if !cx.toggle("Copy") {
                let rest: Vec<usize> = (0..count).filter(|f| !faces.contains(f)).collect();
                if rest.is_empty() {
                    made.remove.push(id);
                } else {
                    let left = body.faces_only(&rest).map_err(|e| failed(&e))?;
                    made.replace.push((id, Shape::Body(left)));
                }
            }
        }
        made.message = Some(Message::with(
            "done.extractsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn extract_a_face_and_merge_faces() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select_faces(&[(cube, 0)]);
        run(&mut doc, &["extractsrf", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        let faces: Vec<usize> = doc
            .state()
            .objects
            .iter()
            .map(|o| o.shape.body().unwrap().face_count().unwrap())
            .collect();
        assert_eq!(faces, vec![5, 1]);
        // Two boxes joined side by side have coplanar faces; merging them leaves a box's six.
        let mut doc = Document::default();
        let a = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let b = Body::cuboid([1.0, 0.0, 0.0], [1.0, 1.0, 1.0]).unwrap();
        let joined = a
            .combined(&b, curva_kernel::Combine::Union)
            .unwrap()
            .unwrap();
        let id = doc.add_body(joined);
        doc.select(&[id]);
        run(&mut doc, &["mergeallfaces"]);
        assert_eq!(
            doc.state().objects[0]
                .shape
                .body()
                .unwrap()
                .face_count()
                .unwrap(),
            6
        );
    }
}
