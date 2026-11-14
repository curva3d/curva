//! `SelDup`: selects objects that exactly repeat an earlier object.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct SelDup;

impl Command for SelDup {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let open = doc.selectable();
        let objects = &doc.state().objects;
        let found: Vec<u64> = objects
            .iter()
            .enumerate()
            .filter(|(index, object)| {
                open.contains(&object.id)
                    && objects[..*index].iter().any(|o| o.shape == object.shape)
            })
            .map(|(_, object)| object.id)
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
    fn only_the_later_copy_is_selected() {
        let mut doc = Document::default();
        let line = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        doc.add(line.clone());
        let copy = doc.add(line);
        doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        run(&mut doc, &["seldup"]);
        assert_eq!(doc.selection(), [copy]);
    }
}
