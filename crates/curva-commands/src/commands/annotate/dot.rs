//! `Dot`, as in Rhino 8 (spec `annotation.md` §4; `rhino-reference.md`): type the words, then
//! pick where the dot goes. Its height, in points on screen, can be set first.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Dot as Made};

const HEIGHT: &str = "Height";

/// The running command.
#[derive(Default)]
pub struct Dot {
    words: Option<String>,
    height: Option<f64>,
    asking_height: bool,
}

impl Dot {
    fn ask(&self) -> Step {
        let height = self.height.unwrap_or_else(Made::usual_height);
        let request = if self.asking_height {
            Request::new("prompt.dot.height", Kind::Number)
        } else if self.words.is_none() {
            Request::new("prompt.dot.words", Kind::Path)
        } else {
            Request::new("prompt.dot.place", Kind::Point)
        };
        Step::Ask(Request {
            options: vec![HEIGHT],
            option_labels: vec![format!("{HEIGHT}={height}")],
            ..request
        })
    }
}

impl Command for Dot {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(HEIGHT)) => {
                self.asking_height = true;
                self.ask()
            }
            Some(Answer::Number(height)) if self.asking_height => {
                if height > 0.0 {
                    self.height = Some(height);
                }
                self.asking_height = false;
                self.ask()
            }
            Some(Answer::Path(words)) if self.words.is_none() => {
                let words = words.replace("\\n", "\n");
                if !words.trim().is_empty() {
                    self.words = Some(words);
                }
                self.ask()
            }
            Some(Answer::Point(at)) if self.words.is_some() => {
                let Some(content) = self.words.take() else {
                    return unexpected();
                };
                doc.add_annotation(Annotation::Dot(Made {
                    at,
                    content,
                    secondary: String::new(),
                    height: self.height.unwrap_or_else(Made::usual_height),
                    font: None,
                }));
                Step::Done(Some(Message::new("done.dot")))
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};

    #[test]
    fn a_dot_labels_a_point_at_a_screen_height() {
        let mut doc = Document::default();
        run(&mut doc, &["Dot", "Height", "20", "A1", "5,6,7"]);
        let Shape::Annotation(Annotation::Dot(dot)) = &doc.state().objects[0].shape else {
            panic!("not a dot");
        };
        assert_eq!(dot.content, "A1");
        assert_eq!(dot.at, [5.0, 6.0, 7.0]);
        assert!((dot.height - 20.0).abs() < 1e-12);
        // In the model it is its point: its extent, and what moving it moves.
        assert_eq!(
            doc.state().objects[0].shape.bounds(),
            Some(([5.0, 6.0, 7.0], [5.0, 6.0, 7.0]))
        );
        let moved = doc.state().objects[0].shape.translated([1.0, 0.0, 0.0]);
        assert_eq!(moved.bounds().unwrap().0, [6.0, 6.0, 7.0]);
    }
}
