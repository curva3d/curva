//! `Orient`: moves and turns objects so two reference points land on two target points.

use super::{ask_objects, ask_point, ghosts, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity, cross, dot, sub, unit};

/// The running command.
#[derive(Default)]
pub struct Orient {
    ids: Option<Vec<u64>>,
    points: Vec<Point>,
}

/// The turn about `pivot` that takes the direction `from` onto the direction `to`.
fn turn_between(pivot: Point, from: Point, to: Point) -> Option<Similarity> {
    let (from, to) = (unit(from)?, unit(to)?);
    let angle = dot(from, to).clamp(-1.0, 1.0).acos();
    let axis = unit(cross(from, to)).or_else(|| {
        // Opposite or equal directions leave the axis free, so any at a right angle will do.
        let helper = if from[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        unit(cross(from, helper))
    })?;
    Some(Similarity::rotation(pivot, axis, angle))
}

fn change(points: &[Point], target_two: Point) -> Option<(Similarity, Point)> {
    let (r1, r2, t1) = (points[0], points[1], points[2]);
    let turn = turn_between(r1, sub(r2, r1), sub(target_two, t1))?;
    Some((turn, sub(t1, r1)))
}

impl Command for Orient {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        const KEYS: [&str; 4] = [
            "prompt.orient.ref1",
            "prompt.orient.ref2",
            "prompt.orient.target1",
            "prompt.orient.target2",
        ];
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.orient.objects");
            }
            self.ids = Some(chosen);
            return ask_point(KEYS[0], Vec::new());
        };
        let Some(Answer::Point(at)) = answer else {
            return unexpected();
        };
        if self.points.len() == 3 {
            let Some((turn, by)) = change(&self.points, at) else {
                return Step::Failed(Message::new("error.reference_point"));
            };
            doc.transform(&ids, &turn);
            doc.translate(&ids, by);
            return Step::Done(Some(Message::with(
                "done.orient",
                vec![ids.len().to_string()],
            )));
        }
        self.points.push(at);
        let from = match self.points.len() {
            2 => vec![self.points[0]],
            1 => vec![self.points[0]],
            _ => vec![self.points[2]],
        };
        ask_point(KEYS[self.points.len()], from)
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(cursor)) = (&self.ids, live.cursor) else {
            return Vec::new();
        };
        match self.points.as_slice() {
            [] => Vec::new(),
            [only] => guide(*only, cursor),
            [a, b] => {
                let mut shown = guide(*a, *b);
                let by = sub(cursor, *a);
                shown.extend(ghosts(doc, ids, |curve| curve.translated(by)));
                shown
            }
            points => {
                let mut shown = guide(points[2], cursor);
                if let Some((turn, by)) = change(points, cursor) {
                    shown.extend(ghosts(doc, ids, |curve| curve.mapped(&turn).translated(by)));
                }
                shown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point, gap};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [2.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document) -> Vec<Point> {
        doc.state().objects[0].curve().unwrap().sample().unwrap()
    }

    #[test]
    fn reference_points_land_on_the_targets() {
        let mut doc = selected_line();
        run(&mut doc, &["orient", "0,0", "2,0", "5,5", "5,9"]);
        let found = ends(&doc);
        assert!(gap(found[0], [5.0, 5.0, 0.0]) < 1e-9, "{found:?}");
        assert!(gap(found[1], [5.0, 7.0, 0.0]) < 1e-9, "{found:?}");
    }

    #[test]
    fn it_works_when_the_direction_is_reversed() {
        let mut doc = selected_line();
        run(&mut doc, &["orient", "0,0", "2,0", "0,0", "-1,0"]);
        let found = ends(&doc);
        assert!(gap(found[1], [-2.0, 0.0, 0.0]) < 1e-9, "{found:?}");
    }

    #[test]
    fn reference_points_in_one_place_change_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["orient", "1,1", "1,1", "5,5", "9,9"]);
        assert!(!processor.is_running());
        assert_eq!(ends(&doc)[1], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_line();
        run(&mut doc, &["orient", "0,0", "2,0", "5,5", "5,9", "undo"]);
        assert_eq!(ends(&doc)[1], [2.0, 0.0, 0.0]);
    }
}
