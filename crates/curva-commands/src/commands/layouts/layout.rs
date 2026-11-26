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

/// The views of a sheet's details, by how many there are, in Rhino's places: one Top; four as
/// Top and Perspective above, Front and Right below.
const DETAIL_VIEWS: [&str; 4] = ["Top", "Perspective", "Front", "Right"];

/// How far the details sit in from the sheet's edges and from each other, in millimetres.
const DETAIL_MARGIN_MM: f64 = 10.0;

/// The running command.
pub struct Layout {
    landscape: bool,
    /// The paper chosen, while it asks how many details (Rhino's Detail Count, #687).
    paper: Option<PaperSize>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            landscape: true,
            paper: None,
        }
    }
}

/// `count` details (up to four) on the open sheet `layout`, in a grid within its margins.
fn add_details(doc: &mut Document, layout: u64, count: usize) {
    let Some(sheet) = doc.layout(layout) else {
        return;
    };
    let [w, h] = sheet.size;
    let margin = DETAIL_MARGIN_MM / sheet.unit.millimetres();
    let (columns, rows) = match count {
        0 => return,
        1 => (1, 1),
        2 => (2, 1),
        _ => (2, 2),
    };
    let cell = [
        (w - margin * (columns as f64 + 1.0)) / columns as f64,
        (h - margin * (rows as f64 + 1.0)) / rows as f64,
    ];
    for (i, view) in DETAIL_VIEWS.iter().take(count.min(4)).enumerate() {
        let (column, row) = (i % columns, i / columns);
        // Rows from the top of the sheet down.
        let x = margin + column as f64 * (cell[0] + margin);
        let y = h - (row as f64 + 1.0) * (cell[1] + margin);
        let a = [x, y, 0.0];
        let b = [x + cell[0], y + cell[1], 0.0];
        super::detail::framed_detail(doc, layout, a, b, view);
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

    /// Asks how many details the new sheet starts with, one by default, as Rhino does.
    fn ask_details(&mut self, paper: PaperSize) -> Step {
        self.paper = Some(paper);
        Step::Ask(Request {
            allow_empty: true,
            ..Request::new("prompt.layout.details", Kind::Number)
        })
    }

    fn make(&self, doc: &mut Document, paper: PaperSize, details: usize) -> Step {
        // Named Page 1, Page 2 and on, as Rhino names new layouts.
        let name = format!("Page {}", doc.layouts().len() + 1);
        let id = doc.add_layout(name, paper, self.landscape);
        doc.set_space(Some(id));
        add_details(doc, id, details);
        Step::Done(Some(Message::with(
            "done.layout",
            vec![paper.name.to_owned()],
        )))
    }
}

impl Command for Layout {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if let Some(paper) = self.paper {
            return match answer {
                Some(Answer::Empty) => self.make(doc, paper, 1),
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Some(Answer::Number(n)) if (0.0..=4.0).contains(&n) => {
                    self.make(doc, paper, n.round() as usize)
                }
                Some(Answer::Number(_)) => Step::Failed(Message::new("error.layout_details")),
                _ => unexpected(),
            };
        }
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
                Some(index) => self.ask_details(PAPER_SIZES[index]),
                None => unexpected(),
            },
            Some(Answer::Empty) => {
                let paper = usual(doc);
                self.ask_details(paper)
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
        run(&mut doc, &["Layout", "", "0"]);
        let first = &doc.layouts()[0];
        assert_eq!(first.size, [420.0, 297.0]);
        assert_eq!(doc.space(), Some(first.id));
        run(&mut doc, &["Layout", "Portrait", "ANSID", "0"]);
        assert_eq!(doc.layouts()[1].size, [22.0, 34.0]);
    }

    #[test]
    fn a_new_sheet_is_named_and_starts_with_one_detail_as_in_rhino() {
        let mut doc = Document::default();
        doc.add(curva_kernel::Curve::line([0.0; 3], [100.0, 50.0, 0.0]).unwrap());
        // Rhino 8: Page 1, with one Top detail showing the model; four make a four-view sheet.
        run(&mut doc, &["Layout", "", ""]);
        let first = &doc.layouts()[0];
        assert_eq!(first.name, "Page 1");
        assert_eq!(first.details.len(), 1);
        assert_eq!(first.details[0].projection, "Top");
        run(&mut doc, &["Layout", "", "4"]);
        let second = &doc.layouts()[1];
        assert_eq!(second.name, "Page 2");
        let views: Vec<&str> = second
            .details
            .iter()
            .map(|d| d.projection.as_str())
            .collect();
        assert_eq!(views, ["Top", "Perspective", "Front", "Right"]);
    }

    #[test]
    fn an_imperial_model_starts_on_ansi_b() {
        let mut doc = Document::default();
        doc.set_unit(curva_document::Unit::Feet, false);
        run(&mut doc, &["Layout", "", "0"]);
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
        run(
            &mut doc,
            &["Layout", "", "0", "Rectangle", "10,10", "110,60"],
        );
        let sheet = doc.layouts()[0].id;
        assert_eq!(doc.state().objects.len(), 1);
        assert_eq!(doc.state().objects[0].space, Some(sheet));
    }
}
