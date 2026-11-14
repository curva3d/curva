//! `Flip`: reverses the direction of curves.

use crate::commands::{ask_objects, curves_of};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Flip;

impl Command for Flip {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.flip.objects");
        }
        for (id, curve) in curves_of(doc, &chosen) {
            doc.replace(id, curve.reversed());
        }
        Step::Done(Some(Message::with(
            "done.flip",
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
    fn a_curve_runs_the_other_way_and_is_the_same_object() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["flip"]);
        assert_eq!(doc.state().objects[0].id, id);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn it_can_be_undone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["flip", "undo"]);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [0.0; 3]
        );
    }
}
