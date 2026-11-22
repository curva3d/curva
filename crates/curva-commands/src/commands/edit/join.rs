//! `Join`, as in Rhino: makes curves that meet end to end into one curve, and meshes into one
//! mesh, nothing welded (#414). Rhino's `Undo` while
//! picking one by one is still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, join};

/// The recipe.
#[derive(Default)]
pub struct Join;

impl Recipe for Join {
    const NAME: &'static str = "join";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.join.objects").at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut result = Made::default();
        join_meshes(cx, chosen, &mut result)?;
        let (ids, curves): (Vec<u64>, Vec<Curve>) = curves_of(cx.doc, chosen).into_iter().unzip();
        let made = if curves.len() >= 2 {
            join(&curves).map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?
        } else {
            Vec::new()
        };
        if made.is_empty() && result.remove.is_empty() {
            return Err(Message::new("error.nothing_to_join"));
        }
        for (replaced, curve) in made {
            let gone: Vec<u64> = replaced.iter().map(|index| ids[*index]).collect();
            result.derived.push(Derived {
                shape: Shape::Curve(curve),
                from: gone[0],
                history: None,
            });
            result.remove.extend(gone);
        }
        result.message = Some(Message::with(
            "done.join",
            vec![result.remove.len().to_string()],
        ));
        Ok(result)
    }
}

/// Two or more meshes among `chosen` made one, nothing welded, on the first one's layer, as
/// Rhino's `Join` does for meshes (#414); curves and others are left to the rest.
fn join_meshes(cx: &Context<'_>, chosen: &[u64], result: &mut Made) -> Result<(), Message> {
    let meshes: Vec<(u64, &curva_mesh::Mesh)> = cx
        .doc
        .state()
        .objects
        .iter()
        .filter(|o| chosen.contains(&o.id))
        .filter_map(|o| o.shape.mesh().map(|m| (o.id, m)))
        .collect();
    if meshes.len() < 2 {
        return Ok(());
    }
    let parts: Vec<&curva_mesh::Mesh> = meshes.iter().map(|(_, m)| *m).collect();
    let joined = curva_mesh::Mesh::joined(&parts)
        .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
    result.derived.push(Derived {
        shape: Shape::Mesh(joined),
        from: meshes[0].0,
        history: None,
    });
    result.remove.extend(meshes.iter().map(|(id, _)| *id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn two_selected_lines(gap: f64) -> Document {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([1.0 + gap, 0.0, 0.0], [2.0, 1.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        doc
    }

    #[test]
    fn two_lines_become_one_curve() {
        let mut doc = two_selected_lines(0.0);
        run(&mut doc, &["join"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = two_selected_lines(0.0);
        run(&mut doc, &["join", "undo"]);
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn curves_that_do_not_meet_stay_as_they_are() {
        let mut doc = two_selected_lines(0.5);
        let processor = run(&mut doc, &["join"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(!processor.is_running());
    }

    #[test]
    fn with_too_little_selected_it_asks_for_objects() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["join"]);
        assert!(processor.is_running());
    }
}

#[cfg(test)]
mod mesh_tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::Mesh;

    fn square(at: f64) -> Mesh {
        let points = vec![
            [at, 0.0, 0.0],
            [at + 1.0, 0.0, 0.0],
            [at + 1.0, 1.0, 0.0],
            [at, 1.0, 0.0],
        ];
        Mesh::new(points, &[[0u32, 1, 2, 3]]).unwrap()
    }

    fn faces(doc: &Document) -> Vec<usize> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| o.shape.mesh().map(|m| m.faces().count()))
            .collect()
    }

    #[test]
    fn meshes_join_into_one_and_explode_back_into_their_pieces() {
        // As Rhino's Join and Explode do for meshes (#414).
        let mut doc = Document::default();
        let a = doc.add_shape(Shape::Mesh(square(0.0)));
        let b = doc.add_shape(Shape::Mesh(square(3.0)));
        doc.select(&[a, b]);
        run(&mut doc, &["join"]);
        assert_eq!(faces(&doc), [2], "one mesh of both");
        let joined = doc.state().objects[0].id;
        doc.select(&[joined]);
        run(&mut doc, &["explode"]);
        assert_eq!(faces(&doc), [1, 1], "its two pieces again");
        // A mesh of one piece stays as it is.
        let one = doc.state().objects[0].id;
        doc.select(&[one]);
        run(&mut doc, &["explode"]);
        assert_eq!(faces(&doc), [1, 1]);
    }
}
