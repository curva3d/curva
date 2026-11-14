//! `Text`: a note, as in Rhino 8 (spec `annotation.md` §2; `rhino-reference.md`): type the text,
//! set its height, alignment or font, then pick where it goes. It lies on the construction plane.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Text as Note, TextAlign};

const HEIGHT: &str = "Height";
const ALIGN: &str = "Alignment";
const FONT: &str = "Font";

/// What the command asks next.
#[derive(Default, PartialEq)]
enum Asking {
    #[default]
    Words,
    Height,
    Font,
    Place,
}

/// The running command.
#[derive(Default)]
pub struct Text {
    asking: Asking,
    words: Option<String>,
    height: Option<f64>,
    align: TextAlign,
    font: Option<String>,
}

/// The style's text height where the text goes: a paper size on a layout, else in the model's
/// unit.
fn usual_height(doc: &Document) -> f64 {
    let style = doc.annotation_style(0);
    match doc.space().and_then(|id| doc.layout(id)) {
        Some(layout) => {
            let millimetres = if style.imperial {
                style.text_height * 25.4
            } else {
                style.text_height
            };
            millimetres / layout.unit.millimetres()
        }
        None => style.height_in(doc.state().unit),
    }
}

impl Text {
    fn ask(&self, doc: &Document) -> Step {
        let align = match self.align {
            TextAlign::Left => "Left",
            TextAlign::Centre => "Centre",
            TextAlign::Right => "Right",
        };
        let height = self.height.unwrap_or_else(|| usual_height(doc));
        let labels = vec![
            format!("{HEIGHT}={}", doc.length_text(height)),
            format!("{ALIGN}={align}"),
            format!("{FONT}={}", self.font.as_deref().unwrap_or("Inter")),
        ];
        let options = vec![HEIGHT, ALIGN, FONT];
        let request = match self.asking {
            Asking::Words => Request::new("prompt.text.words", Kind::Path),
            Asking::Height => Request::new("prompt.text.height", Kind::Number),
            Asking::Font => Request::new("prompt.text.font", Kind::Path),
            Asking::Place => Request::new("prompt.text.place", Kind::Point),
        };
        Step::Ask(Request {
            options,
            option_labels: labels,
            ..request
        })
    }
}

impl Command for Text {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(doc),
            Some(Answer::Option(HEIGHT)) => {
                self.asking = Asking::Height;
                self.ask(doc)
            }
            Some(Answer::Option(ALIGN)) => {
                self.align = match self.align {
                    TextAlign::Left => TextAlign::Centre,
                    TextAlign::Centre => TextAlign::Right,
                    TextAlign::Right => TextAlign::Left,
                };
                self.ask(doc)
            }
            Some(Answer::Option(FONT)) => {
                self.asking = Asking::Font;
                self.ask(doc)
            }
            Some(Answer::Number(height)) if self.asking == Asking::Height => {
                if height > 0.0 {
                    self.height = Some(height);
                }
                self.asking = if self.words.is_some() {
                    Asking::Place
                } else {
                    Asking::Words
                };
                self.ask(doc)
            }
            Some(Answer::Path(font)) if self.asking == Asking::Font => {
                let font = font.trim();
                self.font = (!font.is_empty()).then(|| font.to_owned());
                self.asking = if self.words.is_some() {
                    Asking::Place
                } else {
                    Asking::Words
                };
                self.ask(doc)
            }
            Some(Answer::Path(words)) if self.asking == Asking::Words => {
                // As in Rhino's -Text, \n starts a new line.
                let words = words.replace("\\n", "\n");
                if words.trim().is_empty() {
                    return self.ask(doc);
                }
                self.words = Some(words);
                self.asking = Asking::Place;
                self.ask(doc)
            }
            Some(Answer::Point(at)) if self.asking == Asking::Place => {
                let Some(content) = self.words.take() else {
                    return unexpected();
                };
                let [x, y] = doc.work_plane();
                let height = self.height.unwrap_or_else(|| usual_height(doc));
                doc.add_annotation(Annotation::Text(Note {
                    content,
                    origin: at,
                    x,
                    y,
                    height,
                    font: self.font.clone(),
                    align: self.align,
                    style: 0,
                }));
                Step::Done(Some(Message::new("done.text")))
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::{Annotation, TextAlign};
    use curva_document::{Document, Shape};

    fn only_text(doc: &Document) -> curva_document::annotation::Text {
        match &doc.state().objects[0].shape {
            Shape::Annotation(Annotation::Text(text)) => text.clone(),
            _ => panic!("not text"),
        }
    }

    #[test]
    fn a_note_takes_the_style_height_and_lies_on_the_construction_plane() {
        let mut doc = Document::default();
        run(&mut doc, &["Text", "Hall\\nKitchen", "10,20"]);
        let text = only_text(&doc);
        assert_eq!(text.content, "Hall\nKitchen");
        assert_eq!(text.origin, [10.0, 20.0, 0.0]);
        assert!((text.height - 3.5).abs() < 1e-12);
        assert_eq!(text.x, [1.0, 0.0, 0.0]);
        // Its letters have a size: two lines, the second below the first.
        let (low, high) = doc.state().objects[0].shape.bounds().unwrap();
        assert!(high[0] > 10.0 && low[1] < 20.0 - 3.5);
    }

    #[test]
    fn height_and_alignment_can_be_set_before_placing() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["Text", "Height", "10", "Alignment", "Plan", "0,0"],
        );
        let text = only_text(&doc);
        assert!((text.height - 10.0).abs() < 1e-12);
        assert_eq!(text.align, TextAlign::Centre);
    }

    #[test]
    fn on_a_sheet_the_height_is_a_paper_size() {
        let mut doc = Document::default();
        doc.set_unit(curva_document::Unit::Metres, false);
        run(&mut doc, &["Layout", "", "Text", "Title", "20,20"]);
        let text = only_text(&doc);
        // 3.5 mm on an A3 sheet measured in millimetres, though the model is in metres.
        assert!((text.height - 3.5).abs() < 1e-12);
        assert_eq!(doc.state().objects[0].space, doc.space());
    }
}
