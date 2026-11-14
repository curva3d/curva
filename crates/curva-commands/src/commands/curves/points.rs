//! The `Points` command.

use crate::Message;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Curve;

/// Steps for one point after another until Enter.
fn many_points(cx: &Context<'_>, prompt: &'static str) -> Vec<StepDef> {
    let mut steps: Vec<StepDef> = cx.values.iter().map(|_| StepDef::point(prompt)).collect();
    if cx.values.last() != Some(&Value::Empty) {
        let mut step = StepDef::point(prompt);
        if let Some(last) = picked(cx).last() {
            step = step.from(*last);
        }
        steps.push(if picked(cx).is_empty() {
            step
        } else {
            step.optional()
        });
    }
    steps
}

/// `Points`: places points one after another until Enter.
#[derive(Default)]
pub struct Points;

impl Recipe for Points {
    const NAME: &'static str = "points";
    const PLACES_AS_IT_GOES: bool = true;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        many_points(cx, "prompt.point.at")
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = picked(cx);
        let count = points.len();
        Ok(Made {
            add: points
                .into_iter()
                .map(|at| Shape::Curve(Curve::Point { at }))
                .collect(),
            message: Some(Message::with("done.points", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use std::f64::consts::PI;

    #[test]
    fn points_lines_and_slot() {
        let mut doc = Document::default();
        run(&mut doc, &["points", "0,0", "1,0", "2,0", ""]);
        assert_eq!(doc.state().objects.len(), 3);
        let mut doc = Document::default();
        run(&mut doc, &["lines", "0,0", "1,0", "0,1", "1,1", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        let mut doc = Document::default();
        run(&mut doc, &["slot", "0,0", "10,0", "4"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - (20.0 + 2.0 * PI * 2.0)).abs() < 1e-3, "{length}");
    }
}
