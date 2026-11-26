//! `Revision`: adds a row to the open sheet's revision table (spec `layouts.md` §4),
//! Serpentine3D's command, as Rhino has none. The mark becomes the sheet's `Revision` layout user
//! text, which the title block shows, and the row shows in the sheet's revision table.

use super::super::organise::set_document_user_text::typed_text;
use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::fields::{FieldContext, resolve};
use curva_document::sheet::Revision as Row;

/// The running command: the mark (Enter takes the next one), what changed, and who made it
/// (Enter takes the document's `DrawnBy`). The date is today's.
#[derive(Default)]
pub struct Revision {
    mark: Option<String>,
    description: Option<String>,
}

/// The mark after `last`: the next letter after a letter (`A` → `B`, `Z` → `AA`), the next
/// number keeping its width after a mark ending in digits (`P01` → `P02`), else `A`.
#[must_use]
pub fn next_mark(last: Option<&str>) -> String {
    let Some(last) = last.map(str::trim).filter(|l| !l.is_empty()) else {
        return "A".to_owned();
    };
    let digits = last.len() - last.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        let (stem, number) = last.split_at(last.len() - digits);
        let next = number.parse::<u64>().map_or(1, |n| n + 1);
        return format!("{stem}{next:0digits$}");
    }
    let letters: Vec<char> = last.chars().collect();
    if letters.iter().all(char::is_ascii_uppercase) {
        // Counting in letters, as spreadsheets name columns.
        let mut value = letters
            .iter()
            .fold(0_u64, |v, c| v * 26 + u64::from(*c as u8 - b'A') + 1);
        value += 1;
        let mut mark = Vec::new();
        while value > 0 {
            value -= 1;
            #[allow(clippy::cast_possible_truncation)]
            mark.push(char::from(b'A' + (value % 26) as u8));
            value /= 26;
        }
        return mark.iter().rev().collect();
    }
    "A".to_owned()
}

impl Revision {
    fn suggested(doc: &Document) -> String {
        let last = doc
            .space()
            .and_then(|id| doc.layout(id))
            .and_then(|l| l.revisions.last())
            .map(|r| r.mark.clone());
        next_mark(last.as_deref())
    }
}

impl Command for Revision {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(layout) = doc.space() else {
            return Step::Failed(Message::new("error.sheet_only"));
        };
        match (&self.mark, &self.description, answer) {
            (_, _, None) => Step::Ask(Request {
                prompt: Message::with("prompt.revision.mark", vec![Self::suggested(doc)]),
                allow_empty: true,
                ..Request::new("prompt.revision.mark", Kind::Path)
            }),
            (None, _, Some(answer @ (Answer::Path(_) | Answer::Empty))) => {
                let typed = typed_text(Some(answer));
                self.mark = Some(if typed.trim().is_empty() {
                    Self::suggested(doc)
                } else {
                    typed.trim().to_owned()
                });
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.revision.description", Kind::Path)
                })
            }
            (Some(_), None, Some(answer @ (Answer::Path(_) | Answer::Empty))) => {
                self.description = Some(typed_text(Some(answer)).trim().to_owned());
                let by = doc
                    .state()
                    .document_text
                    .get("DrawnBy")
                    .cloned()
                    .unwrap_or_default();
                Step::Ask(Request {
                    prompt: Message::with("prompt.revision.by", vec![by]),
                    allow_empty: true,
                    ..Request::new("prompt.revision.by", Kind::Path)
                })
            }
            (Some(mark), Some(description), Some(answer @ (Answer::Path(_) | Answer::Empty))) => {
                let typed = typed_text(Some(answer));
                let by = if typed.trim().is_empty() {
                    doc.state()
                        .document_text
                        .get("DrawnBy")
                        .cloned()
                        .unwrap_or_default()
                } else {
                    typed.trim().to_owned()
                };
                let date = resolve(
                    "%<Date(\"yyyy-MM-dd\")>%",
                    doc.state(),
                    None,
                    &FieldContext::now(None),
                );
                let row = Row {
                    mark: mark.clone(),
                    date,
                    description: description.clone(),
                    by,
                };
                doc.add_revision(layout, row);
                Step::Done(Some(Message::with("done.revision", vec![mark.clone()])))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::next_mark;
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn marks_count_on_in_letters_or_numbers() {
        assert_eq!(next_mark(None), "A");
        assert_eq!(next_mark(Some("A")), "B");
        assert_eq!(next_mark(Some("Z")), "AA");
        assert_eq!(next_mark(Some("P01")), "P02");
        assert_eq!(next_mark(Some("C09")), "C10");
        assert_eq!(next_mark(Some("3")), "4");
        assert_eq!(next_mark(Some("draft")), "A");
    }

    #[test]
    fn a_revision_adds_a_row_and_becomes_the_sheets_revision() {
        let mut doc = Document::default();
        run(&mut doc, &["SetDocumentUserText", "DrawnBy", "LV"]);
        run(&mut doc, &["Layout", "", "0"]);
        run(&mut doc, &["Revision", "", "Issued for planning", ""]);
        run(&mut doc, &["Revision", "", "Doors moved", "AB"]);
        let sheet = &doc.layouts()[0];
        assert_eq!(sheet.revisions.len(), 2);
        assert_eq!(sheet.revisions[0].mark, "A");
        assert_eq!(sheet.revisions[0].by, "LV", "Enter takes DrawnBy");
        assert_eq!(sheet.revisions[0].date.len(), 10, "today, as yyyy-MM-dd");
        assert_eq!(sheet.revisions[1].mark, "B");
        assert_eq!(sheet.revisions[1].description, "Doors moved");
        assert_eq!(sheet.revisions[1].by, "AB");
        assert_eq!(sheet.user_text["Revision"], "B");
        run(&mut doc, &["Revision", "P01", "", ""]);
        assert_eq!(doc.layouts()[0].revisions[2].mark, "P01");
    }

    #[test]
    fn a_revision_needs_a_sheet() {
        let mut doc = Document::default();
        run(&mut doc, &["Revision", "A", "Note", "LV"]);
        assert!(doc.layouts().is_empty());
    }
}
