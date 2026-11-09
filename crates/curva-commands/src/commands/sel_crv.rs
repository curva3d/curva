//! `SelCrv`: selects every curve that can be selected.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct SelCrv;

impl Command for SelCrv {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let open = doc.selectable();
        let found: Vec<u64> = doc
            .state()
            .objects
            .iter()
            .filter(|object| {
                open.contains(&object.id) && !matches!(object.curve, Curve::Point { .. })
            })
            .map(|object| object.id)
            .collect();
        doc.select(&found);
        Step::Done(Some(Message::with(
            "done.select",
            vec![found.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_selects_only_that_kind() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let point = doc.add(Curve::Point {
            at: [2.0, 0.0, 0.0],
        });
        run(&mut doc, &["selcrv"]);
        assert_eq!(doc.selection(), [line]);
        let _ = (line, point);
    }
}
