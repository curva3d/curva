//! `Join`, as in Rhino: makes curves that meet end to end into one curve. Rhino's `Undo` while
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
        vec![StepDef::objects("prompt.join.objects").curves().at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let (ids, curves): (Vec<u64>, Vec<Curve>) = curves_of(cx.doc, chosen).into_iter().unzip();
        let made =
            join(&curves).map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        if made.is_empty() {
            return Err(Message::new("error.nothing_to_join"));
        }
        let mut result = Made::default();
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
