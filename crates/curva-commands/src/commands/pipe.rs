//! `Pipe`: a round tube along curves, as in Rhino. Options `Cap` (closed ends) and `Thick`
//! (a wall between two radii).

use super::{ask_objects, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve, cross, sub, unit};

/// The radius used until another is typed, in millimetres.
const STARTING_RADIUS: f64 = 1.0;
const RADIUS: &str = "pipe.radius";
const CAP: &str = "pipe.cap";
const THICK: &str = "pipe.thick";
const CAP_OPTION: &str = "Cap";
const THICK_OPTION: &str = "Thick";
/// Rings shown along each curve in the preview.
const PREVIEW_RINGS: usize = 6;

/// The running command.
#[derive(Default)]
pub struct Pipe {
    ids: Option<Vec<u64>>,
    /// The outer radius, once given, while the inner one of a thick pipe is asked for.
    outer: Option<f64>,
}

fn on(doc: &Document, key: &str, starting: bool) -> bool {
    doc.remembered(key).map_or(starting, |value| value != 0.0)
}

fn yes_no(value: bool) -> String {
    if value { "Yes" } else { "No" }.to_owned()
}

fn radius_of(doc: &Document) -> f64 {
    doc.remembered(RADIUS).unwrap_or(STARTING_RADIUS)
}

impl Pipe {
    fn ask_radius(doc: &Document) -> Step {
        Step::Ask(Request {
            prompt: Message::with(
                "prompt.pipe.radius",
                vec![
                    radius_of(doc).to_string(),
                    yes_no(on(doc, CAP, true)),
                    yes_no(on(doc, THICK, false)),
                ],
            ),
            options: vec![CAP_OPTION, THICK_OPTION],
            allow_empty: true,
            ..Request::new("prompt.pipe.radius", Kind::Number)
        })
    }

    fn ask_inner(outer: f64) -> Step {
        Step::Ask(Request {
            prompt: Message::with("prompt.pipe.inner", vec![(outer / 2.0).to_string()]),
            allow_empty: true,
            ..Request::new("prompt.pipe.inner", Kind::Number)
        })
    }

    fn make(&self, doc: &mut Document, radius: f64, inner: Option<f64>) -> Step {
        let Some(ids) = &self.ids else {
            return unexpected();
        };
        let cap = on(doc, CAP, true);
        let mut made = Vec::new();
        for (id, rail) in curves_of(doc, ids) {
            match Body::pipe(&rail, radius, inner, cap) {
                Ok(body) => made.push((id, body)),
                Err(error) => return geometry_failed(&error),
            }
        }
        let count = made.len();
        for (source, body) in made {
            let layer = doc.layer_of(source);
            doc.add_shape_on(Shape::Body(body), layer);
        }
        Step::Done(Some(Message::with("done.pipe", vec![count.to_string()])))
    }

    /// Given the outer radius: a thick pipe asks for its inner one, a plain pipe is made.
    fn outer_given(&mut self, doc: &mut Document, radius: f64) -> Step {
        if on(doc, THICK, false) {
            self.outer = Some(radius);
            return Self::ask_inner(radius);
        }
        self.make(doc, radius, None)
    }
}

impl Command for Pipe {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.ids.is_none() {
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
                return ask_objects("prompt.pipe.objects");
            }
            self.ids = Some(curves);
            return Self::ask_radius(doc);
        }
        if let Some(outer) = self.outer {
            return match answer {
                Some(Answer::Empty) => self.make(doc, outer, Some(outer / 2.0)),
                Some(Answer::Number(inner)) if inner > 0.0 && inner < outer => {
                    self.make(doc, outer, Some(inner))
                }
                Some(Answer::Number(_)) => Self::ask_inner(outer),
                _ => unexpected(),
            };
        }
        match answer {
            Some(Answer::Option(CAP_OPTION)) => {
                let flipped = !on(doc, CAP, true);
                doc.remember(CAP, f64::from(u8::from(flipped)));
                Self::ask_radius(doc)
            }
            Some(Answer::Option(_)) => {
                let flipped = !on(doc, THICK, false);
                doc.remember(THICK, f64::from(u8::from(flipped)));
                Self::ask_radius(doc)
            }
            Some(Answer::Empty) => {
                let radius = radius_of(doc);
                self.outer_given(doc, radius)
            }
            Some(Answer::Number(radius)) if radius > 0.0 => {
                doc.remember(RADIUS, radius);
                self.outer_given(doc, radius)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let typed = live.number.filter(|n| *n > 0.0);
        let radii: Vec<f64> = match self.outer {
            Some(outer) => vec![outer, typed.filter(|n| *n < outer).unwrap_or(outer / 2.0)],
            None => vec![typed.unwrap_or_else(|| radius_of(doc))],
        };
        // Rings across each curve, square to it, at points spread along it.
        let mut shown = Vec::new();
        for (_, rail) in curves_of(doc, ids) {
            let Ok(points) = rail.sample() else {
                continue;
            };
            let step = (points.len() / PREVIEW_RINGS).max(1);
            for (index, at) in points.iter().enumerate().step_by(step) {
                let next = points.get(index + 1).or(points.get(index.wrapping_sub(1)));
                let Some(along) = next.and_then(|n| unit(sub(*n, *at))) else {
                    continue;
                };
                let Some(x) = unit(cross(along, [0.0, 0.0, 1.0]))
                    .or_else(|| unit(cross(along, [1.0, 0.0, 0.0])))
                else {
                    continue;
                };
                let y = cross(along, x);
                for radius in &radii {
                    shown.extend(Curve::circle_on(*at, *radius, x, y));
                }
            }
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

    fn selected_rail(doc: &mut Document) {
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
    }

    #[test]
    fn a_pipe_follows_the_curve_with_the_radius_typed() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        let mut processor = run(&mut doc, &["pipe"]);
        assert!(!processor.preview(&doc, None, "2").is_empty());
        processor.submit(&mut doc, "2");
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!((body.volume().unwrap() - PI * 4.0 * 10.0).abs() < 1e-2);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn a_thick_pipe_has_a_wall() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        run(&mut doc, &["pipe", "Thick", "2", "1"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!((body.volume().unwrap() - PI * 3.0 * 10.0).abs() < 1e-2);
    }

    #[test]
    fn an_inner_radius_wider_than_the_outer_is_asked_again() {
        let mut doc = Document::default();
        selected_rail(&mut doc);
        let processor = run(&mut doc, &["pipe", "t", "2", "3"]);
        assert!(processor.is_running());
    }
}
