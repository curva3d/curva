//! `Arc`, as in Rhino. By default from its centre: centre, start, then the end point, or a typed
//! angle in degrees. Option `StartPoint`: start, end, then a point the arc passes through.
//! Rhino's `Tangent`, `ExtendByArc` and `ExtendByLine` ways are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, cross, dot, sub};

const START_POINT: &str = "StartPoint";

/// The recipe.
#[derive(Default)]
pub struct Arc;

fn failed(error: &curva_kernel::KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

/// Coordinates of `p` on the plane through `origin` with axes `x` and `y`.
fn on_plane(p: Point, origin: Point, [x, y]: [Point; 2]) -> [f64; 2] {
    let d = sub(p, origin);
    [dot(d, x), dot(d, y)]
}

/// The angle of `p` about `centre` on the plane, from 0 to a full turn.
fn angle_of(p: Point, centre: Point, plane: [Point; 2]) -> f64 {
    let [u, v] = on_plane(p, centre, plane);
    v.atan2(u).rem_euclid(std::f64::consts::TAU)
}

/// The arc from `start` through `through` to `end`: the circle through the three points, on the
/// plane, swept the way that passes `through`.
fn three_point(
    start: Point,
    through: Point,
    end: Point,
    plane: [Point; 2],
) -> Option<(Point, Point, Point)> {
    let (centre, _) = super::circle_through(start, through, end, plane)?;
    // Arcs run anticlockwise from start to end; if that misses `through`, go the other way.
    let (a0, a1, am) = (
        angle_of(start, centre, plane),
        angle_of(end, centre, plane),
        angle_of(through, centre, plane),
    );
    let span = (a1 - a0).rem_euclid(std::f64::consts::TAU);
    let passes = (am - a0).rem_euclid(std::f64::consts::TAU) < span;
    Some(if passes {
        (centre, start, end)
    } else {
        (centre, end, start)
    })
}

impl Recipe for Arc {
    const NAME: &'static str = "arc";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(START_POINT)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let first = cx.values.first().and_then(Value::point);
        if cx.mode == Some(START_POINT) {
            let mut steps = vec![StepDef::point("prompt.arc.start_first").offering(&[])];
            if let Some(start) = first {
                steps.push(StepDef::point("prompt.arc.end_point").from(start));
                if let Some(end) = cx.values.get(1).and_then(Value::point) {
                    steps.push(StepDef::point("prompt.arc.through").from(end));
                }
            }
            return steps;
        }
        let mut steps = vec![StepDef::point("prompt.arc.centre")];
        if let Some(centre) = first {
            steps.push(
                StepDef::point("prompt.arc.start")
                    .from(centre)
                    .offering(&[]),
            );
            if cx.values.len() >= 2 {
                steps.push(
                    StepDef::point_or_number("prompt.arc.end")
                        .from(centre)
                        .offering(&[]),
                );
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let plane = cx.plane();
        let points: Vec<Option<Point>> = cx.values.iter().map(Value::point).collect();
        let mut made = Made::default();
        let guide = |a: Point, b: Point| Curve::line(a, b).ok();
        let arc = if cx.mode == Some(START_POINT) {
            match (
                points.first().copied().flatten(),
                points.get(1).copied().flatten(),
                points.get(2).copied().flatten(),
            ) {
                (Some(start), Some(end), Some(through)) => {
                    let Some((centre, a, b)) = three_point(start, through, end, plane) else {
                        return Err(Message::new("error.arc_points_in_line"));
                    };
                    Some(Curve::arc(centre, a, b, plane[0], plane[1]).map_err(|e| failed(&e))?)
                }
                (Some(start), Some(end), None) => {
                    made.guides.extend(guide(start, end));
                    None
                }
                _ => None,
            }
        } else {
            let centre = points.first().copied().flatten();
            let start = points.get(1).copied().flatten();
            match (centre, start, cx.values.get(2)) {
                (Some(centre), Some(start), Some(end)) => {
                    let end = match end {
                        Value::Point(p) => *p,
                        // A typed angle: the end turned that far from the start, anticlockwise.
                        Value::Number(degrees) => {
                            let normal = cross(plane[0], plane[1]);
                            let r = sub(start, centre);
                            let side = cross(normal, r);
                            let (sin, cos) = degrees.to_radians().sin_cos();
                            std::array::from_fn(|i| centre[i] + cos * r[i] + sin * side[i])
                        }
                        _ => return Err(Message::new("error.unexpected")),
                    };
                    let turning_back =
                        matches!(cx.values.get(2), Some(Value::Number(d)) if *d < 0.0);
                    made.guides.extend(guide(centre, start));
                    made.guides.extend(guide(centre, end));
                    let (a, b) = if turning_back {
                        (end, start)
                    } else {
                        (start, end)
                    };
                    Some(Curve::arc(centre, a, b, plane[0], plane[1]).map_err(|e| failed(&e))?)
                }
                (Some(centre), Some(start), None) => {
                    made.guides.extend(guide(centre, start));
                    None
                }
                _ => None,
            }
        };
        if let Some(arc) = arc {
            made.add.push(Shape::Curve(arc));
            made.message = Some(Message::new("done.arc"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn length(doc: &Document) -> f64 {
        doc.state().objects[0].curve().unwrap().length().unwrap()
    }

    #[test]
    fn makes_an_arc_of_the_right_length() {
        let mut doc = Document::default();
        run(&mut doc, &["arc", "0,0", "4,0", "0,9"]);
        assert!((length(&doc) - 4.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn it_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["arc", "0,0,0", "4,0,0", "0,4,0"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
    }

    #[test]
    fn points_that_make_no_arc_make_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["arc", "0,0", "0,0", "1,0"]);
        assert!(doc.state().objects.is_empty());
        assert!(!processor.is_running());
    }

    #[test]
    fn a_typed_angle_ends_the_arc() {
        let mut doc = Document::default();
        run(&mut doc, &["arc", "0,0", "5,0", "180"]);
        assert!((length(&doc) - 5.0 * PI).abs() < 1e-9);
        let mut doc = Document::default();
        run(&mut doc, &["arc", "0,0", "5,0", "-90"]);
        assert!((length(&doc) - 5.0 * FRAC_PI_2).abs() < 1e-9);
        let end = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(
            end.iter().all(|p| p[1] <= 1e-9),
            "turned clockwise, below the start"
        );
    }

    #[test]
    fn start_point_makes_the_arc_through_three_points() {
        let mut doc = Document::default();
        run(&mut doc, &["arc", "startpoint", "-5,0", "5,0", "0,5"]);
        // The upper half of a circle of radius 5.
        assert!((length(&doc) - 5.0 * PI).abs() < 1e-9);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1] >= -1e-9));
        // Through a point below, it goes the other way round.
        let mut doc = Document::default();
        run(&mut doc, &["arc", "startpoint", "-5,0", "5,0", "0,-5"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1] <= 1e-9));
    }
}
