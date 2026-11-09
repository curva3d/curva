//! `Circle`: a circle from its centre and radius.

use super::{ask_point, geometry_failed, guide, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, dot, sub};

/// The running command.
#[derive(Default)]
pub struct Circle {
    centre: Option<Point>,
    plane: Option<[Point; 2]>,
}

impl Command for Circle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.centre, answer) {
            (None, None) => ask_point("prompt.circle.centre", Vec::new()),
            (None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                self.plane = Some(doc.work_plane());
                Step::Ask(Request {
                    from: Some(centre),
                    preview: vec![centre],
                    ..Request::new("prompt.circle.radius", Kind::Number)
                })
            }
            (Some(centre), Some(Answer::Number(radius))) => {
                let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
                match Curve::circle_on(centre, radius, x, y) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::new("done.circle")))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(centre) = self.centre else {
            return Vec::new();
        };
        let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
        let reach = |at: Point| dot(sub(at, centre), sub(at, centre)).sqrt();
        let radius = live.number.or(live.cursor.map(reach));
        let mut shown: Vec<Curve> = radius
            .and_then(|radius| Curve::circle_on(centre, radius, x, y).ok())
            .into_iter()
            .collect();
        if let Some(cursor) = live.cursor {
            shown.extend(guide(centre, cursor));
        }
        shown
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
    fn a_circle_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["circle", "0,10", "5"]);
        let points = doc.state().objects[0].curve.sample().unwrap();
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
        let points = doc.state().objects[0].curve.sample().unwrap();
        assert!(points.iter().all(|p| p[0].abs() < 1e-9));
    }
    #[test]
    fn a_zero_radius_makes_nothing() {
        let mut doc = Document::default();
        run(&mut doc, &["c", "0,0", "0"]);
        assert!(doc.state().objects.is_empty());
    }
}
