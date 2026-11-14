//! The `UnlockSelected` command.

use crate::commands::unexpected;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `UnlockSelected`: unlocks the locked objects picked (they can be picked while this asks).
#[derive(Default)]
pub struct UnlockSelected;

impl Command for UnlockSelected {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Objects(list)) => {
                let count = doc.unlock(&list);
                Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
            }
            None => {
                // Every locked object, as Rhino offers them to pick from.
                let locked: Vec<u64> = doc
                    .state()
                    .objects
                    .iter()
                    .filter(|o| o.locked)
                    .map(|o| o.id)
                    .collect();
                if locked.is_empty() {
                    return Step::Done(Some(Message::with("done.unlock", vec!["0".to_owned()])));
                }
                let count = doc.unlock(&locked);
                Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
            }
            _ => unexpected(),
        }
    }
}
