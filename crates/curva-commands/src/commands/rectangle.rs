//! `Rectangle`: a closed polyline from two opposite corners, on the plane being drawn on.

use super::{ask_point, geometry_failed, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, dot, sub};

/// Shortest side treated as real, in millimetres.
const SMALLEST_SIDE: f64 = 1e-9;

/// The running command.
#[derive(Default)]
pub struct Rectangle {
    first: Option<Point>,
    plane: Option<[Point; 2]>,
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [
        p[0] + direction[0] * amount,
        p[1] + direction[1] * amount,
        p[2] + direction[2] * amount,
    ]
}

/// The four corners on a plane from two opposite ones, or `None` if they are in line.
fn corners(first: Point, other: Point, x: Point, y: Point) -> Option<Vec<Point>> {
    let across = sub(other, first);
    let (wide, high) = (dot(across, x), dot(across, y));
    if wide.abs() < SMALLEST_SIDE || high.abs() < SMALLEST_SIDE {
        return None;
    }
    let side = plus(first, x, wide);
    Some(vec![first, side, plus(side, y, high), plus(first, y, high)])
}

impl Command for Rectangle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.first, answer) {
            (None, None) => ask_point("prompt.rectangle.first", Vec::new()),
            (None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                self.plane = Some(doc.work_plane());
                ask_point("prompt.rectangle.other", vec![first])
            }
            (Some(first), Some(Answer::Point(other))) => {
                let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
                let Some(corners) = corners(first, other, x, y) else {
                    return Step::Failed(Message::new("error.flat_rectangle"));
                };
                match Curve::polyline(corners, true) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::new("done.rectangle")))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(first), Some(cursor)) = (self.first, live.cursor) else {
            return Vec::new();
        };
        let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
        corners(first, cursor, x, y)
            .and_then(|corners| Curve::polyline(corners, true).ok())
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn makes_a_closed_rectangle_of_the_right_size() {
        let mut doc = Document::default();
        run(&mut doc, &["rectangle", "1,1", "4,5"]);
        let curve = &doc.state().objects[0].curve;
        assert!((curve.length().unwrap() - 14.0).abs() < 1e-9);
        let points = curve.sample().unwrap();
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], points[4]);
        assert_eq!(points[2], [4.0, 5.0, 0.0]);
    }

    #[test]
    fn it_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["rectangle", "0,0,0", "3,2,0"]);
        let points = doc.state().objects[0].curve.sample().unwrap();
        assert_eq!(points[2], [3.0, 0.0, 2.0]);
        assert!(points.iter().all(|p| p[1] == 0.0));
    }

    #[test]
    fn corners_in_line_make_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["rectangle", "0,0", "5,0"]);
        assert!(doc.state().objects.is_empty());
        assert!(!processor.is_running());
    }
}
