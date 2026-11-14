//! The `LockSwap` command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `LockSwap`: locks what is unlocked and unlocks what is locked.
#[derive(Default)]
pub struct LockSwap;

impl Command for LockSwap {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let count = doc.lock_swap();
        Step::Done(Some(Message::with(
            "done.lockswap",
            vec![count.to_string()],
        )))
    }
}
