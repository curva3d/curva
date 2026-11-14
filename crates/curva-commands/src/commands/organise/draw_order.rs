//! The `DrawOrder` command.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `BringToFront`, `SendToBack`, `BringForward` and `SendBackward`: the order curves draw in
/// where they overlap.
#[derive(Default)]
pub struct DrawOrder<const FRONT: bool, const ALL_THE_WAY: bool>;

impl<const FRONT: bool, const ALL_THE_WAY: bool> Command for DrawOrder<FRONT, ALL_THE_WAY> {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        if ids.is_empty() {
            return ask_objects("prompt.draworder.objects");
        }
        doc.reorder(&ids, FRONT, ALL_THE_WAY);
        Step::Done(Some(Message::with(
            "done.draworder",
            vec![ids.len().to_string()],
        )))
    }
}
