//! The `TextObject` command.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Body, Point};

/// `TextObject`: text as curves (the default), flat surfaces or solids: the text, where it
/// starts, its height; `Font` names a system font (Curva's own type otherwise).
#[derive(Default)]
pub struct TextObject {
    text: Option<String>,
    at: Option<Point>,
    output: u8,
    font: Option<String>,
    choosing_font: bool,
}

impl TextObject {
    fn ask_text(&self) -> Step {
        Step::Ask(Request {
            options: vec!["Curves", "Surfaces", "Solids", "Font"],
            ..Request::new(
                if self.choosing_font {
                    "prompt.textobject.font"
                } else {
                    "prompt.textobject.text"
                },
                Kind::Path,
            )
        })
    }
}

impl Command for TextObject {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask_text(),
            Some(Answer::Option(word)) => {
                match word {
                    "Curves" => self.output = 0,
                    "Surfaces" => self.output = 1,
                    "Solids" => self.output = 2,
                    _ => self.choosing_font = true,
                }
                self.ask_text()
            }
            Some(Answer::Path(text)) if self.choosing_font => {
                self.font = Some(text.trim().to_owned());
                self.choosing_font = false;
                self.ask_text()
            }
            Some(Answer::Path(text)) if self.text.is_none() => {
                self.text = Some(text);
                Step::Ask(Request::new("prompt.textobject.point", Kind::Point))
            }
            Some(Answer::Point(p)) if self.at.is_none() => {
                self.at = Some(p);
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.textobject.height", Kind::Number)
                })
            }
            Some(Answer::Number(_) | Answer::Empty) => {
                let height = match answer {
                    Some(Answer::Number(h)) if h > 0.0 => h,
                    _ => 10.0,
                };
                let font = curva_kernel::font_bytes(self.font.as_deref());
                let at = self.at.unwrap_or([0.0; 3]);
                let glyphs = match curva_kernel::text_outlines(
                    self.text.as_deref().unwrap_or_default(),
                    &font,
                    height,
                    at,
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                ) {
                    Ok(glyphs) => glyphs,
                    Err(e) => {
                        return Step::Failed(Message::with("error.geometry", vec![e.to_string()]));
                    }
                };
                let mut added = Vec::new();
                for glyph in glyphs {
                    match self.output {
                        0 => {
                            for curve in glyph {
                                added.push(doc.add(curve));
                            }
                        }
                        kind => {
                            // Each outline with the holes inside it, filled.
                            let made = if kind == 1 {
                                Body::planar(&glyph)
                            } else {
                                Body::extruded(&glyph, [0.0, 0.0, 1.0], height / 5.0, true)
                            };
                            if let Ok(body) = made {
                                added.push(doc.add_body(body));
                            }
                        }
                    }
                }
                doc.select(&added);
                Step::Done(Some(Message::with(
                    "done.textobject",
                    vec![added.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}
