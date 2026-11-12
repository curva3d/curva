//! `Rectangle`, as in Rhino: two opposite corners on the plane being drawn on. Option `Center`
//! takes the centre and a corner; `3Point` takes two corners along one side, then a point on the
//! opposite side, so the rectangle can lie at any angle. Rhino's `EdgeMidpoint`, `Vertical`,
//! `AroundCurve` and `Rounded` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, cross, dot, sub, unit};

/// Shortest side treated as real, in millimetres.
const SMALLEST_SIDE: f64 = 1e-9;
const CENTER: &str = "Center";
const THREE_POINT: &str = "3Point";

/// The recipe.
#[derive(Default)]
pub struct Rectangle;

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    std::array::from_fn(|i| p[i] + direction[i] * amount)
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

/// The four corners from one side, `a` to `b`, and a point `c` on the opposite side.
fn from_side(a: Point, b: Point, c: Point, [x, y]: [Point; 2]) -> Option<Vec<Point>> {
    let along = unit(sub(b, a))?;
    let across = cross(cross(x, y), along);
    let high = dot(sub(c, a), across);
    (high.abs() >= SMALLEST_SIDE).then(|| vec![a, b, plus(b, across, high), plus(a, across, high)])
}

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn ring(cx: &Context<'_>) -> Option<Vec<Point>> {
    let [x, y] = cx.plane();
    let (a, b) = (point(cx, 0)?, point(cx, 1)?);
    match cx.mode {
        Some(THREE_POINT) => from_side(a, b, point(cx, 2)?, [x, y]),
        Some(CENTER) => corners(std::array::from_fn(|i| 2.0 * a[i] - b[i]), b, x, y),
        _ => corners(a, b, x, y),
    }
}

impl Recipe for Rectangle {
    const NAME: &'static str = "rectangle";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(CENTER), OptionDef::mode(THREE_POINT)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let first_prompt = match cx.mode {
            Some(CENTER) => "prompt.rectangle.centre",
            _ => "prompt.rectangle.first",
        };
        let mut steps = vec![StepDef::point(first_prompt)];
        if let Some(a) = point(cx, 0) {
            let other = if cx.mode == Some(THREE_POINT) {
                "prompt.rectangle.side_end"
            } else {
                "prompt.rectangle.other"
            };
            let step = StepDef::point(other).from(a).offering(&[]);
            // Two opposite corners show width × depth; 3Point's side does not.
            steps.push(if cx.mode == Some(THREE_POINT) {
                step
            } else {
                step.corner()
            });
            if cx.mode == Some(THREE_POINT)
                && let Some(b) = point(cx, 1)
            {
                steps.push(
                    StepDef::point("prompt.rectangle.opposite")
                        .from(b)
                        .offering(&[]),
                );
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        let complete = if cx.mode == Some(THREE_POINT) { 3 } else { 2 };
        if cx.values.len() >= complete && ring(cx).is_none() {
            return Err(Message::new("error.flat_rectangle"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(corners) = ring(cx) else {
            // While picking the third point of `3Point`, show the first side.
            let side = match (point(cx, 0), point(cx, 1)) {
                (Some(a), Some(b)) if cx.mode == Some(THREE_POINT) => Curve::line(a, b).ok(),
                _ => None,
            };
            return Ok(Made {
                guides: side.into_iter().collect(),
                ..Made::default()
            });
        };
        let rectangle = Curve::polyline(corners, true)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(rectangle)],
            message: Some(Message::new("done.rectangle")),
            ..Made::default()
        })
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
        let curve = &doc.state().objects[0].curve().unwrap();
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
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
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

    #[test]
    fn center_takes_the_middle_first() {
        let mut doc = Document::default();
        run(&mut doc, &["rectangle", "center", "0,0", "3,2"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert_eq!(points[0], [-3.0, -2.0, 0.0]);
        assert_eq!(points[2], [3.0, 2.0, 0.0]);
    }

    #[test]
    fn three_point_lies_at_any_angle() {
        let mut doc = Document::default();
        run(&mut doc, &["rectangle", "3point", "0,0", "3,4", "-4,3"]);
        let curve = doc.state().objects[0].curve().unwrap();
        // A side of 5 and a side of 5, turned to follow the first side.
        assert!((curve.length().unwrap() - 20.0).abs() < 1e-9);
        let points = curve.sample().unwrap();
        assert!(
            curva_kernel::gap(points[2], [-1.0, 7.0, 0.0]) < 1e-9,
            "{points:?}"
        );
    }
}
