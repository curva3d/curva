//! `OffsetSrf`, as in Rhino: a copy of surfaces or solids at a distance; Enter uses the last one.
//! `Solid` thickens an open surface into a solid between where it was and the copy;
//! `DeleteInput` takes the originals away. Rhino's `Corner`, `ThroughPoint`, `FlipAll`, `Loose`,
//! `Tolerance` and `BothSides` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;

/// Shortest offset treated as real, in millimetres.
const SHORTEST: f64 = 1e-9;
const SOLID: &str = "Solid";
const DELETE_INPUT: &str = "DeleteInput";

/// The recipe.
#[derive(Default)]
pub struct OffsetSrf;

impl Recipe for OffsetSrf {
    const NAME: &'static str = "offsetsrf";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(SOLID, false),
        OptionDef::toggle(DELETE_INPUT, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.offsetsrf.objects")
                .bodies()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.offsetsrf.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(distance) = cx.values.get(1).and_then(Value::number)
            && distance.abs() <= SHORTEST
        {
            return Err(Message::new("error.bad_distance"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(distance)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::number))
        else {
            return Ok(Made::default());
        };
        if distance.abs() <= SHORTEST {
            return Ok(Made::default());
        }
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, ids) {
            let copy = body
                .offset(distance, cx.toggle(SOLID))
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(copy),
                from: id,
                history: None,
            });
            if cx.toggle(DELETE_INPUT) {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.offsetsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn selected_sheet(doc: &mut Document) {
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let id = doc.add_body(Body::planar(&[square]).unwrap());
        doc.select(&[id]);
    }

    #[test]
    fn a_surface_is_copied_at_the_distance() {
        let mut doc = Document::default();
        selected_sheet(&mut doc);
        let mut processor = run(&mut doc, &["offsetsrf"]);
        assert!(!processor.preview(&doc, None, "3").is_empty());
        processor.submit(&mut doc, "3");
        assert_eq!(doc.state().objects.len(), 2);
        let (low, _) = doc.state().objects[1]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((low[2].abs() - 3.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn solid_thickens_the_surface() {
        let mut doc = Document::default();
        selected_sheet(&mut doc);
        run(&mut doc, &["offsetsrf", "Solid", "2"]);
        let volume = doc.state().objects[1]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 200.0).abs() < 1e-3);
    }

    #[test]
    fn delete_input_takes_the_original_away() {
        let mut doc = Document::default();
        selected_sheet(&mut doc);
        run(&mut doc, &["offsetsrf", "deleteinput", "2"]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
