//! The `BlockManager` command.

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// `BlockManager`: opens the Blocks panel.
#[derive(Default)]
pub struct BlockManager;

impl Command for BlockManager {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowBlocks)
    }
}
