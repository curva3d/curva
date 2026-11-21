//! A choice of words at a step of its own (`StepDef::choice`), as Rhino asks Fill's patch style.

use super::*;

const STYLE: &str = "Style";
const STYLES: [&str; 3] = ["Automatic", "SingleFace", "Triangulated"];

/// A point, then the style; it makes nothing and says the style it was given.
#[derive(Default)]
struct Styled;

impl Recipe for Styled {
    const NAME: &'static str = "styledtest";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::name(STYLE, "Automatic")];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::point("prompt.sphere.centre"),
            StepDef::choice("prompt.option.name", STYLE, &STYLES),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        Ok(Made {
            message: Some(Message::with("done.sphere", vec![cx.name(STYLE)])),
            ..Made::default()
        })
    }
}

fn asked(step: &Step) -> &Request {
    match step {
        Step::Ask(request) => request,
        _ => panic!("expected a question"),
    }
}

fn finished_with(step: &Step) -> String {
    match step {
        Step::Done(Some(message)) => message.args.join(","),
        _ => panic!("expected the end"),
    }
}

#[test]
fn a_choice_step_asks_its_words_and_enter_keeps_the_one_shown() {
    let mut doc = Document::default();
    let mut command = RecipeCommand::<Styled>::default();
    command.step(&mut doc, None);
    let step = command.step(&mut doc, Some(Answer::Point([0.0; 3])));
    let request = asked(&step);
    assert_eq!(request.kind, Kind::Choice);
    assert_eq!(request.options, STYLES.to_vec());
    assert!(request.allow_empty);
    assert_eq!(request.prompt.args, vec!["Automatic".to_owned()]);
    let done = command.step(&mut doc, Some(Answer::Empty));
    assert_eq!(finished_with(&done), "Automatic");
}

#[test]
fn a_word_picked_answers_the_step_and_is_remembered() {
    let mut doc = Document::default();
    let mut command = RecipeCommand::<Styled>::default();
    command.step(&mut doc, None);
    command.step(&mut doc, Some(Answer::Point([0.0; 3])));
    let done = command.step(&mut doc, Some(Answer::Option("SingleFace")));
    assert_eq!(finished_with(&done), "SingleFace");
    // Shown, and kept by Enter, the next time.
    let mut again = RecipeCommand::<Styled>::default();
    again.step(&mut doc, None);
    let step = again.step(&mut doc, Some(Answer::Point([0.0; 3])));
    assert_eq!(asked(&step).prompt.args, vec!["SingleFace".to_owned()]);
}
