//! `Scale1D`: stretches objects in one direction.

use crate::commands::{
    ask_objects, ask_point, ask_point_or_number, bodies_of, curves_of, guide, instances_of,
    objects_or_selection, opened, result_bodies, unexpected,
};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::{Document, Shape};
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
) -> Result<Vec<(u64, Shape)>, KernelError> {
    let mut made = Vec::new();
    for (id, curve) in curves_of(doc, ids) {
        made.push((id, Shape::Curve(curve.stretched(origin, axis, factor)?)));
    }
    // Solids and surfaces stretch too, as in Rhino (#507).
    for (id, body) in bodies_of(doc, ids) {
        made.push((id, Shape::Body(body.stretched(origin, axis, factor)?)));
    }
    // Blocks stretch too, their placement taking the stretch.
    let stretch = curva_document::blocks::stretched_by(origin, axis, factor);
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
            let Some(chosen) = objects_or_selection(doc, answer) else {
                return ask_objects("prompt.scale.objects");
            };
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
            let shapes: Vec<Shape> = made.into_iter().map(|(_, shape)| shape).collect();
            shown.extend(opened(doc, &shapes).iter().flat_map(Shape::outlines));
        }
        shown
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<curva_kernel::Body> {
        let (Some(ids), Some(origin), Some(cursor)) = (&self.ids, self.origin, live.cursor) else {
            return Vec::new();
        };
        self.plan(origin, cursor)
            .and_then(|(axis, factor)| stretched(doc, ids, origin, axis, factor).ok())
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
    fn a_solid_stretches_too_as_in_rhino() {
        let mut doc = Document::default();
        let id = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale1d", "0,0", "2", "9,0"]);
        let (low, high) = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((high[0] - low[0] - 2.0).abs() < 1e-6, "{low:?} {high:?}");
        assert!((high[1] - low[1] - 1.0).abs() < 1e-6, "{low:?} {high:?}");
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
