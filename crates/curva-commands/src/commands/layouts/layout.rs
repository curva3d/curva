//! `Layout`: a new paper sheet, as Rhino 8's New Layout (spec `layouts.md` §1). Rhino asks in a
//! dialog; Curva asks for the paper size at the prompt, with the orientation as an option, and
//! opens the new sheet.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::layouts::{PAPER_SIZES, PaperSize, PaperUnit};
use curva_document::{Document, Unit};

const PORTRAIT: &str = "Portrait";
const LANDSCAPE: &str = "Landscape";

/// The paper sizes as option words: their names without spaces (`A3`, `ANSIB`, `ArchD`).
fn size_words() -> Vec<&'static str> {
    const WORDS: [&str; 15] = [
        "A4", "A3", "A2", "A1", "A0", "ANSIA", "ANSIB", "ANSIC", "ANSID", "ANSIE", "ArchA",
        "ArchB", "ArchC", "ArchD", "ArchE",
    ];
    WORDS.to_vec()
}

/// The sheet a model starts with: A3 for a metric model, ANSI B (11 × 17 in) for an imperial one.
fn usual(doc: &Document) -> PaperSize {
    let imperial = matches!(doc.state().unit, Unit::Feet | Unit::Inches);
    let wanted = if imperial {
        PaperUnit::Inches
    } else {
        PaperUnit::Millimetres
    };
    let name = if imperial { "ANSI B" } else { "A3" };
    PAPER_SIZES
        .iter()
        .copied()
        .find(|p| p.name == name && p.unit == wanted)
        .unwrap_or(PAPER_SIZES[1])
}

/// The running command.
pub struct Layout {
    landscape: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self { landscape: true }
    }
}

impl Layout {
    fn ask(&self) -> Step {
        let mut options = size_words();
        options.push(if self.landscape { PORTRAIT } else { LANDSCAPE });
        Step::Ask(Request {
            options,
            allow_empty: true,
            ..Request::new("prompt.layout.size", Kind::Choice)
        })
    }

    fn make(&self, doc: &mut Document, paper: PaperSize) -> Step {
        let id = doc.add_layout(String::new(), paper, self.landscape);
        doc.set_space(Some(id));
        Step::Done(Some(Message::with(
            "done.layout",
            vec![paper.name.to_owned()],
        )))
    }
}

impl Command for Layout {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(PORTRAIT)) => {
                self.landscape = false;
                self.ask()
            }
            Some(Answer::Option(LANDSCAPE)) => {
                self.landscape = true;
                self.ask()
            }
            Some(Answer::Option(word)) => match size_words().iter().position(|w| *w == word) {
                Some(index) => self.make(doc, PAPER_SIZES[index]),
                None => unexpected(),
            },
            Some(Answer::Empty) => {
                let paper = usual(doc);
                self.make(doc, paper)
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn a_layout_is_made_and_opened_on_the_usual_sheet_or_the_one_chosen() {
        let mut doc = Document::default();
        run(&mut doc, &["Layout", ""]);
        let first = &doc.layouts()[0];
        assert_eq!(first.size, [420.0, 297.0]);
        assert_eq!(doc.space(), Some(first.id));
        run(&mut doc, &["Layout", "Portrait", "ANSID"]);
        assert_eq!(doc.layouts()[1].size, [22.0, 34.0]);
    }

    #[test]
    fn an_imperial_model_starts_on_ansi_b() {
        let mut doc = Document::default();
        doc.set_unit(curva_document::Unit::Feet, false);
        run(&mut doc, &["Layout", ""]);
        assert_eq!(doc.layouts()[0].size, [17.0, 11.0]);
    }
}

#[cfg(test)]
mod drawing_on_a_sheet {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn curves_drawn_with_a_layout_open_go_on_its_sheet() {
        let mut doc = Document::default();
        run(&mut doc, &["Layout", "", "Rectangle", "10,10", "110,60"]);
        let sheet = doc.layouts()[0].id;
        assert_eq!(doc.state().objects.len(), 1);
        assert_eq!(doc.state().objects[0].space, Some(sheet));
    }
}
