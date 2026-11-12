//! `Pipe`, as in Rhino: a round tube along curves; Enter uses the last radius. `Cap` closes the
//! ends; `Thick` asks for an inner radius too, for a wall between the two (Enter: half the outer);
//! `Diameter` takes diameters instead of radii. Rhino's radii along the curve, `ChainEdges`,
//! `Multiple`, `FitRail`, `ShapeBlending` and `WallThickness` are still to come
//! (`docs/commands/rhino-reference.md`).

use super::curves_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

const CAP: &str = "Cap";
const THICK: &str = "Thick";
const DIAMETER: &str = "Diameter";

/// The recipe.
#[derive(Default)]
pub struct Pipe;

/// The outer radius and, for a thick pipe, the inner one, from the sizes given.
fn radii(cx: &Context<'_>) -> Option<(f64, Option<f64>)> {
    let scale = if cx.toggle(DIAMETER) { 0.5 } else { 1.0 };
    let outer = cx.values.get(1).and_then(Value::number)? * scale;
    if !cx.toggle(THICK) {
        return Some((outer, None));
    }
    let inner = match cx.values.get(2) {
        Some(Value::Number(inner)) => inner * scale,
        _ => outer / 2.0,
    };
    Some((outer, Some(inner)))
}

impl Recipe for Pipe {
    const NAME: &'static str = "pipe";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(CAP, true),
        OptionDef::toggle(THICK, false),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.pipe.objects")
                .curves()
                .offering(&[]),
        ];
        if cx.values.is_empty() {
            return steps;
        }
        steps.push(StepDef::number("prompt.pipe.radius").remembered("Radius", 1.0));
        if cx.toggle(THICK) && cx.values.len() >= 2 {
            steps.push(
                StepDef::number("prompt.pipe.inner")
                    .optional()
                    .offering(&[]),
            );
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx
            .values
            .get(1)
            .and_then(Value::number)
            .is_some_and(|r| r <= 0.0)
        {
            return Err(Message::new("error.bad_distance"));
        }
        Ok(())
    }

    /// An inner radius must be more than zero and less than the outer one; otherwise it is
    /// asked again.
    fn accepts(&self, cx: &Context<'_>) -> bool {
        match (cx.values.len(), radii(cx)) {
            (3, Some((outer, Some(inner)))) => inner > 0.0 && inner < outer,
            _ => true,
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some((radius, inner))) = (cx.values.first(), radii(cx))
        else {
            return Ok(Made::default());
        };
        // While the inner radius is asked for, the plain pipe shows.
        let inner = inner.filter(|_| cx.values.len() >= 3 || !cx.toggle(THICK));
        if radius <= 0.0 || inner.is_some_and(|i| i <= 0.0 || i >= radius) {
            return Ok(Made::default());
        }
        let mut made = Made::default();
        for (id, rail) in curves_of(cx.doc, ids) {
            if matches!(rail, Curve::Point { .. }) {
                continue;
            }
            let body = Body::pipe(&rail, radius, inner, cx.toggle(CAP))
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.pipe",
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
    use std::f64::consts::PI;

    fn selected_rail(doc: &mut Document) {
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
    }

    #[test]
    fn a_pipe_follows_the_curve_with_the_radius_typed() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        let mut processor = run(&mut doc, &["pipe"]);
        assert!(!processor.preview(&doc, None, "2").is_empty());
        processor.submit(&mut doc, "2");
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!((body.volume().unwrap() - PI * 4.0 * 10.0).abs() < 1e-2);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn a_thick_pipe_has_a_wall() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        run(&mut doc, &["pipe", "Thick", "2", "1"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!((body.volume().unwrap() - PI * 3.0 * 10.0).abs() < 1e-2);
    }

    #[test]
    fn an_inner_radius_wider_than_the_outer_is_asked_again() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        let processor = run(&mut doc, &["pipe", "t", "2", "3"]);
        assert!(processor.is_running());
    }

    #[test]
    fn diameter_takes_the_width_across() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        run(&mut doc, &["pipe", "diameter", "4"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!((body.volume().unwrap() - PI * 4.0 * 10.0).abs() < 1e-2);
    }
}
