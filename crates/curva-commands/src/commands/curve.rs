//! `Curve`: a control-point curve, as in Rhino. Click points, Enter to finish. Options `Degree`
//! (remembered) and `Close`.

use super::{geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve as Shape, Point};

const DEGREE: &str = "curve.degree";
const DEGREE_OPTION: &str = "Degree";
const CLOSE_OPTION: &str = "Close";
const STARTING_DEGREE: usize = 3;
const MOST_DEGREE: usize = 11;

/// The running command.
#[derive(Default)]
pub struct CurveCommand {
    points: Vec<Point>,
    /// Waiting for a degree to be typed.
    asking_degree: bool,
}

fn degree_of(doc: &Document) -> usize {
    doc.remembered(DEGREE)
        .map_or(STARTING_DEGREE, |value| value as usize)
}

impl CurveCommand {
    fn ask_point(&self, doc: &Document) -> Step {
        let count = self.points.len();
        let mut options = vec![DEGREE_OPTION];
        if count >= 3 {
            options.push(CLOSE_OPTION);
        }
        let key = if count == 0 {
            "prompt.curve.start"
        } else {
            "prompt.curve.next"
        };
        Step::Ask(Request {
            prompt: Message::with(key, vec![degree_of(doc).to_string()]),
            allow_empty: count >= 2,
            options,
            preview: self.points.clone(),
            ..Request::new(key, Kind::Point)
        })
    }

    fn finish(&self, doc: &mut Document, closed: bool) -> Step {
        match Shape::control_point(self.points.clone(), degree_of(doc), closed) {
            Ok(curve) => {
                doc.add(curve);
                Step::Done(Some(Message::with(
                    "done.curve",
                    vec![self.points.len().to_string()],
                )))
            }
            Err(error) => geometry_failed(&error),
        }
    }
}

impl Command for CurveCommand {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.asking_degree {
            return match answer {
                Some(Answer::Number(typed))
                    if typed.fract() == 0.0 && (1.0..=MOST_DEGREE as f64).contains(&typed) =>
                {
                    doc.remember(DEGREE, typed);
                    self.asking_degree = false;
                    self.ask_point(doc)
                }
                Some(Answer::Empty) => {
                    self.asking_degree = false;
                    self.ask_point(doc)
                }
                Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_degree")),
                _ => unexpected(),
            };
        }
        match answer {
            None => self.ask_point(doc),
            Some(Answer::Point(point)) => {
                self.points.push(point);
                self.ask_point(doc)
            }
            Some(Answer::Option(DEGREE_OPTION)) => {
                self.asking_degree = true;
                Step::Ask(Request {
                    prompt: Message::with("prompt.curve.degree", vec![degree_of(doc).to_string()]),
                    allow_empty: true,
                    ..Request::new("prompt.curve.degree", Kind::Number)
                })
            }
            Some(Answer::Option(_)) => self.finish(doc, true),
            Some(Answer::Empty) => self.finish(doc, false),
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Shape> {
        let through: Vec<Point> = self.points.iter().copied().chain(live.cursor).collect();
        Shape::control_point(through, degree_of(doc), false)
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects[0].curve().unwrap().clone()
    }

    #[test]
    fn has_one_control_point_per_click_and_runs_from_first_to_last() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "0,0", "5,8", "10,-3", "15,2", "20,0", ""],
        );
        let curve = only_curve(&doc);
        let Curve::Nurbs { points, degree, .. } = &curve else {
            panic!("not a control-point curve");
        };
        assert_eq!((points.len(), *degree), (5, 3));
        let path = curve.sample().unwrap();
        assert!(gap(path[0], [0.0, 0.0, 0.0]) < 1e-6);
        assert!(gap(path[path.len() - 1], [20.0, 0.0, 0.0]) < 1e-6);
    }

    #[test]
    fn close_joins_it_round() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "0,0", "10,0", "10,10", "0,10", "close"],
        );
        let path = only_curve(&doc).sample().unwrap();
        assert!(gap(path[0], path[path.len() - 1]) < 1e-6);
    }

    #[test]
    fn the_degree_is_an_option_and_is_remembered() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "degree", "2", "0,0", "5,5", "10,0", "15,5", ""],
        );
        run(&mut doc, &["curve", "0,0", "5,5", "10,0", "15,5", ""]);
        for object in &doc.state().objects {
            assert!(matches!(
                object.curve(),
                Some(Curve::Nurbs { degree: 2, .. })
            ));
        }
    }

    #[test]
    fn a_degree_out_of_range_is_refused() {
        let mut doc = Document::default();
        run(&mut doc, &["curve", "degree", "12"]);
        assert!(doc.state().objects.is_empty());
    }
}
