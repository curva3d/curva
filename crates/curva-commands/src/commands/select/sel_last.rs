//! `SelLast`: selects what the last command made or changed.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelLast;

impl Command for SelLast {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let open = doc.selectable();
        let found: Vec<u64> = doc
            .last_changed()
            .into_iter()
            .filter(|id| open.contains(id))
            .collect();
        doc.select(&found);
        Step::Done(Some(Message::with(
            "done.select",
            vec![found.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_selects_the_copies_just_made() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["copy", "0,0", "5,0", "", "sn", "sellast"]);
        let selected = doc.selection();
        assert_eq!(selected.len(), 1);
        assert_ne!(selected[0], id);
    }

    #[test]
    fn a_moved_object_counts_as_changed() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["move", "0,0", "5,0", "sn", "sellast"]);
        assert_eq!(doc.selection(), [id]);
    }
}
