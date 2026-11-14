//! The `Purge` command.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// `Purge`: removes layers nothing is on and block definitions nothing uses.
#[derive(Default)]
pub struct Purge;

impl Command for Purge {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let (layers, blocks) = doc.purge();
        Step::Done(Some(Message::with(
            "done.purge",
            vec![layers.to_string(), blocks.to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn line(doc: &mut Document, y: f64) -> u64 {
        doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
    }

    #[test]
    fn purge_audit_lockswap_and_draw_order() {
        let mut doc = Document::default();
        doc.add_layer("Empty".to_owned());
        let a = line(&mut doc, 0.0);
        let b = line(&mut doc, 1.0);
        run(&mut doc, &["purge"]);
        assert_eq!(doc.layers().len(), 1);
        let processor = run(&mut doc, &["audit"]);
        assert!(processor.log().last().unwrap().contains("No problems"));
        doc.lock(&[a]);
        run(&mut doc, &["lockswap"]);
        assert!(
            !doc.state()
                .objects
                .iter()
                .find(|o| o.id == a)
                .unwrap()
                .locked
        );
        assert!(
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == b)
                .unwrap()
                .locked
        );
        run(&mut doc, &["unlockselected"]);
        assert!(doc.state().objects.iter().all(|o| !o.locked));
        doc.select(&[a]);
        run(&mut doc, &["bringtofront"]);
        assert_eq!(doc.state().objects.last().unwrap().id, a);
        doc.select(&[a]);
        run(&mut doc, &["sendtoback"]);
        assert_eq!(doc.state().objects[0].id, a);
    }
}
