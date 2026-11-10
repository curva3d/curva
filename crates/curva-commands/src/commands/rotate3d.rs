//! `Rotate3D`: turns objects about a line between two points.

use super::{ask_objects, ask_point, ghosts, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity, sub, unit};

/// The running command.
#[derive(Default)]
pub struct Rotate3D {
    ids: Option<Vec<u64>>,
    start: Option<Point>,
    axis: Option<Point>,
}

impl Command for Rotate3D {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.rotate.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.rotate3d.start", Vec::new());
        };
        match (self.start, self.axis, answer) {
            (None, None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.rotate3d.end", vec![start])
            }
            (Some(start), None, Some(Answer::Point(end))) => {
                let Some(axis) = unit(sub(end, start)) else {
                    return Step::Failed(Message::new("error.reference_point"));
                };
                self.axis = Some(axis);
                Step::Ask(Request::new("prompt.rotate3d.angle", Kind::Number))
            }
            (Some(start), Some(axis), Some(Answer::Number(degrees))) => {
                doc.transform(
                    &ids,
                    &Similarity::rotation(start, axis, degrees.to_radians()),
                );
                Step::Done(Some(Message::with(
                    "done.rotate",
                    vec![ids.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(start)) = (&self.ids, self.start) else {
            return Vec::new();
        };
        match (self.axis, live.number, live.cursor) {
            (Some(axis), Some(degrees), _) => {
                let turn = Similarity::rotation(start, axis, degrees.to_radians());
                ghosts(doc, ids, |curve| curve.mapped(&turn))
            }
            (None, _, Some(cursor)) => guide(start, cursor),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0, 1.0, 0.0], [0.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn objects_turn_about_the_line_right_handed() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate3d", "0,0,0", "1,0,0", "90"]);
        let start = doc.state().objects[0].curve().unwrap().sample().unwrap()[0];
        assert!(
            start[1].abs() < 1e-9 && (start[2] - 1.0).abs() < 1e-9,
            "{start:?}"
        );
    }

    #[test]
    fn a_line_with_no_length_changes_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["rotate3d", "1,1,1", "1,1,1"]);
        assert!(!processor.is_running());
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn the_preview_follows_a_typed_angle() {
        let mut doc = selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["rotate3d", "0,0,0", "1,0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(processor.preview(&doc, None, "90").len(), 1);
    }
}
