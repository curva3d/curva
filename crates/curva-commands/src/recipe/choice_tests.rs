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

/// A count remembered for each style, starting at 2 for Quad and 1 for Triangle, as Rhino's
/// MeshSphere keeps `Subdivisions`.
#[derive(Default)]
struct PerStyle;

const KINDS: [&str; 2] = ["Quad", "Triangle"];

impl Recipe for PerStyle {
    const NAME: &'static str = "perstyletest";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::name("Kind", "Quad"),
        OptionDef::whole("Count", 5, 1, 10).per("Kind", &[("Quad", 2.0), ("Triangle", 1.0)]),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == "Kind").then(|| KINDS.map(str::to_owned).to_vec())
    }

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point("prompt.sphere.centre")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        Ok(Made {
            message: Some(Message::with(
                "done.sphere",
                vec![cx.name("Kind"), cx.number("Count").to_string()],
            )),
            ..Made::default()
        })
    }
}

fn count_after(doc: &mut Document, typed: &[Answer]) -> String {
    let mut command = RecipeCommand::<PerStyle>::default();
    command.step(doc, None);
    for answer in typed {
        command.step(doc, Some(answer.clone()));
    }
    finished_with(&command.step(doc, Some(Answer::Point([0.0; 3]))))
}

#[test]
fn a_count_is_remembered_for_each_style_from_its_own_start() {
    let mut doc = Document::default();
    assert_eq!(count_after(&mut doc, &[]), "Quad,2");
    let triangle = [Answer::Option("Kind"), Answer::Path("Triangle".to_owned())];
    assert_eq!(count_after(&mut doc, &triangle), "Triangle,1");
    let seven = [Answer::Option("Count"), Answer::Number(7.0)];
    assert_eq!(count_after(&mut doc, &seven), "Triangle,7");
    let quad = [Answer::Option("Kind"), Answer::Path("Quad".to_owned())];
    assert_eq!(
        count_after(&mut doc, &quad),
        "Quad,2",
        "Quad's own, untouched"
    );
    assert_eq!(
        count_after(&mut doc, &triangle),
        "Triangle,7",
        "Triangle's remembered"
    );
}
