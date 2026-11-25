//! The `Lines` command, as in Rhino: separate lines in a chain, each starting where the last ended,
//! until Enter.

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
        // A chain, as in Rhino: each line starts where the last ended, until Enter.
        let points = picked(cx);
        let mut steps: Vec<StepDef> = (0..cx.values.len())
            .map(|i| {
                StepDef::point(if i == 0 {
                    "prompt.line.start"
                } else {
                    "prompt.line.end"
                })
            })
            .collect();
        if cx.values.last() == Some(&Value::Empty) {
            return steps;
        }
        steps.push(match points.last() {
            None => StepDef::point("prompt.line.start"),
            Some(last) => {
                let step = StepDef::point("prompt.line.end").from(*last);
                if points.len() >= 2 {
                    step.optional()
                } else {
                    step
                }
            }
        });
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = picked(cx);
        let lines: Vec<Shape> = points
            .windows(2)
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

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn each_line_starts_where_the_last_ended_as_in_rhino() {
        let mut doc = Document::default();
        run(&mut doc, &["lines", "0,0,0", "1,0,0", "0,1,0", "0,1,5", ""]);
        let ends: Vec<_> = doc
            .state()
            .objects
            .iter()
            .map(|o| {
                let points = o.curve().unwrap().sample().unwrap();
                (points[0], points[points.len() - 1])
            })
            .collect();
        assert_eq!(
            ends,
            [
                ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                ([0.0, 1.0, 0.0], [0.0, 1.0, 5.0]),
            ]
        );
    }
}
