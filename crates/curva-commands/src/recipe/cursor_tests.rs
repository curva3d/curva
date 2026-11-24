//! Where the pointer is when a number is typed (`Context::cursor`), so a typed amount goes on
//! the side the pointer is on, as Rhino's InsertEdge puts its new edges.

use super::*;

/// An amount, typed or clicked; a guide from the origin that far along X, on the pointer's side.
#[derive(Default)]
struct Sided;

/// The amount the answer gives, signed by the side of X the pointer is on.
fn reach(cx: &Context<'_>) -> Option<f64> {
    let amount = match cx.values.first()? {
        Value::Number(amount) => *amount,
        Value::Point(at) => at[0].abs(),
        _ => return None,
    };
    let side = cx.cursor().map_or(1.0, |at| at[0].signum());
    Some(amount * side)
}

impl Recipe for Sided {
    const NAME: &'static str = "sidedtest";
    const OPTIONS: &'static [OptionDef] = &[];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point_or_number("prompt.option.name")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let reach = reach(cx).ok_or_else(|| Message::new("error.unexpected"))?;
        Ok(Made {
            guides: Curve::line([0.0; 3], [reach, 0.0, 0.0])
                .into_iter()
                .collect(),
            message: Some(Message::with("done.sphere", vec![reach.to_string()])),
            ..Made::default()
        })
    }
}

/// Where the preview's guide ends, along X.
fn previewed(command: &RecipeCommand<Sided>, doc: &Document, live: &Live) -> f64 {
    let shown = command.preview(doc, live);
    match shown.first() {
        Some(Curve::Line { end, .. }) => end[0],
        _ => 0.0,
    }
}

#[test]
fn a_typed_amount_goes_on_the_side_the_pointer_is_on() {
    let mut doc = Document::default();
    let mut command = RecipeCommand::<Sided>::default();
    command.step(&mut doc, None);
    // The preview follows the pointer while the number is typed.
    let left = Live {
        cursor: Some([-3.0, 1.0, 0.0]),
        number: Some(2.0),
    };
    assert!((previewed(&command, &doc, &left) + 2.0).abs() < 1e-9);
    let right = Live {
        cursor: Some([3.0, 1.0, 0.0]),
        number: Some(2.0),
    };
    assert!((previewed(&command, &doc, &right) - 2.0).abs() < 1e-9);
    // Entered with the pointer on the left, the result is on the left.
    command.note_cursor(Some([-3.0, 1.0, 0.0]));
    match command.step(&mut doc, Some(Answer::Number(2.0))) {
        Step::Done(Some(message)) => assert_eq!(message.args, vec!["-2".to_owned()]),
        _ => panic!("expected the end"),
    }
}

#[test]
fn an_amount_given_without_the_pointer_has_none() {
    let mut doc = Document::default();
    let mut command = RecipeCommand::<Sided>::default();
    command.step(&mut doc, None);
    command.note_cursor(None);
    match command.step(&mut doc, Some(Answer::Number(2.0))) {
        Step::Done(Some(message)) => assert_eq!(message.args, vec!["2".to_owned()]),
        _ => panic!("expected the end"),
    }
}
