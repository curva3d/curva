//! `CopyLayout`: copies the layout being worked on, with everything on it, and opens the copy
//! (spec `layouts.md` §1).

use crate::commands::unexpected;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct CopyLayout;

impl Command for CopyLayout {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if answer.is_some() {
            return unexpected();
        }
        let Some(layout) = doc.space().and_then(|id| doc.layout(id)).cloned() else {
            return Step::Failed(Message::new("error.copylayout_no_layout"));
        };
        // A named sheet's copy takes the next number after its name; an unnamed one stays
        // unnamed, and the interface numbers it.
        let name = if layout.name.is_empty() {
            String::new()
        } else {
            let mut n = 2;
            while doc
                .layouts()
                .iter()
                .any(|l| l.name == format!("{} {n}", layout.name))
            {
                n += 1;
            }
            format!("{} {n}", layout.name)
        };
        match doc.copy_layout(layout.id, name) {
            Some(copy) => {
                doc.set_space(Some(copy));
                Step::Done(Some(Message::new("done.copylayout")))
            }
            None => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn the_sheet_being_worked_on_is_copied_and_opened() {
        let mut doc = Document::default();
        run(&mut doc, &["Layout", ""]);
        let first = doc.layouts()[0].id;
        doc.rename_layout(first, "Plans".to_owned());
        run(&mut doc, &["CopyLayout"]);
        assert_eq!(doc.layouts().len(), 2);
        assert_eq!(doc.layouts()[1].name, "Plans 2");
        assert_eq!(doc.space(), Some(doc.layouts()[1].id));
    }

    #[test]
    fn copying_needs_a_layout_open() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["CopyLayout"]);
        assert!(doc.layouts().is_empty());
        assert!(!processor.log().is_empty());
    }
}
