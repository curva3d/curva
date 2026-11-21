//! `Scale2D`: resizes objects evenly across the plane being drawn on, but not out of it.

use crate::commands::{
    ask_objects, ask_point, ask_point_or_number, bodies_of, curves_of, guide, instances_of,
    objects_or_selection, opened, result_bodies, unexpected,
};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::{Document, Shape};
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
) -> Result<Vec<(u64, Shape)>, KernelError> {
    let [x, y] = doc.work_plane();
    let mut made = Vec::new();
    for (id, curve) in curves_of(doc, ids) {
        let curve = curve
            .stretched(origin, x, factor)?
            .stretched(origin, y, factor)?;
        made.push((id, Shape::Curve(curve)));
    }
    // Solids and surfaces scale too, as in Rhino (#507).
    for (id, body) in bodies_of(doc, ids) {
        let body = body
            .stretched(origin, x, factor)?
            .stretched(origin, y, factor)?;
        made.push((id, Shape::Body(body)));
    }
    // Blocks scale too, their placement taking the stretch both ways.
    let stretch = curva_document::blocks::followed_by(
        &curva_document::blocks::stretched_by(origin, x, factor),
        &curva_document::blocks::stretched_by(origin, y, factor),
    );
    for (id, instance) in instances_of(doc, ids) {
        let placement = curva_document::blocks::followed_by(&instance.placement, &stretch);
        made.push((
            id,
            Shape::Instance(curva_document::blocks::Instance {
                placement,
                ..instance
            }),
        ));
    }
    Ok(made)
}

fn finish(doc: &mut Document, ids: &[u64], origin: Point, factor: f64) -> Step {
    if !factor.is_finite() || factor < SMALLEST_FACTOR {
        return Step::Failed(Message::new("error.bad_factor"));
    }
    match scaled(doc, ids, origin, factor) {
        Ok(made) => {
            for (id, shape) in made {
                doc.replace_shape(id, shape);
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
            let Some(chosen) = objects_or_selection(doc, answer) else {
                return ask_objects("prompt.scale.objects");
            };
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
            let shapes: Vec<Shape> = made.into_iter().map(|(_, shape)| shape).collect();
            shown.extend(opened(doc, &shapes).iter().flat_map(Shape::outlines));
        }
        shown
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<curva_kernel::Body> {
        let (Some(ids), Some(origin)) = (&self.ids, self.origin) else {
            return Vec::new();
        };
        let factor = match (self.first, live.number, live.cursor) {
            (None, Some(typed), _) => Some(typed),
            (Some(first), _, Some(cursor)) => Some(reach(origin, cursor) / reach(origin, first)),
            _ => None,
        };
        factor
            .filter(|f| f.is_finite() && *f >= SMALLEST_FACTOR)
            .and_then(|f| scaled(doc, ids, origin, f).ok())
            .map(|made| {
                let shapes: Vec<Shape> = made.into_iter().map(|(_, shape)| shape).collect();
                result_bodies(&opened(doc, &shapes))
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_solid_scales_across_the_plane_too_as_in_rhino() {
        let mut doc = Document::default();
        let id = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale2d", "0,0", "2"]);
        let (low, high) = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((high[0] - low[0] - 2.0).abs() < 1e-6, "{low:?} {high:?}");
        assert!((high[2] - low[2] - 1.0).abs() < 1e-6, "height stays");
    }

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
