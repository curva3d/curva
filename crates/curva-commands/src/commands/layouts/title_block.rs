//! `TitleBlock`: a drawing border and title block on the open sheet (spec `layouts.md` §4),
//! Serpentine3D's command, as Rhino has none. Everything placed is ordinary lines and text,
//! grouped, so it can be moved, edited or exploded as Rhino users do with their own title blocks.
//! Its values are Rhino text fields, so they fill themselves in: the project and who drew it
//! from the document's user text (`Project`, `DrawnBy`, `CheckedBy`), the drawing title and
//! revision from the sheet's (`Title`, `Revision`), the scale of the sheet's main detail, the
//! date, the file name and the sheet number. A revision table sits on top, filled by `Revision`.
//!
//! Layouts follow ISO 5457 (a 20 mm filing margin on the left, 10 mm elsewhere) and an ISO 7200
//! title block 180 mm wide; the ANSI one is the same block 6.25 inches wide in a half-inch border
//! (ASME Y14.1).

use super::super::organise::set_document_user_text::typed_text;
use crate::commands::unexpected;
use crate::text::Translations;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Text, TextAlign};
use curva_document::layouts::PaperUnit;
use curva_document::sheet::{Table, TableKind};
use curva_kernel::{Curve, Point};

const STANDARD: &str = "Standard";

/// The title block's width in its own units (millimetres for ISO).
const WIDE: f64 = 180.0;
/// Its height: four rows of 9.
const TALL: f64 = 36.0;

/// What the command asks next.
#[derive(Clone, Copy, Default, PartialEq)]
enum Asking {
    #[default]
    Project,
    Title,
    DrawnBy,
}

/// The running command: the project, the drawing title and who drew it, each with Enter keeping
/// what the document already says; `Standard` switches between ISO and ANSI.
#[derive(Default)]
pub struct TitleBlock {
    asking: Asking,
    ansi: Option<bool>,
}

/// A cell of the block: its corners in block units, its label's key, its value (text with
/// fields) and the value's height in block units.
struct Cell {
    low: [f64; 2],
    high: [f64; 2],
    label: &'static str,
    value: String,
    height: f64,
}

/// What the title block places, in paper units: lines, then text.
struct Placed {
    lines: Vec<Curve>,
    texts: Vec<Text>,
    table: Table,
}

/// A layout's name as its tab shows it: its own, or `Layout 2` for the second if it has none.
fn shown_name(doc: &Document, id: u64) -> String {
    let layouts = doc.layouts();
    let Some(index) = layouts.iter().position(|l| l.id == id) else {
        return String::new();
    };
    if layouts[index].name.is_empty() {
        Translations::english().render(&Message::with(
            "ui.layout.unnamed",
            vec![(index + 1).to_string()],
        ))
    } else {
        layouts[index].name.clone()
    }
}

impl TitleBlock {
    fn ansi(&self, doc: &Document) -> bool {
        self.ansi.unwrap_or_else(|| {
            doc.space()
                .and_then(|id| doc.layout(id))
                .is_some_and(|l| l.unit == PaperUnit::Inches)
        })
    }

    /// Everything the block places on the open sheet.
    fn placed(&self, doc: &Document) -> Option<Placed> {
        let layout = doc.layout(doc.space()?)?;
        let ansi = self.ansi(doc);
        let per_mm = 1.0 / layout.unit.millimetres();
        // Paper units per block unit, and the border's margins in paper units.
        let k = if ansi { 6.25 * 25.4 / WIDE } else { 1.0 } * per_mm;
        let (left, other) = if ansi { (12.7, 12.7) } else { (20.0, 10.0) };
        let [w, h] = layout.size;
        let (left, other) = (left * per_mm, other * per_mm);
        // The block's lower left corner, in the border's lower right corner.
        let corner = [w - other - WIDE * k, other];
        let lines = sheet_lines([w, h], left, other, corner, k);
        let scale = main_detail(doc, layout).map_or_else(
            || "-".to_owned(),
            |frame| {
                let format = if ansi { "#=1" } else { "1:#" };
                format!("%<DetailScale(\"{frame}\",\"{format}\")>%")
            },
        );
        let date = if ansi { "M/d/yyyy" } else { "yyyy-MM-dd" };
        let style = doc.current_annotation_style();
        let text = |content: String, u: f64, v: f64, height: f64| Text {
            content,
            origin: [corner[0] + u * k, corner[1] + v * k, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height: height * k,
            font: None,
            align: TextAlign::Left,
            style,
        };
        let words = Translations::english();
        let say = |key: &'static str| words.render(&Message::new(key));
        let texts = cell_texts(&cells(scale, date), &text, &say);
        let table = Table {
            at: text(String::new(), 0.0, TALL, 2.5),
            kind: TableKind::Revisions(layout.id),
            headings: vec![
                say("sheet.heading.mark"),
                say("sheet.heading.date"),
                say("sheet.heading.description"),
                say("sheet.heading.by"),
            ],
            title: None,
            width: Some(WIDE * k),
            unnamed: None,
        };
        Some(Placed {
            lines,
            texts,
            table,
        })
    }

    fn ask(&self, doc: &Document) -> Step {
        let state = doc.state();
        let sheet = doc.space().and_then(|id| doc.layout(id));
        let (key, now) = match self.asking {
            Asking::Project => (
                "prompt.titleblock.project",
                state.document_text.get("Project").cloned(),
            ),
            Asking::Title => (
                "prompt.titleblock.title",
                sheet.map(|l| {
                    l.user_text
                        .get("Title")
                        .cloned()
                        .unwrap_or_else(|| shown_name(doc, l.id))
                }),
            ),
            Asking::DrawnBy => (
                "prompt.titleblock.drawn_by",
                state.document_text.get("DrawnBy").cloned(),
            ),
        };
        let standard = if self.ansi(doc) { "ANSI" } else { "ISO" };
        Step::Ask(Request {
            prompt: match now.filter(|n| !n.is_empty()) {
                Some(now) => Message::with(key, vec![now]),
                None => Message::new(match self.asking {
                    Asking::Project => "prompt.titleblock.project_new",
                    Asking::Title => "prompt.titleblock.title_new",
                    Asking::DrawnBy => "prompt.titleblock.drawn_by_new",
                }),
            },
            allow_empty: true,
            options: vec![STANDARD],
            option_labels: vec![format!("{STANDARD}={standard}")],
            ..Request::new(key, Kind::Path)
        })
    }
}

impl Command for TitleBlock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(layout) = doc.space() else {
            return Step::Failed(Message::new("error.sheet_only"));
        };
        let answer = match answer {
            None => return self.ask(doc),
            Some(Answer::Option(STANDARD)) => {
                self.ansi = Some(!self.ansi(doc));
                return self.ask(doc);
            }
            Some(answer @ (Answer::Path(_) | Answer::Empty)) => typed_text(Some(answer)),
            Some(_) => return unexpected(),
        };
        let typed = answer.trim();
        match self.asking {
            Asking::Project => {
                if !typed.is_empty() {
                    doc.set_document_text("Project", typed);
                }
                self.asking = Asking::Title;
                self.ask(doc)
            }
            Asking::Title => {
                let title = if typed.is_empty() {
                    doc.layout(layout).map(|l| {
                        l.user_text
                            .get("Title")
                            .cloned()
                            .unwrap_or_else(|| shown_name(doc, l.id))
                    })
                } else {
                    Some(typed.to_owned())
                };
                if let Some(title) = title.filter(|t| !t.is_empty()) {
                    doc.set_layout_text(layout, "Title", &title);
                }
                self.asking = Asking::DrawnBy;
                self.ask(doc)
            }
            Asking::DrawnBy => {
                if !typed.is_empty() {
                    doc.set_document_text("DrawnBy", typed);
                }
                let Some(placed) = self.placed(doc) else {
                    return unexpected();
                };
                let mut ids: Vec<u64> = placed.lines.into_iter().map(|c| doc.add(c)).collect();
                for text in placed.texts {
                    ids.push(doc.add_annotation(Annotation::Text(text)));
                }
                ids.push(doc.add_annotation(Annotation::Table(placed.table)));
                doc.group(&ids);
                Step::Done(Some(Message::new("done.titleblock")))
            }
        }
    }

    fn preview(&self, doc: &Document, _live: &Live) -> Vec<Curve> {
        self.placed(doc).map(|p| p.lines).unwrap_or_default()
    }
}

/// The border inside the sheet's margins, and the block's frame and cell lines, its lower left
/// corner at `corner`, `k` paper units to a block unit.
fn sheet_lines(size: [f64; 2], left: f64, other: f64, corner: [f64; 2], k: f64) -> Vec<Curve> {
    let [w, h] = size;
    let point = |x: f64, y: f64| -> Point { [x, y, 0.0] };
    let mut lines = Vec::new();
    let border = [
        point(left, other),
        point(w - other, other),
        point(w - other, h - other),
        point(left, h - other),
    ];
    lines.extend(Curve::polyline(border.to_vec(), true));
    let at = |u: f64, v: f64| point(corner[0] + u * k, corner[1] + v * k);
    let mut line = |a: [f64; 2], b: [f64; 2]| {
        lines.extend(Curve::line(at(a[0], a[1]), at(b[0], b[1])));
    };
    line([0.0, 0.0], [0.0, TALL]);
    line([0.0, TALL], [WIDE, TALL]);
    for v in [9.0, 27.0] {
        line([0.0, v], [WIDE, v]);
    }
    line([110.0, 18.0], [WIDE, 18.0]);
    line([110.0, 0.0], [110.0, 27.0]);
    line([145.0, 0.0], [145.0, 27.0]);
    lines
}

/// The main detail on the sheet: the largest scaled one, by its frame.
fn main_detail(doc: &Document, layout: &curva_document::layouts::Layout) -> Option<u64> {
    layout
        .details
        .iter()
        .filter(|d| d.scale.is_some())
        .filter_map(|d| {
            let (low, high) = doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == d.frame)?
                .shape
                .bounds()?;
            Some((d.frame, (high[0] - low[0]) * (high[1] - low[1])))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(frame, _)| frame)
}

/// The block's cells: where each lies, its label, and the field that fills it.
fn cells(scale: String, date: &str) -> [Cell; 9] {
    [
        Cell {
            low: [0.0, 27.0],
            high: [WIDE, TALL],
            label: "titleblock.project",
            value: "%<DocumentText(\"Project\")>%".to_owned(),
            height: 3.0,
        },
        Cell {
            low: [0.0, 9.0],
            high: [110.0, 27.0],
            label: "titleblock.title",
            value: "%<LayoutUserText(\"Title\")>%".to_owned(),
            height: 5.0,
        },
        Cell {
            low: [110.0, 18.0],
            high: [145.0, 27.0],
            label: "titleblock.scale",
            value: scale,
            height: 2.5,
        },
        Cell {
            low: [145.0, 18.0],
            high: [WIDE, 27.0],
            label: "titleblock.date",
            value: format!("%<Date(\"{date}\")>%"),
            height: 2.5,
        },
        Cell {
            low: [110.0, 9.0],
            high: [145.0, 18.0],
            label: "titleblock.drawn_by",
            value: "%<DocumentText(\"DrawnBy\")>%".to_owned(),
            height: 2.5,
        },
        Cell {
            low: [145.0, 9.0],
            high: [WIDE, 18.0],
            label: "titleblock.checked_by",
            value: "%<DocumentText(\"CheckedBy\")>%".to_owned(),
            height: 2.5,
        },
        Cell {
            low: [0.0, 0.0],
            high: [110.0, 9.0],
            label: "titleblock.file",
            value: "%<FileName()>%".to_owned(),
            height: 2.5,
        },
        Cell {
            low: [110.0, 0.0],
            high: [145.0, 9.0],
            label: "titleblock.sheet",
            value: "%<PageNumber()>% / %<NumPages()>%".to_owned(),
            height: 2.5,
        },
        Cell {
            low: [145.0, 0.0],
            high: [WIDE, 9.0],
            label: "titleblock.revision",
            value: "%<LayoutUserText(\"Revision\")>%".to_owned(),
            height: 2.5,
        },
    ]
}

/// Each cell's label in its top left corner and its value in its bottom left corner.
fn cell_texts(
    cells: &[Cell],
    text: &impl Fn(String, f64, f64, f64) -> Text,
    say: &impl Fn(&'static str) -> String,
) -> Vec<Text> {
    let label_height = 1.8;
    let mut texts = Vec::new();
    for cell in cells {
        texts.push(text(
            say(cell.label),
            cell.low[0] + 1.5,
            cell.high[1] - 1.5 - label_height,
            label_height,
        ));
        texts.push(text(
            cell.value.clone(),
            cell.low[0] + 1.5,
            cell.low[1] + 1.5,
            cell.height,
        ));
    }
    texts
}

/// Whether an object is part of a title block's text, for tests.
#[cfg(test)]
fn texts(doc: &Document) -> Vec<String> {
    use curva_document::Shape;
    doc.state()
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Text(t)) => Some(t.content.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::texts;
    use crate::commands::run;
    use crate::{Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::fields::{FieldContext, resolve};
    use curva_document::{Document, Shape};

    #[test]
    fn a_title_block_fills_itself_from_the_document_and_the_sheet() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &[
                "Layout", "", "0", "Detail", "40,60", "300,250", "Layout", "", "0",
            ],
        );
        let first = doc.layouts()[0].id;
        doc.set_space(Some(first));
        run(
            &mut doc,
            &["TitleBlock", "Harbour Library", "Ground floor plan", "LV"],
        );
        let state = doc.state();
        assert_eq!(state.document_text["Project"], "Harbour Library");
        assert_eq!(state.document_text["DrawnBy"], "LV");
        assert_eq!(doc.layouts()[0].user_text["Title"], "Ground floor plan");
        let shown: Vec<String> = texts(&doc)
            .iter()
            .map(|t| resolve(t, doc.state(), Some(first), &FieldContext::default()))
            .collect();
        for wanted in ["Harbour Library", "Ground floor plan", "LV", "1 / 2"] {
            assert!(shown.iter().any(|s| s == wanted), "{wanted} in {shown:?}");
        }
        assert!(
            shown.iter().any(|s| s.starts_with("1:")),
            "the main detail's scale: {shown:?}"
        );
        // Everything is one group, with a revision table.
        let placed: Vec<_> = doc
            .state()
            .objects
            .iter()
            .filter(|o| o.space == Some(first) && o.group.is_some())
            .collect();
        assert!(placed.len() > 20);
        assert!(
            placed
                .iter()
                .any(|o| matches!(o.shape, Shape::Annotation(Annotation::Table(_))))
        );
        // The border sits 20 mm from the left of the A3 sheet and 10 mm from the others (text
        // holding fields is measured as typed, so only the lines are).
        let (low, high) = placed
            .iter()
            .filter(|o| matches!(o.shape, Shape::Curve(_)))
            .filter_map(|o| o.shape.bounds())
            .fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), (a, b)| {
                (
                    std::array::from_fn(|i| lo[i].min(a[i])),
                    std::array::from_fn(|i| hi[i].max(b[i])),
                )
            });
        assert!((low[0] - 20.0).abs() < 1e-9 && (low[1] - 10.0).abs() < 1e-9);
        assert!((high[0] - 410.0).abs() < 1e-9 && (high[1] - 287.0).abs() < 1e-9);
    }

    #[test]
    fn enter_keeps_what_the_document_says_and_ansi_fits_inches() {
        let mut doc = Document::default();
        doc.set_unit(curva_document::Unit::Feet, false);
        run(&mut doc, &["SetDocumentUserText", "Project", "Pier"]);
        run(&mut doc, &["Layout", "", "0"]);
        let mut command = super::TitleBlock::default();
        command.step(&mut doc, None);
        let live = Live {
            cursor: None,
            number: None,
        };
        assert!(!command.preview(&doc, &live).is_empty(), "the border shows");
        run(&mut doc, &["TitleBlock", "", "", ""]);
        assert_eq!(doc.state().document_text["Project"], "Pier");
        let sheet = &doc.layouts()[0];
        // The sheet's own name, Page 1 as Rhino names a new layout (#687).
        assert_eq!(sheet.user_text["Title"], "Page 1");
        assert!(
            texts(&doc).iter().any(|t| t.contains("M/d/yyyy")),
            "American dates"
        );
        // A half-inch border on the 17 x 11 inch sheet.
        let lowest = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.bounds())
            .map(|(low, _)| low[0])
            .fold(f64::MAX, f64::min);
        assert!((lowest - 0.5).abs() < 1e-9, "{lowest}");
    }

    #[test]
    fn the_standard_can_be_switched() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["Layout", "", "0", "TitleBlock", "Standard", "", "", ""],
        );
        assert!(texts(&doc).iter().any(|t| t.contains("M/d/yyyy")));
    }

    #[test]
    fn a_title_block_needs_a_sheet() {
        let mut doc = Document::default();
        run(&mut doc, &["TitleBlock", "A", "B", "C"]);
        assert!(doc.state().objects.is_empty());
    }
}
