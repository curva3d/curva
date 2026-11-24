//! `Styles`: opens the Style panel, where the active view's style is chosen and edited (spec
//! `docs/specs/views-and-styles.md` §3).

use crate::{Answer, Command, Step, ViewRequest};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Styles;

impl Command for Styles {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::ShowStyles)
    }
}
