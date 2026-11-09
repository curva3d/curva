//! `Arc`: an arc from its centre, start and end, on the plane being drawn on.

use super::{ask_point, geometry_failed, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// The running command.
#[derive(Default)]
pub struct Arc {
    centre: Option<Point>,
    start: Option<Point>,
    plane: Option<[Point; 2]>,
}

impl Command for Arc {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.centre, self.start, answer) {
            (None, None, None) => ask_point("prompt.arc.centre", Vec::new()),
            (None, None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                self.plane = Some(doc.work_plane());
                ask_point("prompt.arc.start", vec![centre])
            }
            (Some(centre), None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.arc.end", vec![centre])
            }
            (Some(centre), Some(start), Some(Answer::Point(end))) => {
                let [x, y] = self.plane.unwrap_or_else(|| doc.work_plane());
                match Curve::arc(centre, start, end, x, y) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::new("done.arc")))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn makes_an_arc_of_the_right_length() {
        let mut doc = Document::default();
        run(&mut doc, &["arc", "0,0", "4,0", "0,9"]);
        let length = doc.state().objects[0].curve.length().unwrap();
        assert!((length - 4.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn it_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["arc", "0,0,0", "4,0,0", "0,0,4"]);
        let points = doc.state().objects[0].curve.sample().unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
    }

    #[test]
    fn points_that_make_no_arc_make_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["arc", "0,0", "0,0", "1,0"]);
        assert!(doc.state().objects.is_empty());
        assert!(!processor.is_running());
    }
}
