//! The `ArrayLinear` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_kernel::sub;

/// `ArrayLinear`: copies objects in a row: how many, then two points for the spacing and way.
#[derive(Default)]
pub struct ArrayLinear;

impl Recipe for ArrayLinear {
    const NAME: &'static str = "arraylinear";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.arraylinear.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.arraylinear.count"));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::point("prompt.arraylinear.first"));
        }
        if let Some(a) = point(cx, 2) {
            steps.push(StepDef::point("prompt.arraylinear.second").from(a));
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(count) = number(cx, 1)
            && !(count >= 2.0 && count.fract() == 0.0 && count <= 10_000.0)
        {
            return Err(Message::new("error.bad_items"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(count), Some(a), Some(b)) =
            (cx.values.first(), number(cx, 1), point(cx, 2), point(cx, 3))
        else {
            return Ok(Made::default());
        };
        let step = sub(b, a);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let changes: Vec<Change> = (1..count as usize)
            .map(|i| Change::Move(step.map(|v| v * i as f64)))
            .collect();
        let mut made = changed(cx, ids, &changes, true, |_, _| None);
        made.message = Some(Message::with(
            "done.array",
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
    fn array_linear_makes_a_row() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        run(&mut doc, &["arraylinear", "4", "0,0", "0,5"]);
        assert_eq!(doc.state().objects.len(), 4);
    }
}
