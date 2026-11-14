//! `Orient3Pt`: moves and turns objects so three reference points line up with three targets.

use crate::commands::{ask_objects, ask_point, ghosts, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity, sub};

const KEYS: [&str; 6] = [
    "prompt.orient3pt.ref1",
    "prompt.orient3pt.ref2",
    "prompt.orient3pt.ref3",
    "prompt.orient3pt.target1",
    "prompt.orient3pt.target2",
    "prompt.orient3pt.target3",
];

/// The running command.
#[derive(Default)]
pub struct Orient3Pt {
    ids: Option<Vec<u64>>,
    points: Vec<Point>,
}

/// The turn about the first reference point, then the move, for the last target point `last`.
fn change(points: &[Point], last: Point) -> Option<(Similarity, Point)> {
    let from = [points[0], points[1], points[2]];
    let to = [points[3], points[4], last];
    let turn = Similarity::between_frames(points[0], from, to)?;
    Some((turn, sub(points[3], points[0])))
}

impl Command for Orient3Pt {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
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
        if self.points.len() == 5 {
            let Some((turn, by)) = change(&self.points, at) else {
                return Step::Failed(Message::new("error.three_on_a_line"));
            };
            doc.transform(&ids, &turn);
            doc.translate(&ids, by);
            return Step::Done(Some(Message::with(
                "done.orient",
                vec![ids.len().to_string()],
            )));
        }
        self.points.push(at);
        let count = self.points.len();
        let from = if count < 3 {
            vec![self.points[0]]
        } else if count == 3 {
            Vec::new()
        } else {
            vec![self.points[3]]
        };
        ask_point(KEYS[count], from)
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(cursor)) = (&self.ids, live.cursor) else {
            return Vec::new();
        };
        let points = self.points.as_slice();
        match points.len() {
            0 => Vec::new(),
            1 | 2 => guide(points[0], cursor),
            3 => guide(points[0], points[1]),
            4 => {
                let by = sub(cursor, points[0]);
                let mut shown = guide(points[0], points[1]);
                shown.extend(ghosts(doc, ids, |curve| curve.translated(by)));
                shown
            }
            _ => {
                let mut shown = guide(points[3], points[4]);
                shown.extend(guide(points[3], cursor));
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
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    #[test]
    fn a_flat_square_stands_up_on_the_target_plane() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([1.0, 1.0, 0.0], 1.0).unwrap());
        doc.select(&[id]);
        run(
            &mut doc,
            &["orient3pt", "0,0", "2,0", "0,2", "5,0,0", "5,2,0", "5,0,2"],
        );
        let Curve::Circle { centre, .. } = doc.state().objects[0].curve().unwrap() else {
            panic!("a circle stays a circle");
        };
        assert!(gap(*centre, [5.0, 1.0, 1.0]) < 1e-9, "{centre:?}");
        for point in doc.state().objects[0].curve().unwrap().sample().unwrap() {
            assert!((point[0] - 5.0).abs() < 1e-9, "{point:?}");
        }
    }

    #[test]
    fn three_points_on_a_line_change_nothing() {
        let mut doc = Document::default();
        let line = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        let id = doc.add(line.clone());
        doc.select(&[id]);
        run(
            &mut doc,
            &["orient3pt", "0,0", "1,0", "2,0", "5,5", "6,5", "5,6"],
        );
        assert_eq!(*doc.state().objects[0].curve().unwrap(), line);
    }
}
