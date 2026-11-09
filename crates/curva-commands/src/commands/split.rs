//! `Split`: cuts curves where they cross other curves, keeping every piece.

use super::{ask_objects, curves_of, cut_by, geometry_failed, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Split {
    targets: Option<Vec<u64>>,
}

impl Command for Split {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(targets) = self.targets.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.split.objects");
            }
            self.targets = Some(chosen);
            // What was chosen to be cut must not also count as chosen to cut with.
            doc.select(&[]);
            return ask_objects("prompt.split.cutters");
        };
        let Some(Answer::Objects(cutter_ids)) = answer else {
            return unexpected();
        };
        let mut made = 0;
        for (id, curve) in curves_of(doc, &targets) {
            let others: Vec<u64> = cutter_ids.iter().copied().filter(|c| *c != id).collect();
            let cutters: Vec<_> = curves_of(doc, &others)
                .into_iter()
                .map(|(_, c)| c)
                .collect();
            let pieces = match cut_by(&curve, &cutters) {
                Ok(pieces) => pieces,
                Err(error) => return geometry_failed(&error),
            };
            if pieces.len() >= 2 {
                let layer = doc.layer_of(id);
                doc.remove(&[id]);
                made += pieces.len();
                for piece in pieces {
                    doc.add_on(piece, layer);
                }
            }
        }
        if made == 0 {
            return Step::Failed(Message::new("error.nothing_to_cut"));
        }
        Step::Done(Some(Message::with("done.split", vec![made.to_string()])))
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn crossing_lines() -> (Document, u64, u64) {
        let mut doc = Document::default();
        let target = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let cutter = doc.add(Curve::line([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        (doc, target, cutter)
    }

    fn split(doc: &mut Document, target: u64, cutter: u64) -> Processor {
        let mut processor = Processor::default();
        doc.select(&[target]);
        processor.submit(doc, "split");
        doc.select(&[cutter]);
        processor.submit(doc, "");
        processor
    }

    #[test]
    fn a_curve_is_cut_where_it_crosses_and_the_cutter_stays() {
        let (mut doc, target, cutter) = crossing_lines();
        let processor = split(&mut doc, target, cutter);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 3);
        assert!(doc.state().objects.iter().any(|o| o.id == cutter));
        assert!(doc.state().objects.iter().all(|o| o.id != target));
    }

    #[test]
    fn it_is_one_undo_step() {
        let (mut doc, target, cutter) = crossing_lines();
        let mut processor = split(&mut doc, target, cutter);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
        assert!(doc.state().objects.iter().any(|o| o.id == target));
    }

    #[test]
    fn a_cutter_that_misses_changes_nothing() {
        let mut doc = Document::default();
        let target = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let cutter = doc.add(Curve::line([3.0, 2.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let processor = split(&mut doc, target, cutter);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(!processor.is_running());
    }
}
