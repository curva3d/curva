//! `Revolve`: spins curves about an axis into surfaces, as in Rhino.

use super::{ask_objects, ask_point, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve, Point, Similarity, sub};

/// A whole turn, in degrees, the angle used unless another is typed.
const FULL_TURN: f64 = 360.0;
/// Copies of the curves shown around the axis in the preview.
const PREVIEW_STEPS: usize = 8;

/// The running command.
#[derive(Default)]
pub struct Revolve {
    ids: Option<Vec<u64>>,
    start: Option<Point>,
    end: Option<Point>,
}

impl Revolve {
    fn ask_angle() -> Step {
        Step::Ask(Request {
            prompt: Message::with("prompt.revolve.angle", vec![FULL_TURN.to_string()]),
            allow_empty: true,
            ..Request::new("prompt.revolve.angle", Kind::Number)
        })
    }

    fn make(&self, doc: &mut Document, degrees: f64) -> Step {
        let (Some(ids), Some(start), Some(end)) = (&self.ids, self.start, self.end) else {
            return unexpected();
        };
        let angle = degrees.to_radians();
        let mut made = Vec::new();
        for (id, curve) in curves_of(doc, ids) {
            // A closed curve turned part of the way is closed at its ends, as in Rhino.
            let cap = degrees < FULL_TURN && curve.is_closed().unwrap_or(false);
            match Body::revolved(&[curve], start, sub(end, start), angle, cap) {
                Ok(body) => made.push((id, body)),
                Err(error) => return geometry_failed(&error),
            }
        }
        let count = made.len();
        for (source, body) in made {
            let layer = doc.layer_of(source);
            doc.add_shape_on(Shape::Body(body), layer);
        }
        Step::Done(Some(Message::with("done.revolve", vec![count.to_string()])))
    }
}

impl Command for Revolve {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(_) = &self.ids else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            let curves: Vec<u64> = curves_of(doc, &chosen)
                .into_iter()
                .filter(|(_, curve)| !matches!(curve, Curve::Point { .. }))
                .map(|(id, _)| id)
                .collect();
            if curves.is_empty() {
                return ask_objects("prompt.revolve.objects");
            }
            self.ids = Some(curves);
            return ask_point("prompt.revolve.axis_start", Vec::new());
        };
        match (self.start, self.end, answer) {
            (None, _, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.revolve.axis_end", vec![start])
            }
            (Some(start), None, Some(Answer::Point(end))) => {
                if curva_kernel::gap(start, end) < 1e-9 {
                    return ask_point("prompt.revolve.axis_end", vec![start]);
                }
                self.end = Some(end);
                Self::ask_angle()
            }
            (Some(_), Some(_), Some(Answer::Empty)) => self.make(doc, FULL_TURN),
            (Some(_), Some(_), Some(Answer::Number(degrees)))
                if degrees > 0.0 && degrees <= FULL_TURN =>
            {
                self.make(doc, degrees)
            }
            (Some(_), Some(_), Some(Answer::Number(_))) => Self::ask_angle(),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let (Some(start), end) = (self.start, self.end.or(live.cursor)) else {
            return Vec::new();
        };
        let Some(end) = end else {
            return Vec::new();
        };
        let mut shown: Vec<Curve> = Curve::line(start, end).into_iter().collect();
        if self.end.is_none() {
            return shown;
        }
        let degrees = live
            .number
            .filter(|d| *d > 0.0 && *d <= FULL_TURN)
            .unwrap_or(FULL_TURN);
        let Some(axis) = curva_kernel::unit(sub(end, start)) else {
            return shown;
        };
        for step in 1..=PREVIEW_STEPS {
            let turn = Similarity::rotation(
                start,
                axis,
                (degrees * step as f64 / PREVIEW_STEPS as f64).to_radians(),
            );
            shown.extend(
                curves_of(doc, ids)
                    .iter()
                    .map(|(_, curve)| curve.mapped(&turn)),
            );
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn selected_profile(doc: &mut Document) {
        let id = doc.add(
            Curve::polyline(
                vec![
                    [3.0, 0.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [4.0, 0.0, 2.0],
                    [3.0, 0.0, 2.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[id]);
    }

    #[test]
    fn a_full_turn_about_an_axis_makes_a_ring() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        let mut processor = run(&mut doc, &["revolve", "0,0,0", "0,0,1"]);
        assert!(processor.preview(&doc, None, "").len() > 8);
        processor.submit(&mut doc, "");
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), PI * 7.0 * 2.0));
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn part_of_a_turn_closes_the_ends() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        run(&mut doc, &["revolve", "0,0,0", "0,0,1", "90"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), PI * 7.0 * 2.0 / 4.0));
    }

    #[test]
    fn an_angle_over_a_turn_is_asked_again() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        let processor = run(&mut doc, &["revolve", "0,0,0", "0,0,1", "400"]);
        assert!(processor.is_running());
    }
}
