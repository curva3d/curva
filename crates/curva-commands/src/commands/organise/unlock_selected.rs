//! The `UnlockSelected` command.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;

/// `UnlockSelected`, as in Rhino 8 (#715): asks for objects to unlock, and while it asks only
/// locked objects can be picked; just those picked are unlocked, the rest stay locked.
#[derive(Default)]
pub struct UnlockSelected;

impl Command for UnlockSelected {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => {
                if !doc.state().objects.iter().any(|o| o.locked) {
                    return Step::Done(Some(Message::with("done.unlock", vec!["0".to_owned()])));
                }
                Step::Ask(Request {
                    locked_only: true,
                    ..Request::new("prompt.unlockselected.objects", Kind::Objects)
                })
            }
            Some(Answer::Objects(list)) => {
                let locked: Vec<u64> = list
                    .into_iter()
                    .filter(|id| doc.state().objects.iter().any(|o| o.id == *id && o.locked))
                    .collect();
                let count = doc.unlock(&locked);
                Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn only_the_locked_objects_picked_are_unlocked_as_in_rhino() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 1.0, 0.0], [1.0, 1.0, 0.0]).unwrap());
        doc.lock(&[a, b]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "unlockselected");
        // Rhino 8: it asks, and only locked objects can be picked while it does.
        assert!(processor.current_request().is_some_and(|r| r.locked_only));
        doc.select(&[a]);
        processor.submit(&mut doc, "");
        let locked = |id: u64| {
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .unwrap()
                .locked
        };
        assert!(!locked(a) && locked(b));
    }
}
