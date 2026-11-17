//! `Isolate`: hides everything except the chosen objects, until `Show`.

use crate::commands::{ask_objects, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Isolate;

impl Command for Isolate {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.isolate.objects");
        };
        let others: Vec<u64> = doc
            .selectable()
            .into_iter()
            .filter(|id| !chosen.contains(id))
            .collect();
        doc.hide(&others);
        Step::Done(Some(Message::with(
            "done.hide",
            vec![others.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn everything_else_hides_and_show_brings_it_back() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.select(&[a]);
        run(&mut doc, &["isolate"]);
        assert_eq!(doc.selectable(), [a]);
        run(&mut doc, &["unisolate"]);
        assert_eq!(doc.selectable(), [a, b]);
    }
}
