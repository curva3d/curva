//! `Copy`: places copies of objects at each point given, until Enter.

use super::{ask_objects, ask_point, ghosts, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, sub};

/// The running command.
#[derive(Default)]
pub struct CopyObjects {
    ids: Option<Vec<u64>>,
    from: Option<Point>,
    placed: usize,
}

fn ask_target(from: Point) -> Step {
    Step::Ask(Request {
        allow_empty: true,
        preview: vec![from],
        ..Request::new("prompt.copy.to", Kind::Point)
    })
}

impl Command for CopyObjects {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.copy.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.copy.from", Vec::new());
        };
        match (self.from, answer) {
            (None, Some(Answer::Point(from))) => {
                self.from = Some(from);
                ask_target(from)
            }
            (Some(from), Some(Answer::Point(to))) => {
                self.placed += doc
                    .copy(&ids, [to[0] - from[0], to[1] - from[1], to[2] - from[2]])
                    .len();
                ask_target(from)
            }
            (Some(_), Some(Answer::Empty)) => Step::Done(Some(Message::with(
                "done.copy",
                vec![self.placed.to_string()],
            ))),
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
    fn places_a_copy_at_each_point_and_keeps_the_original() {
        let mut doc = one_selected_line();
        run(&mut doc, &["copy", "0,0", "5,0", "0,5", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 3);
        assert_eq!(
            objects[0].curve,
            Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap()
        );
        assert_eq!(
            objects[1].curve,
            Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap()
        );
        assert_eq!(
            objects[2].curve,
            Curve::line([0.0, 5.0, 0.0], [1.0, 5.0, 0.0]).unwrap()
        );
    }

    #[test]
    fn undo_removes_all_the_copies_at_once() {
        let mut doc = one_selected_line();
        let before = doc.state().clone();
        run(&mut doc, &["cp", "0,0", "5,0", "9,0", "", "undo"]);
        assert_eq!(doc.state(), &before);
    }
}
