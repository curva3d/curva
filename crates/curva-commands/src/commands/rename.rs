//! `Rename`: gives objects a name.

use super::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Rename {
    ids: Option<Vec<u64>>,
}

impl Command for Rename {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.rename.objects");
            }
            self.ids = Some(chosen);
            return Step::Ask(Request::new("prompt.rename.name", Kind::Path));
        };
        let Some(Answer::Path(name)) = answer else {
            return unexpected();
        };
        doc.rename(&ids, &name);
        Step::Done(Some(Message::with(
            "done.rename",
            vec![ids.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_name_can_have_spaces_and_be_undone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = run(&mut doc, &["rename", "front wall"]);
        assert_eq!(doc.state().objects[0].name, "front wall");
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects[0].name, "");
    }
}
