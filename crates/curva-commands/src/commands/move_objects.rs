//! `Move`: moves objects from one point to another.

use super::{ask_objects, ask_point, ghosts, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, sub};

/// The running command.
#[derive(Default)]
pub struct MoveObjects {
    ids: Option<Vec<u64>>,
    from: Option<Point>,
}

impl Command for MoveObjects {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.move.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.move.from", Vec::new());
        };
        match (self.from, answer) {
            (None, Some(Answer::Point(from))) => {
                self.from = Some(from);
                ask_point("prompt.move.to", vec![from])
            }
            (Some(from), Some(Answer::Point(to))) => {
                doc.translate(&ids, [to[0] - from[0], to[1] - from[1], to[2] - from[2]]);
                Step::Done(Some(Message::with(
                    "done.move",
                    vec![ids.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(from), Some(cursor)) = (&self.ids, self.from, live.cursor) else {
            return Vec::new();
        };
        let by = sub(cursor, from);
        let mut shown = guide(from, cursor);
        shown.extend(ghosts(doc, ids, |curve| curve.translated(by)));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn one_selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn moves_by_the_difference_between_the_points() {
        let mut doc = one_selected_line();
        run(&mut doc, &["move", "1,1,1", "4,5,6"]);
        let expected = Curve::line([3.0, 4.0, 5.0], [4.0, 4.0, 5.0]).unwrap();
        assert_eq!(doc.state().objects[0].curve, expected);
    }

    #[test]
    fn asks_for_objects_when_nothing_is_selected() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["m"]);
        assert!(processor.prompt().unwrap().contains("Select"));
    }

    #[test]
    fn undo_puts_it_back() {
        let mut doc = one_selected_line();
        let before = doc.state().clone();
        run(&mut doc, &["move", "0,0", "9,9", "undo"]);
        assert_eq!(doc.state(), &before);
    }
}
