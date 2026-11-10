//! `Smooth`: evens out picked control points, or all the points of chosen curves, by moving
//! each part of the way towards the middle of its neighbours.

use super::{ask_objects, geometry_failed, ghost_results, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Grip};
use curva_kernel::{Curve, KernelError};

/// How far each point moves towards the middle of its neighbours unless another is typed.
const DEFAULT_FACTOR: f64 = 0.5;

/// The running command.
#[derive(Default)]
pub struct Smooth {
    grips: Option<Vec<Grip>>,
}

fn ask_factor() -> Step {
    Step::Ask(Request {
        prompt: Message::with("prompt.smooth.factor", vec![DEFAULT_FACTOR.to_string()]),
        allow_empty: true,
        ..Request::new("prompt.smooth.factor", Kind::Number)
    })
}

impl Smooth {
    fn smoothed(&self, doc: &Document, factor: f64) -> Result<Vec<(u64, Curve)>, KernelError> {
        doc.grips_smoothed(self.grips.as_deref().unwrap_or_default(), factor)
    }
}

impl Command for Smooth {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(grips) = self.grips.clone() else {
            let picked = doc.grip_selection();
            if answer.is_none() && !picked.is_empty() {
                self.grips = Some(picked);
                return ask_factor();
            }
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.smooth.objects");
            }
            // Every control point of the chosen curves; those without points are left out.
            let all: Vec<Grip> = doc
                .state()
                .objects
                .iter()
                .filter(|o| chosen.contains(&o.id))
                .flat_map(|o| {
                    let count = o.curve.control_points().map_or(0, |p| p.len());
                    (0..count).map(move |index| (o.id, index))
                })
                .collect();
            if all.is_empty() {
                return Step::Failed(Message::new("error.no_points"));
            }
            self.grips = Some(all);
            return ask_factor();
        };
        let factor = match answer {
            Some(Answer::Empty) => DEFAULT_FACTOR,
            Some(Answer::Number(factor)) if (0.0..=1.0).contains(&factor) => factor,
            Some(Answer::Number(_)) => return ask_factor(),
            _ => return unexpected(),
        };
        match self.smoothed(doc, factor) {
            Ok(made) => {
                for (id, curve) in made {
                    doc.replace(id, curve);
                }
                Step::Done(Some(Message::with(
                    "done.smooth",
                    vec![grips.len().to_string()],
                )))
            }
            Err(error) => geometry_failed(&error),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        if self.grips.is_none() {
            return Vec::new();
        }
        let factor = live
            .number
            .filter(|n| (0.0..=1.0).contains(n))
            .unwrap_or(DEFAULT_FACTOR);
        self.smoothed(doc, factor)
            .map(ghost_results)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn zigzag(doc: &mut Document) -> u64 {
        doc.add(
            Curve::polyline(
                vec![[0.0; 3], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0], [3.0, 2.0, 0.0]],
                false,
            )
            .unwrap(),
        )
    }

    #[test]
    fn a_selected_curve_is_smoothed_with_its_ends_kept() {
        let mut doc = Document::default();
        let id = zigzag(&mut doc);
        doc.select(&[id]);
        run(&mut doc, &["smooth", ""]);
        assert_eq!(
            doc.state().objects[0].curve.control_points().unwrap(),
            [[0.0; 3], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0], [3.0, 2.0, 0.0]]
        );
    }

    #[test]
    fn only_picked_points_move() {
        let mut doc = Document::default();
        let id = zigzag(&mut doc);
        doc.points_on(&[id]);
        doc.select_grips(&[(id, 1)]);
        run(&mut doc, &["smooth", "1"]);
        let points = doc.state().objects[0].curve.control_points().unwrap();
        assert_eq!(points[1], [1.0, 0.0, 0.0]);
        assert_eq!(points[2], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn a_factor_outside_nought_to_one_is_asked_again() {
        let mut doc = Document::default();
        let id = zigzag(&mut doc);
        doc.select(&[id]);
        let processor = run(&mut doc, &["smooth", "3"]);
        assert!(processor.is_running());
    }
}
