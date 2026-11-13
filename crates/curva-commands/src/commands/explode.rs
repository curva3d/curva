//! `Explode`, as in Rhino: breaks joined curves and polylines back into separate curves.

use crate::Message;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// The recipe.
#[derive(Default)]
pub struct Explode;

impl Recipe for Explode {
    const NAME: &'static str = "explode";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.explode.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for object in &cx.doc.state().objects {
            if !chosen.contains(&object.id) {
                continue;
            }
            // An instance opens into its definition's objects, placed (one level), as in Rhino.
            if let Shape::Instance(instance) = &object.shape {
                let Some(definition) = cx
                    .doc
                    .definitions()
                    .iter()
                    .find(|d| d.id == instance.definition)
                else {
                    continue;
                };
                made.remove.push(object.id);
                for part in &definition.objects {
                    if let Some(shape) =
                        curva_document::blocks::shape_placed(&part.shape, &instance.placement)
                    {
                        made.derived.push(Derived {
                            shape,
                            from: object.id,
                            history: None,
                        });
                    }
                }
                continue;
            }
            let Some(parts) = object.curve().and_then(curva_kernel::Curve::exploded) else {
                continue;
            };
            made.remove.push(object.id);
            made.derived.extend(parts.into_iter().map(|part| Derived {
                shape: Shape::Curve(part),
                from: object.id,
                history: None,
            }));
        }
        if made.remove.is_empty() {
            return Err(Message::new("error.nothing_to_explode"));
        }
        made.message = Some(Message::with(
            "done.explode",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected(curve: Curve) -> Document {
        let mut doc = Document::default();
        let id = doc.add(curve);
        doc.select(&[id]);
        doc
    }

    #[test]
    fn a_polyline_becomes_separate_lines() {
        let points = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let mut doc = selected(Curve::polyline(points, false).unwrap());
        run(&mut doc, &["explode"]);
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn it_is_one_undo_step() {
        let points = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let mut doc = selected(Curve::polyline(points, false).unwrap());
        run(&mut doc, &["explode", "undo"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn a_single_curve_stays_as_it_is() {
        let mut doc = selected(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let processor = run(&mut doc, &["explode"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }
}
