//! `Scale`: resizes objects evenly about a point.

use super::{ask_objects, ask_point, ask_point_or_number, ghosts, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity, dot, sub};

/// Smallest size change treated as real.
const SMALLEST_FACTOR: f64 = 1e-9;

/// The running command.
#[derive(Default)]
pub struct Scale {
    ids: Option<Vec<u64>>,
    origin: Option<Point>,
    first: Option<Point>,
}

/// How much further `second` is from `origin` than `first` is.
fn ratio(origin: Point, first: Point, second: Point) -> f64 {
    let reach = |p: Point| {
        let offset = sub(p, origin);
        dot(offset, offset).sqrt()
    };
    reach(second) / reach(first)
}

impl Scale {
    fn finish(doc: &mut Document, ids: &[u64], origin: Point, factor: f64) -> Step {
        if !factor.is_finite() || factor < SMALLEST_FACTOR {
            return Step::Failed(Message::new("error.bad_factor"));
        }
        doc.transform(ids, &Similarity::scaling(origin, factor));
        Step::Done(Some(Message::with(
            "done.scale",
            vec![ids.len().to_string()],
        )))
    }
}

impl Command for Scale {
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
        match (self.origin, self.first, answer) {
            (None, None, Some(Answer::Point(origin))) => {
                self.origin = Some(origin);
                ask_point_or_number("prompt.scale.factor", vec![origin])
            }
            (Some(origin), None, Some(Answer::Number(factor))) => {
                Self::finish(doc, &ids, origin, factor)
            }
            (Some(origin), None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                ask_point("prompt.scale.second", vec![origin])
            }
            (Some(origin), Some(first), Some(Answer::Point(second))) => {
                Self::finish(doc, &ids, origin, ratio(origin, first, second))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(origin)) = (&self.ids, self.origin) else {
            return Vec::new();
        };
        let factor = match (self.first, live.number, live.cursor) {
            (None, Some(typed), _) => Some(typed),
            (Some(first), _, Some(cursor)) => Some(ratio(origin, first, cursor)),
            _ => None,
        };
        let mut shown = Vec::new();
        if let Some(factor) = factor.filter(|f| f.is_finite() && *f >= SMALLEST_FACTOR) {
            let change = Similarity::scaling(origin, factor);
            shown.extend(ghosts(doc, ids, |curve| curve.mapped(&change)));
        }
        if let Some(first) = self.first {
            shown.extend(guide(origin, first));
        }
        if let Some(cursor) = live.cursor {
            shown.extend(guide(origin, cursor));
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 0.0, 0.0], [3.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document) -> Vec<Point> {
        doc.state().objects[0].curve.sample().unwrap()
    }

    #[test]
    fn a_typed_factor_scales_about_the_origin() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "1,0", "2"]);
        assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [5.0, 0.0, 0.0]]);
    }

    #[test]
    fn two_reference_points_give_the_ratio_of_their_distances() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "0,0", "2,0", "0,6"]);
        assert_eq!(ends(&doc), [[3.0, 0.0, 0.0], [9.0, 0.0, 0.0]]);
    }

    #[test]
    fn a_circle_stays_a_circle() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale", "0,0", "3"]);
        let length = doc.state().objects[0].curve.length().unwrap();
        assert!((length - std::f64::consts::TAU * 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_or_negative_factor_changes_nothing() {
        for factor in ["0", "-2"] {
            let mut doc = selected_line();
            run(&mut doc, &["scale", "0,0", factor]);
            assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
        }
    }

    #[test]
    fn a_reference_point_on_the_origin_changes_nothing() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "0,0", "0,0", "2,0"]);
        assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
    }
}
