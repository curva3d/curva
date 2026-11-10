//! `Box`: a solid box from two corners of its base on the plane being drawn on, then a height,
//! as in Rhino.

use super::{ask_point, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Body, Curve, Point, cross, dot, gap, sub};

/// Shortest side treated as real, in millimetres.
const SMALLEST_SIDE: f64 = 1e-9;

/// The running command.
#[derive(Default)]
pub struct BoxSolid {
    first: Option<Point>,
    /// The base's width and depth along the plane, once its other corner is known.
    base: Option<[f64; 2]>,
    plane: Option<[Point; 2]>,
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [0, 1, 2].map(|k| p[k] + direction[k] * amount)
}

impl BoxSolid {
    fn axes(&self, doc: &Document) -> [Point; 3] {
        let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
        [x, y, cross(x, y)]
    }

    /// Width and depth of the base from its first corner to `other`, along the plane.
    fn sides(&self, doc: &Document, other: Point) -> Option<[f64; 2]> {
        let first = self.first?;
        let [x, y, _] = self.axes(doc);
        let across = sub(other, first);
        let sides = [dot(across, x), dot(across, y)];
        sides
            .iter()
            .all(|side| side.abs() >= SMALLEST_SIDE)
            .then_some(sides)
    }

    /// The twelve edges of the box, for its preview.
    fn edges(&self, doc: &Document, [wide, deep]: [f64; 2], high: f64) -> Vec<Curve> {
        let Some(first) = self.first else {
            return Vec::new();
        };
        let [x, y, z] = self.axes(doc);
        let corner = |a: f64, b: f64, c: f64| plus(plus(plus(first, x, a), y, b), z, c);
        let ring = |c: f64| {
            [
                corner(0.0, 0.0, c),
                corner(wide, 0.0, c),
                corner(wide, deep, c),
                corner(0.0, deep, c),
            ]
        };
        let (bottom, top) = (ring(0.0), ring(high));
        let mut made = Vec::new();
        for i in 0..4 {
            made.extend(Curve::line(bottom[i], bottom[(i + 1) % 4]));
            if high.abs() >= SMALLEST_SIDE {
                made.extend(Curve::line(top[i], top[(i + 1) % 4]));
                made.extend(Curve::line(bottom[i], top[i]));
            }
        }
        made
    }

    fn ask_height(&self) -> Step {
        Step::Ask(Request {
            from: self.first,
            ..Request::new("prompt.box.height", Kind::Number)
        })
    }
}

impl Command for BoxSolid {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.first, self.base, answer) {
            (None, _, None) => ask_point("prompt.box.first", Vec::new()),
            (None, _, Some(Answer::Point(first))) => {
                self.first = Some(first);
                self.plane = Some(doc.work_plane());
                ask_point("prompt.box.other", vec![first])
            }
            (Some(_), None, Some(Answer::Point(other))) => match self.sides(doc, other) {
                Some(sides) => {
                    self.base = Some(sides);
                    self.ask_height()
                }
                None => Step::Failed(Message::new("error.flat_rectangle")),
            },
            (Some(first), Some([wide, deep]), Some(Answer::Number(high))) => {
                if high.abs() < SMALLEST_SIDE {
                    return Step::Failed(Message::new("error.flat_box"));
                }
                let [x, y, _] = self.axes(doc);
                match Body::cuboid_on(first, x, y, [wide, deep, high]) {
                    Ok(body) => {
                        doc.add_body(body);
                        Step::Done(Some(Message::new("done.box")))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        match (self.first, self.base) {
            (Some(_), None) => live
                .cursor
                .and_then(|cursor| self.sides(doc, cursor))
                .map(|sides| self.edges(doc, sides, 0.0))
                .unwrap_or_default(),
            (Some(first), Some(sides)) => {
                // A typed height wins; otherwise the cursor's distance from the first corner.
                let high = live
                    .number
                    .or_else(|| live.cursor.map(|cursor| gap(cursor, first)))
                    .unwrap_or(0.0);
                self.edges(doc, sides, high)
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    #[test]
    fn two_corners_and_a_height_make_a_solid_box() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["box", "0,0", "10,20"]);
        assert_eq!(processor.preview(&doc, None, "5").len(), 12);
        processor.submit(&mut doc, "5");
        let body = doc.state().objects[0].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), 1000.0));
        let (low, high) = body.bounds().unwrap();
        assert!(close(low[2], 0.0) && close(high[2], 5.0));
        processor.submit(&mut doc, "undo");
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn a_negative_height_goes_below_the_plane() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "0,0", "2,2", "-3"]);
        let (low, high) = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!(close(low[2], -3.0) && close(high[2], 0.0));
    }

    #[test]
    fn a_flat_base_is_refused() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["box", "0,0", "5,0"]);
        assert!(!processor.is_running());
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn a_box_can_be_moved_rotated_and_copied() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "0,0", "2,3", "4"]);
        let id = doc.state().objects[0].id;
        doc.select(&[id]);
        run(&mut doc, &["rotate", "0,0", "30"]);
        run(&mut doc, &["move", "0,0", "10,0"]);
        run(&mut doc, &["copy", "0,0", "0,10", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        for object in &doc.state().objects {
            assert!(close(object.shape.body().unwrap().volume().unwrap(), 24.0));
        }
    }
}
