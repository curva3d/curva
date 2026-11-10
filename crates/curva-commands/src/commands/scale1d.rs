//! `Scale1D`: stretches objects in one direction.

use super::{
    ask_objects, ask_point, ask_point_or_number, curves_of, ghost_results, guide, unexpected,
};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, dot, sub, unit};

/// Smallest size change treated as real.
const SMALLEST_FACTOR: f64 = 1e-9;

/// The running command.
#[derive(Default)]
pub struct Scale1D {
    ids: Option<Vec<u64>>,
    origin: Option<Point>,
    first: Option<Point>,
    typed: Option<f64>,
}

fn reach(origin: Point, p: Point) -> f64 {
    let offset = sub(p, origin);
    dot(offset, offset).sqrt()
}

fn stretched(
    doc: &Document,
    ids: &[u64],
    origin: Point,
    axis: Point,
    factor: f64,
) -> Result<Vec<(u64, Curve)>, KernelError> {
    curves_of(doc, ids)
        .into_iter()
        .map(|(id, curve)| Ok((id, curve.stretched(origin, axis, factor)?)))
        .collect()
}

impl Scale1D {
    /// The direction and factor for a second point, as far as they are known.
    fn plan(&self, origin: Point, second: Point) -> Option<(Point, f64)> {
        let factor_ok = |factor: f64| factor.is_finite() && factor >= SMALLEST_FACTOR;
        if let Some(factor) = self.typed {
            return Some((unit(sub(second, origin))?, factor)).filter(|(_, f)| factor_ok(*f));
        }
        let first = self.first?;
        let factor = reach(origin, second) / reach(origin, first);
        Some((unit(sub(first, origin))?, factor)).filter(|(_, f)| factor_ok(*f))
    }
}

impl Command for Scale1D {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.scale.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.scale.origin", Vec::new());
        };
        let Some(origin) = self.origin else {
            let Some(Answer::Point(origin)) = answer else {
                return unexpected();
            };
            self.origin = Some(origin);
            return ask_point_or_number("prompt.scale.factor", vec![origin]);
        };
        match (self.first, self.typed, answer) {
            (None, None, Some(Answer::Number(factor))) => {
                self.typed = Some(factor);
                ask_point("prompt.scale1d.direction", vec![origin])
            }
            (None, None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                ask_point("prompt.scale.second", vec![origin])
            }
            (_, _, Some(Answer::Point(second))) => {
                let Some((axis, factor)) = self.plan(origin, second) else {
                    return Step::Failed(Message::new("error.bad_factor"));
                };
                match stretched(doc, &ids, origin, axis, factor) {
                    Ok(made) => {
                        for (id, curve) in made {
                            doc.replace(id, curve);
                        }
                        Step::Done(Some(Message::with(
                            "done.scale",
                            vec![ids.len().to_string()],
                        )))
                    }
                    Err(error) => super::geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(origin), Some(cursor)) = (&self.ids, self.origin, live.cursor) else {
            return Vec::new();
        };
        let mut shown = guide(origin, cursor);
        if let Some((axis, factor)) = self.plan(origin, cursor)
            && let Ok(made) = stretched(doc, ids, origin, axis, factor)
        {
            shown.extend(ghost_results(made));
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_diagonal() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 1.0, 0.0], [2.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document) -> Vec<Point> {
        doc.state().objects[0].curve().unwrap().sample().unwrap()
    }

    #[test]
    fn a_typed_factor_then_a_direction_stretches_that_way_only() {
        let mut doc = selected_diagonal();
        run(&mut doc, &["scale1d", "0,0", "2", "9,0"]);
        assert_eq!(ends(&doc), [[2.0, 1.0, 0.0], [4.0, 2.0, 0.0]]);
    }

    #[test]
    fn two_reference_points_give_the_direction_and_ratio() {
        let mut doc = selected_diagonal();
        run(&mut doc, &["scale1d", "0,0", "0,1", "0,3"]);
        assert_eq!(ends(&doc), [[1.0, 3.0, 0.0], [2.0, 6.0, 0.0]]);
    }

    #[test]
    fn a_circle_becomes_an_ellipse() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale1d", "0,0", "3", "1,0"]);
        for point in ends(&doc) {
            let value = (point[0] / 6.0).powi(2) + (point[1] / 2.0).powi(2);
            assert!((value - 1.0).abs() < 1e-6, "{point:?}");
        }
    }

    #[test]
    fn a_bad_factor_changes_nothing() {
        let mut doc = selected_diagonal();
        run(&mut doc, &["scale1d", "0,0", "-2", "9,0"]);
        assert_eq!(ends(&doc)[0], [1.0, 1.0, 0.0]);
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_diagonal();
        run(&mut doc, &["scale1d", "0,0", "2", "9,0", "undo"]);
        assert_eq!(ends(&doc)[0], [1.0, 1.0, 0.0]);
    }
}
