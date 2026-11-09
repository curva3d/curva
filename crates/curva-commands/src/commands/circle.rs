//! `Circle`: a circle from its centre and radius.

use super::{ask_point, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// The running command.
#[derive(Default)]
pub struct Circle {
    centre: Option<Point>,
}

impl Command for Circle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.centre, answer) {
            (None, None) => ask_point("prompt.circle.centre", Vec::new()),
            (None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                Step::Ask(Request {
                    from: Some(centre),
                    preview: vec![centre],
                    ..Request::new("prompt.circle.radius", Kind::Number)
                })
            }
            (Some(centre), Some(Answer::Number(radius))) => match Curve::circle(centre, radius) {
                Ok(curve) => {
                    doc.add(curve);
                    Step::Done(Some(Message::new("done.circle")))
                }
                Err(error) => geometry_failed(&error),
            },
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use std::f64::consts::TAU;

    #[test]
    fn makes_a_circle_of_the_right_size() {
        let mut doc = Document::default();
        run(&mut doc, &["circle", "1,2,3", "5"]);
        let length = doc.state().objects[0].curve.length().unwrap();
        assert!((length - TAU * 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_radius_makes_nothing() {
        let mut doc = Document::default();
        run(&mut doc, &["c", "0,0", "0"]);
        assert!(doc.state().objects.is_empty());
    }
}
