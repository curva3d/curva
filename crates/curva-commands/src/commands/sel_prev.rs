//! `SelPrev`: brings back the selection from before it was last replaced.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelPrev;

impl Command for SelPrev {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let open = doc.selectable();
        let found: Vec<u64> = doc
            .previous_selection()
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
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_cleared_selection_comes_back() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [0.0, 0.0, 1.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["sn", "selprev"]);
        assert_eq!(doc.selection(), [a, b]);
    }

    #[test]
    fn deleted_objects_are_left_out() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        doc.select(&[]);
        doc.remove(&[b]);
        run(&mut doc, &["selprev"]);
        assert_eq!(doc.selection(), [a]);
    }
}
