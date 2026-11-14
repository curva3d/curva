//! `Line`: a straight line between two points, as in Rhino. Option `BothSides` makes the first
//! point the middle of the line.

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Point};

const BOTH_SIDES: &str = "BothSides";

/// The recipe.
#[derive(Default)]
pub struct Line;

impl Recipe for Line {
    const NAME: &'static str = "line";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(BOTH_SIDES, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.line.start")];
        if let Some(start) = cx.values.first().and_then(|v| v.point()) {
            steps.push(StepDef::point("prompt.line.end").from(start).offering(&[]));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(first), Some(second)) = (
            cx.values.first().and_then(|v| v.point()),
            cx.values.get(1).and_then(|v| v.point()),
        ) else {
            return Ok(Made::default());
        };
        let start: Point = if cx.toggle(BOTH_SIDES) {
            std::array::from_fn(|i| 2.0 * first[i] - second[i])
        } else {
            first
        };
        let line = Curve::line(start, second)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(line)],
            message: Some(Message::new("done.line")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn makes_a_line_of_the_right_length() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0,0", "3,4,0"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 5.0).abs() < 1e-9);
    }

    #[test]
    fn identical_points_make_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["l", "1,1", "1,1"]);
        assert!(doc.state().objects.is_empty());
        assert!(!processor.is_running());
    }

    #[test]
    fn both_sides_starts_from_the_middle_and_is_remembered() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["line", "bothsides"]);
        assert!(processor.prompt().unwrap().contains("BothSides=Yes"));
        run(&mut doc, &["line", "0,0", "3,4"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert_eq!(points[0], [-3.0, -4.0, 0.0]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 10.0).abs() < 1e-9);
    }
}
