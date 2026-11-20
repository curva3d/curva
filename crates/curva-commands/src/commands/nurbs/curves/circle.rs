//! `Circle`, as in Rhino. By default from its centre, then a radius, typed or picked; at the radius,
//! `Diameter` takes the diameter instead. Options `2Point` (the ends of a diameter) and `3Point`
//! (three points on the circle). Rhino's other ways are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, gap};

const TWO_POINT: &str = "2Point";
const THREE_POINT: &str = "3Point";
const DIAMETER: &str = "Diameter";

/// The recipe.
#[derive(Default)]
pub struct Circle;

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

impl Recipe for Circle {
    const NAME: &'static str = "circle";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::mode(THREE_POINT),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let first = point(cx, 0);
        match cx.mode {
            Some(TWO_POINT) => {
                let mut steps = vec![StepDef::point("prompt.circle.diameter_start").offering(&[])];
                steps.extend(first.map(|a| {
                    StepDef::point("prompt.circle.diameter_end")
                        .from(a)
                        .offering(&[])
                }));
                steps
            }
            Some(THREE_POINT) => {
                let mut steps = vec![StepDef::point("prompt.circle.first").offering(&[])];
                if let Some(a) = first {
                    steps.push(StepDef::point("prompt.circle.second").from(a).offering(&[]));
                    if let Some(b) = point(cx, 1) {
                        steps.push(StepDef::point("prompt.circle.third").from(b).offering(&[]));
                    }
                }
                steps
            }
            _ => {
                let mut steps = vec![
                    StepDef::point("prompt.circle.centre").offering(&[TWO_POINT, THREE_POINT]),
                ];
                if let Some(centre) = first {
                    let prompt = if cx.toggle(DIAMETER) {
                        "prompt.circle.diameter"
                    } else {
                        "prompt.circle.radius"
                    };
                    steps.push(StepDef::number(prompt).from(centre).offering(&[DIAMETER]));
                }
                steps
            }
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let [x, y] = cx.plane();
        let (centre, radius, guides) = match cx.mode {
            Some(TWO_POINT) => match (point(cx, 0), point(cx, 1)) {
                (Some(a), Some(b)) => (
                    std::array::from_fn(|i| (a[i] + b[i]) / 2.0),
                    gap(a, b) / 2.0,
                    Curve::line(a, b).ok().into_iter().collect(),
                ),
                _ => return Ok(Made::default()),
            },
            Some(THREE_POINT) => match (point(cx, 0), point(cx, 1), point(cx, 2)) {
                (Some(a), Some(b), Some(c)) => {
                    let Some((centre, radius)) = crate::commands::circle_through(a, b, c, [x, y])
                    else {
                        return Err(Message::new("error.arc_points_in_line"));
                    };
                    (centre, radius, Vec::new())
                }
                (Some(a), Some(b), None) => {
                    return Ok(Made {
                        guides: Curve::line(a, b).ok().into_iter().collect(),
                        ..Made::default()
                    });
                }
                _ => return Ok(Made::default()),
            },
            _ => match (point(cx, 0), cx.values.get(1).and_then(Value::number)) {
                (Some(centre), Some(size)) => {
                    let radius = if cx.toggle(DIAMETER) {
                        size / 2.0
                    } else {
                        size
                    };
                    (centre, radius, Vec::new())
                }
                _ => return Ok(Made::default()),
            },
        };
        let circle = Curve::circle_on(centre, radius, x, y)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(circle)],
            guides,
            message: Some(Message::new("done.circle")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use std::f64::consts::TAU;

    fn length(doc: &Document) -> f64 {
        doc.state().objects[0].curve().unwrap().length().unwrap()
    }

    #[test]
    fn makes_a_circle_of_the_right_size() {
        let mut doc = Document::default();
        run(&mut doc, &["circle", "1,2,3", "5"]);
        assert!((length(&doc) - TAU * 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_circle_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["circle", "0,10", "5"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
        assert!(points.iter().any(|p| (p[2] - 15.0).abs() < 1e-6));
    }

    #[test]
    fn a_circle_keeps_the_plane_its_centre_was_picked_on() {
        let mut doc = Document::default();
        let mut processor = crate::Processor::default();
        doc.set_work_plane([[0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        processor.submit(&mut doc, "circle");
        processor.submit(&mut doc, "0,0,0");
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        processor.submit(&mut doc, "4");
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[0].abs() < 1e-9));
    }

    #[test]
    fn a_zero_radius_makes_nothing() {
        let mut doc = Document::default();
        run(&mut doc, &["c", "0,0", "0"]);
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn diameter_takes_the_diameter_and_is_remembered() {
        let mut doc = Document::default();
        run(&mut doc, &["circle", "0,0", "diameter", "10"]);
        run(&mut doc, &["circle", "20,0", "10"]);
        for object in &doc.state().objects {
            let length = object.curve().unwrap().length().unwrap();
            assert!((length - TAU * 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn two_point_and_three_point() {
        let mut doc = Document::default();
        run(&mut doc, &["circle", "2point", "-3,0", "3,0"]);
        assert!((length(&doc) - TAU * 3.0).abs() < 1e-9);
        let mut doc = Document::default();
        run(&mut doc, &["circle", "3point", "4,0", "0,4", "-4,0"]);
        assert!((length(&doc) - TAU * 4.0).abs() < 1e-9);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| (p[0].hypot(p[1]) - 4.0).abs() < 1e-6));
    }
}
