//! `History`: turns on or off whether new results follow the curves they came from.

use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct History;

impl Command for History {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let on = !doc.recording_history();
        doc.set_recording_history(on);
        Step::Done(Some(Message::new(if on {
            "done.history_on"
        } else {
            "done.history_off"
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn start_of(doc: &Document, index: usize) -> [f64; 3] {
        doc.state().objects[index]
            .curve()
            .unwrap()
            .sample()
            .unwrap()[0]
    }

    fn select_all(doc: &mut Document) {
        let all = doc.selectable();
        doc.select(&all);
    }

    #[test]
    fn it_turns_recording_on_and_off() {
        let mut doc = Document::default();
        run(&mut doc, &["history"]);
        assert!(doc.recording_history());
        run(&mut doc, &["history"]);
        assert!(!doc.recording_history());
    }

    #[test]
    fn a_mirror_made_with_history_follows_when_the_source_is_moved() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        for typed in ["history", "mirror", "0,0", "0,10"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[id]);
        for typed in ["move", "0,0", "5,0"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(start_of(&doc, 1), [-6.0, 2.0, 0.0]);
    }

    #[test]
    fn a_mirror_made_without_history_stays_put() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        for typed in ["mirror", "0,0", "0,10"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[id]);
        for typed in ["move", "0,0", "5,0"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(start_of(&doc, 1), [-1.0, 2.0, 0.0]);
    }

    #[test]
    fn an_offset_keeps_its_side_and_distance_when_the_source_is_moved() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        for typed in ["history", "offset", "2"] {
            processor.submit(&mut doc, typed);
        }
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        doc.select(&[id]);
        for typed in ["move", "0,0", "0,7"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(start_of(&doc, 1), [0.0, 9.0, 0.0]);
    }

    #[test]
    fn undo_takes_the_child_back_with_the_source() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        for typed in ["history", "mirror", "0,0", "0,10"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[id]);
        for typed in ["move", "0,0", "5,0", "undo"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(start_of(&doc, 1), [-1.0, 2.0, 0.0]);
    }

    #[test]
    fn select_all_includes_children() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        for typed in ["history", "mirror", "0,0", "0,10"] {
            processor.submit(&mut doc, typed);
        }
        select_all(&mut doc);
        assert_eq!(doc.selection().len(), 2);
    }
}
