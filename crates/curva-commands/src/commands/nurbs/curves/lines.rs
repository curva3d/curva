//! The `Lines` command.

use crate::Message;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Curve;

/// `Lines`: separate lines, each from its own two points, until Enter.
#[derive(Default)]
pub struct Lines;

impl Recipe for Lines {
    const NAME: &'static str = "lines";
    const PLACES_AS_IT_GOES: bool = true;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let points = picked(cx);
        let mut steps: Vec<StepDef> = Vec::new();
        for (i, _) in cx.values.iter().enumerate() {
            steps.push(StepDef::point(if i % 2 == 0 {
                "prompt.line.start"
            } else {
                "prompt.line.end"
            }));
        }
        if cx.values.last() == Some(&Value::Empty) {
            return steps;
        }
        if points.len() % 2 == 1 {
            steps.push(StepDef::point("prompt.line.end").from(points[points.len() - 1]));
        } else {
            let step = StepDef::point("prompt.line.start");
            steps.push(if points.is_empty() {
                step
            } else {
                step.optional()
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = picked(cx);
        let lines: Vec<Shape> = points
            .chunks(2)
            .filter(|pair| pair.len() == 2)
            .filter_map(|pair| Curve::line(pair[0], pair[1]).ok().map(Shape::Curve))
            .collect();
        let count = lines.len();
        Ok(Made {
            add: lines,
            message: Some(Message::with("done.lines", vec![count.to_string()])),
            ..Made::default()
        })
    }
}
