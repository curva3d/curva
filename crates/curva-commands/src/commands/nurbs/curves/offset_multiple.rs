//! The `OffsetMultiple` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `OffsetMultiple`: offsets several curves at once by one distance, each to the side of a
/// point.
#[derive(Default)]
pub struct OffsetMultiple;

impl Recipe for OffsetMultiple {
    const NAME: &'static str = "offsetmultiple";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.offsetmultiple.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.offset.distance").remembered("Distance", 1.0));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::point("prompt.offsetmultiple.side"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(distance), Some(side)) = (number(cx, 1), point(cx, 2)) else {
            return Ok(Made::default());
        };
        if distance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let normal = up(cx);
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let moved = curve
                .offset(normal, distance, side)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(moved),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.offsetmultiple",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn offset_multiple_moves_each_curve_to_the_side() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 5.0, 0.0], [10.0, 5.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["offsetmultiple", "1", "5,20"]);
        assert_eq!(doc.state().objects.len(), 4);
        let ys: Vec<f64> = doc.state().objects[2..]
            .iter()
            .map(|o| o.curve().unwrap().sample().unwrap()[0][1])
            .collect();
        assert!(
            ys.iter().any(|y| (y - 1.0).abs() < 1e-9) && ys.iter().any(|y| (y - 6.0).abs() < 1e-9),
            "{ys:?}"
        );
    }
}
