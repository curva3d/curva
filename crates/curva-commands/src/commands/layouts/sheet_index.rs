//! `SheetIndex`: a table of every layout's number, name and title (spec `layouts.md` §4),
//! Serpentine3D's command, as Rhino has none. It follows the layouts as they are added,
//! renamed, reordered or given a `Title` (layout user text).

use crate::commands::annotate::text::usual_height;
use crate::commands::unexpected;
use crate::text::Translations;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Text, TextAlign};
use curva_document::sheet::{Table, TableKind};
use curva_kernel::{Curve, Point};

/// The running command: where the table's top left corner goes, on a layout.
#[derive(Default)]
pub struct SheetIndex;

/// The sheet index placed at `at`, its words in the user's language.
fn index_at(doc: &Document, at: Point) -> Table {
    let words = Translations::english();
    let say = |key: &'static str| words.render(&Message::new(key));
    let [x, y] = doc.work_plane();
    Table {
        at: Text {
            content: String::new(),
            origin: at,
            x,
            y,
            height: usual_height(doc),
            font: None,
            align: TextAlign::Left,
            style: doc.current_annotation_style(),
        },
        kind: TableKind::SheetIndex,
        headings: vec![
            say("sheet.heading.number"),
            say("sheet.heading.sheet"),
            say("sheet.heading.title"),
        ],
        title: Some(say("sheet.index_title")),
        unnamed: Some(say("ui.layout.unnamed")),
        width: None,
    }
}

impl Command for SheetIndex {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if doc.space().is_none() {
            return Step::Failed(Message::new("error.sheet_only"));
        }
        match answer {
            None => Step::Ask(Request::new("prompt.sheetindex.place", Kind::Point)),
            Some(Answer::Point(at)) => {
                doc.add_annotation(Annotation::Table(index_at(doc, at)));
                Step::Done(Some(Message::with(
                    "done.sheetindex",
                    vec![doc.layouts().len().to_string()],
                )))
            }
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(at) = live.cursor.filter(|_| doc.space().is_some()) else {
            return Vec::new();
        };
        let table = Annotation::Table(index_at(doc, at));
        curva_document::shape::annotation_drawing(&table, doc.state(), doc.space()).lines
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::sheet::{TableKind, table_rows};
    use curva_document::{Document, Shape};

    #[test]
    fn the_index_lists_every_sheet_and_follows_them() {
        let mut doc = Document::default();
        run(&mut doc, &["Layout", "", "Layout", ""]);
        let mut command = super::SheetIndex;
        command.step(&mut doc, None);
        let live = Live {
            cursor: Some([20.0, 280.0, 0.0]),
            number: None,
        };
        assert!(!command.preview(&doc, &live).is_empty());
        run(&mut doc, &["SheetIndex", "20,280"]);
        let table = doc
            .state()
            .objects
            .iter()
            .find_map(|o| match &o.shape {
                Shape::Annotation(Annotation::Table(t)) => Some(t.clone()),
                _ => None,
            })
            .expect("a table");
        assert_eq!(table.kind, TableKind::SheetIndex);
        assert_eq!(table.headings.len(), 3);
        assert_eq!(table_rows(&table, doc.state()).len(), 2);
        run(&mut doc, &["Layout", ""]);
        assert_eq!(
            table_rows(&table, doc.state()).len(),
            3,
            "a new sheet is listed"
        );
    }

    #[test]
    fn the_index_goes_on_a_sheet() {
        let mut doc = Document::default();
        run(&mut doc, &["SheetIndex", "0,0"]);
        assert!(doc.state().objects.is_empty());
    }
}
