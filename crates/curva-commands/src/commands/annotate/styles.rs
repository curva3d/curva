//! Annotation style commands (spec `annotation.md` §1; `rhino-annotation-styles.md`):
//! `AnnotationStyle` opens the styles' panel; `SetCurrentAnnotationStyle` and
//! `DupAnnotationStyle` are Rhino's.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::Document;

/// `AnnotationStyle` (Rhino's DimStyle): opens the Annotation Styles panel.
#[derive(Default)]
pub struct AnnotationStyle;

impl Command for AnnotationStyle {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowAnnotationStyles)
    }
}

/// The style named `name`, ignoring case.
fn style_named(doc: &Document, name: &str) -> Option<u64> {
    doc.annotation_styles()
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
        .map(|s| s.id)
}

/// The styles' names, for a message.
fn names(doc: &Document) -> String {
    doc.annotation_styles()
        .iter()
        .map(|s| s.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `SetCurrentAnnotationStyle`: the style new annotation takes, by name.
#[derive(Default)]
pub struct SetCurrentAnnotationStyle;

impl Command for SetCurrentAnnotationStyle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.annotationstyle.current", Kind::Path)),
            Some(Answer::Path(name)) => match style_named(doc, &name) {
                Some(id) => {
                    doc.set_current_annotation_style(id);
                    Step::Done(Some(Message::with(
                        "done.annotationstyle.current",
                        vec![doc.annotation_style(id).name],
                    )))
                }
                None => Step::Failed(Message::with(
                    "error.annotationstyle.none",
                    vec![name, names(doc)],
                )),
            },
            Some(_) => unexpected(),
        }
    }
}

const SET_CURRENT: &str = "SetCurrent";

/// `DupAnnotationStyle`: the style to copy, the new style's name, and whether it becomes the
/// current one.
#[derive(Default)]
pub struct DupAnnotationStyle {
    from: Option<u64>,
    set_current: bool,
}

impl DupAnnotationStyle {
    fn ask_name(&self) -> Step {
        Step::Ask(Request {
            options: vec![SET_CURRENT],
            option_labels: vec![format!(
                "{SET_CURRENT}={}",
                if self.set_current { "Yes" } else { "No" }
            )],
            ..Request::new("prompt.annotationstyle.new_name", Kind::Path)
        })
    }
}

impl Command for DupAnnotationStyle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.from, answer) {
            (_, None) => Step::Ask(Request::new("prompt.annotationstyle.copy", Kind::Path)),
            (None, Some(Answer::Path(name))) => match style_named(doc, &name) {
                Some(id) => {
                    self.from = Some(id);
                    self.ask_name()
                }
                None => Step::Failed(Message::with(
                    "error.annotationstyle.none",
                    vec![name, names(doc)],
                )),
            },
            (Some(_), Some(Answer::Option(SET_CURRENT))) => {
                self.set_current = !self.set_current;
                self.ask_name()
            }
            (Some(from), Some(Answer::Path(name))) => {
                let name = name.trim().to_owned();
                if name.is_empty() || style_named(doc, &name).is_some() {
                    return Step::Failed(Message::with("error.annotationstyle.name", vec![name]));
                }
                let id = doc.copy_annotation_style(from, name.clone());
                if self.set_current {
                    doc.set_current_annotation_style(id);
                }
                Step::Done(Some(Message::with(
                    "done.annotationstyle.copied",
                    vec![name],
                )))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn a_style_is_copied_and_made_current_by_name() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["DupAnnotationStyle", "iso 5", "SetCurrent", "Titles"],
        );
        let titles = doc
            .annotation_styles()
            .iter()
            .find(|s| s.name == "Titles")
            .map(|s| s.id)
            .unwrap();
        assert_eq!(doc.current_annotation_style(), titles);
        assert!((doc.annotation_style(titles).text_height - 5.0).abs() < 1e-12);
        run(&mut doc, &["SetCurrentAnnotationStyle", "ISO 2.5"]);
        assert_eq!(
            doc.annotation_style(doc.current_annotation_style()).name,
            "ISO 2.5"
        );
    }

    #[test]
    fn new_text_takes_the_current_style() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["SetCurrentAnnotationStyle", "ISO 7", "Text", "Big", "0,0"],
        );
        let curva_document::Shape::Annotation(curva_document::annotation::Annotation::Text(text)) =
            &doc.state().objects[0].shape
        else {
            panic!("text");
        };
        assert!((text.height - 7.0).abs() < 1e-12);
        assert_eq!(text.style, doc.current_annotation_style());
    }
}
