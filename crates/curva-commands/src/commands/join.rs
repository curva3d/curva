//! `Join`: makes curves that meet end to end into one curve.

use super::{ask_objects, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, join};

/// The running command.
#[derive(Default)]
pub struct Join;

impl Command for Join {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.len() < 2 {
            return ask_objects("prompt.join.objects");
        }
        let (ids, curves): (Vec<u64>, Vec<Curve>) = doc
            .state()
            .objects
            .iter()
            .filter(|object| chosen.contains(&object.id))
            .map(|object| (object.id, object.curve.clone()))
            .unzip();
        let made = match join(&curves) {
            Ok(made) => made,
            Err(error) => return geometry_failed(&error),
        };
        if made.is_empty() {
            return Step::Failed(Message::new("error.nothing_to_join"));
        }
        let mut joined = 0;
        for (replaced, curve) in made {
            let gone: Vec<u64> = replaced.iter().map(|index| ids[*index]).collect();
            joined += gone.len();
            let layer = gone.first().and_then(|id| doc.layer_of(*id));
            doc.remove(&gone);
            doc.add_on(curve, layer);
        }
        Step::Done(Some(Message::with("done.join", vec![joined.to_string()])))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn two_selected_lines(gap: f64) -> Document {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([1.0 + gap, 0.0, 0.0], [2.0, 1.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        doc
    }

    #[test]
    fn two_lines_become_one_curve() {
        let mut doc = two_selected_lines(0.0);
        run(&mut doc, &["join"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = two_selected_lines(0.0);
        run(&mut doc, &["join", "undo"]);
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn curves_that_do_not_meet_stay_as_they_are() {
        let mut doc = two_selected_lines(0.5);
        let processor = run(&mut doc, &["join"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(!processor.is_running());
    }

    #[test]
    fn with_too_little_selected_it_asks_for_objects() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["join"]);
        assert!(processor.is_running());
    }
}
