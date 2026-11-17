//! `Delete`: removes objects.

use crate::commands::{ask_objects, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Delete;

impl Command for Delete {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.delete.objects");
        };
        doc.remove(&chosen);
        Step::Done(Some(Message::with(
            "done.delete",
            vec![chosen.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn removes_the_selection_and_undo_restores_it() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let before = doc.state().clone();
        let mut processor = run(&mut doc, &["delete"]);
        assert!(doc.state().objects.is_empty());
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state(), &before);
    }
}
