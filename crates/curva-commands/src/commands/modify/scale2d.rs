//! `Scale2D`: resizes objects evenly across the plane being drawn on, but not out of it.

use crate::commands::{
    ask_objects, ask_point, ask_point_or_number, curves_of, ghost_results, guide, unexpected,
};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, dot, sub};

/// Smallest size change treated as real.
const SMALLEST_FACTOR: f64 = 1e-9;

/// The running command.
#[derive(Default)]
pub struct Scale2D {
    ids: Option<Vec<u64>>,
    origin: Option<Point>,
    first: Option<Point>,
}

fn reach(origin: Point, p: Point) -> f64 {
    let offset = sub(p, origin);
    dot(offset, offset).sqrt()
}

fn scaled(
    doc: &Document,
    ids: &[u64],
    origin: Point,
    factor: f64,
) -> Result<Vec<(u64, Curve)>, KernelError> {
    let [x, y] = doc.work_plane();
    curves_of(doc, ids)
        .into_iter()
        .map(|(id, curve)| {
            Ok((
                id,
                curve
                    .stretched(origin, x, factor)?
                    .stretched(origin, y, factor)?,
            ))
        })
        .collect()
}

fn finish(doc: &mut Document, ids: &[u64], origin: Point, factor: f64) -> Step {
    if !factor.is_finite() || factor < SMALLEST_FACTOR {
        return Step::Failed(Message::new("error.bad_factor"));
    }
    match scaled(doc, ids, origin, factor) {
        Ok(made) => {
            for (id, curve) in made {
                doc.replace(id, curve);
            }
            Step::Done(Some(Message::with(
                "done.scale",
                vec![ids.len().to_string()],
            )))
        }
        Err(error) => crate::commands::geometry_failed(&error),
    }
}

impl Command for Scale2D {
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
            (Some(origin), None, Some(Answer::Number(factor))) => finish(doc, &ids, origin, factor),
            (Some(origin), None, Some(Answer::Point(first))) => {
                self.first = Some(first);
                ask_point("prompt.scale.second", vec![origin])
            }
            (Some(origin), Some(first), Some(Answer::Point(second))) => finish(
                doc,
                &ids,
                origin,
                reach(origin, second) / reach(origin, first),
            ),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(origin)) = (&self.ids, self.origin) else {
            return Vec::new();
        };
        let factor = match (self.first, live.number, live.cursor) {
            (None, Some(typed), _) => Some(typed),
            (Some(first), _, Some(cursor)) => Some(reach(origin, cursor) / reach(origin, first)),
            _ => None,
        };
        let mut shown = live.cursor.map_or_else(Vec::new, |c| guide(origin, c));
        if let Some(made) = factor
            .filter(|f| f.is_finite() && *f >= SMALLEST_FACTOR)
            .and_then(|f| scaled(doc, ids, origin, f).ok())
        {
            shown.extend(ghost_results(made));
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_scales_across_the_plane_and_leaves_height_alone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 1.0, 5.0], [2.0, 1.0, 5.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale2d", "0,0", "2"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert_eq!(points, [[2.0, 2.0, 5.0], [4.0, 2.0, 5.0]]);
    }

    #[test]
    fn it_follows_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 7.0, 1.0], [2.0, 7.0, 1.0]).unwrap());
        doc.select(&[id]);
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["scale2d", "0,0", "2"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert_eq!(points, [[2.0, 7.0, 2.0], [4.0, 7.0, 2.0]]);
    }

    #[test]
    fn a_bad_factor_changes_nothing() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 1.0, 0.0], [2.0, 1.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale2d", "0,0", "0"]);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [1.0, 1.0, 0.0]
        );
    }
}
