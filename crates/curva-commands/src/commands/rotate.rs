//! `Rotate`: turns objects about a point, on the plane being drawn on.

use super::{ask_objects, ask_point, ask_point_or_number, ghosts, guide, plane_normal, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity, cross, dot, flattened, sub, unit};

/// The running command.
#[derive(Default)]
pub struct Rotate {
    ids: Option<Vec<u64>>,
    centre: Option<Point>,
    first: Option<Point>,
    axis: Point,
}

impl Rotate {
    fn finish(doc: &mut Document, ids: &[u64], centre: Point, axis: Point, angle: f64) -> Step {
        doc.transform(ids, &Similarity::rotation(centre, axis, angle));
        Step::Done(Some(Message::with(
            "done.rotate",
            vec![ids.len().to_string()],
        )))
    }
}

impl Command for Rotate {
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
            return ask_point("prompt.rotate.centre", Vec::new());
        };
        match (self.centre, self.first, answer) {
            (None, None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                self.axis = plane_normal(doc);
                ask_point_or_number("prompt.rotate.angle", vec![centre])
            }
            (Some(centre), None, Some(Answer::Number(degrees))) => {
                Self::finish(doc, &ids, centre, self.axis, degrees.to_radians())
            }
            (Some(centre), None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                ask_point("prompt.rotate.second", vec![centre])
            }
            (Some(centre), Some(first), Some(Answer::Point(second))) => {
                let Some(angle) = self.turn(centre, first, second) else {
                    return Step::Failed(Message::new("error.reference_point"));
                };
                Self::finish(doc, &ids, centre, self.axis, angle)
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(centre)) = (&self.ids, self.centre) else {
            return Vec::new();
        };
        let turned = |angle: f64| {
            let change = Similarity::rotation(centre, self.axis, angle);
            ghosts(doc, ids, |curve| curve.mapped(&change))
        };
        let mut shown = Vec::new();
        match (self.first, live.number, live.cursor) {
            (None, Some(degrees), _) => shown.extend(turned(degrees.to_radians())),
            (Some(first), _, Some(cursor)) => {
                if let Some(angle) = self.turn(centre, first, cursor) {
                    shown.extend(turned(angle));
                }
                shown.extend(guide(centre, first));
            }
            _ => {}
        }
        if let Some(cursor) = live.cursor {
            shown.extend(guide(centre, cursor));
        }
        shown
    }
}

impl Rotate {
    /// The signed turn from one reference point to another, seen along the axis.
    fn turn(&self, centre: Point, first: Point, second: Point) -> Option<f64> {
        let across = |p: Point| unit(flattened(sub(p, centre), self.axis));
        let (from, to) = (across(first)?, across(second)?);
        Some(dot(self.axis, cross(from, to)).atan2(dot(from, to)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn start_of_first(doc: &Document) -> Point {
        doc.state().objects[0].curve().unwrap().sample().unwrap()[0]
    }

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-9)
    }

    #[test]
    fn a_typed_angle_turns_counterclockwise_in_degrees() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "90"]);
        assert!(close(start_of_first(&doc), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn two_reference_points_give_the_angle_between_them() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "5,0", "0,-3"]);
        assert!(close(start_of_first(&doc), [0.0, -1.0, 0.0]));
    }

    #[test]
    fn it_turns_about_the_plane_being_drawn_on() {
        let mut doc = selected_line();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["rotate", "0,0,0", "90"]);
        assert!(close(start_of_first(&doc), [0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_reference_point_on_the_centre_changes_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["rotate", "0,0", "0,0", "1,0"]);
        assert!(close(start_of_first(&doc), [1.0, 0.0, 0.0]));
        assert!(!processor.is_running());
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "90", "undo"]);
        assert!(close(start_of_first(&doc), [1.0, 0.0, 0.0]));
    }
}
