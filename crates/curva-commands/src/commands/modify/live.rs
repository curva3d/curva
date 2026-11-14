//! `EditBoolean` and `BreakBoolean`: working with live booleans (spec `docs/specs/live-booleans.md`).

use crate::commands::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

fn live_results(doc: &Document, chosen: &[u64]) -> Vec<u64> {
    doc.state()
        .objects
        .iter()
        .filter(|o| chosen.contains(&o.id) && o.live.is_some())
        .map(|o| o.id)
        .collect()
}

/// `EditBoolean`: shows a live boolean's inputs so they can be moved and edited, the result
/// following them; run again on the result to hide them.
#[derive(Default)]
pub struct EditBoolean;

impl Command for EditBoolean {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.editboolean.object");
        }
        let results = live_results(doc, &chosen);
        let Some(&result) = results.first() else {
            return Step::Failed(Message::new("error.not_live"));
        };
        let count = doc.inputs_of(result).len();
        let shown = doc.toggle_inputs(result);
        doc.select(&[]);
        Step::Done(Some(Message::with(
            if shown {
                "done.editboolean_shown"
            } else {
                "done.editboolean_hidden"
            },
            vec![count.to_string()],
        )))
    }
}

/// `BreakBoolean`: drops a live boolean's inputs and link, leaving a plain solid.
#[derive(Default)]
pub struct BreakBoolean;

impl Command for BreakBoolean {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.breakboolean.objects");
        }
        let results = live_results(doc, &chosen);
        if results.is_empty() {
            return Step::Failed(Message::new("error.not_live"));
        }
        for result in &results {
            doc.break_live(*result);
        }
        Step::Done(Some(Message::with(
            "done.breakboolean",
            vec![results.len().to_string()],
        )))
    }
}
