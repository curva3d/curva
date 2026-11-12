//! `ScaleNU`: resizes objects by a different factor along each axis of the construction
//! plane, about a base point, as in Rhino. Enter keeps a factor at 1. Option `Copy`.

use super::{ask_objects, ask_point, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Curve, KernelError, Point, cross};

/// Smallest size change treated as real.
const SMALLEST_FACTOR: f64 = 1e-9;
const COPY: &str = "scalenu.copy";
const COPY_OPTION: &str = "Copy";
const PROMPTS: [&str; 3] = ["prompt.scalenu.x", "prompt.scalenu.y", "prompt.scalenu.z"];

/// The running command.
#[derive(Default)]
pub struct ScaleNu {
    ids: Option<Vec<u64>>,
    origin: Option<Point>,
    /// The factors given so far, X first.
    factors: Vec<f64>,
}

fn copying(doc: &Document) -> bool {
    doc.remembered(COPY).is_some_and(|value| value != 0.0)
}

/// The construction plane's three directions: its X, its Y and straight out of it.
fn axes(doc: &Document) -> [Point; 3] {
    let [x, y] = doc.work_plane();
    [x, y, cross(x, y)]
}

fn stretched_curve(
    curve: &Curve,
    origin: Point,
    axes: &[Point; 3],
    factors: &[f64],
) -> Result<Curve, KernelError> {
    let mut made = curve.clone();
    for (axis, factor) in axes.iter().zip(factors) {
        if (factor - 1.0).abs() > f64::EPSILON {
            made = made.stretched(origin, *axis, *factor)?;
        }
    }
    Ok(made)
}

fn stretched_shape(
    shape: &Shape,
    origin: Point,
    axes: &[Point; 3],
    factors: &[f64],
) -> Result<Shape, KernelError> {
    match shape {
        Shape::Curve(curve) => Ok(Shape::Curve(stretched_curve(curve, origin, axes, factors)?)),
        Shape::Body(body) => {
            let mut made = body.clone();
            for (axis, factor) in axes.iter().zip(factors) {
                if (factor - 1.0).abs() > f64::EPSILON {
                    made = made.stretched(origin, *axis, *factor)?;
                }
            }
            Ok(Shape::Body(made))
        }
    }
}

impl ScaleNu {
    fn ask_factor(&self, doc: &Document) -> Step {
        let key = PROMPTS[self.factors.len()];
        let copy = if copying(doc) { "Yes" } else { "No" };
        Step::Ask(Request {
            prompt: Message::with(key, vec![copy.to_owned()]),
            allow_empty: true,
            options: vec![COPY_OPTION],
            ..Request::new(key, Kind::Number)
        })
    }

    fn finish(&self, doc: &mut Document) -> Step {
        let (Some(ids), Some(origin)) = (&self.ids, self.origin) else {
            return unexpected();
        };
        let axes = axes(doc);
        let mut made = Vec::new();
        for object in doc.state().objects.iter().filter(|o| ids.contains(&o.id)) {
            match stretched_shape(&object.shape, origin, &axes, &self.factors) {
                Ok(shape) => made.push((object.id, shape)),
                Err(error) => return geometry_failed(&error),
            }
        }
        let copy = copying(doc);
        for (id, shape) in made {
            if copy {
                let layer = doc.layer_of(id);
                doc.add_shape_on(shape, layer);
            } else {
                doc.replace_shape(id, shape);
            }
        }
        Step::Done(Some(Message::with(
            "done.scale",
            vec![ids.len().to_string()],
        )))
    }
}

impl Command for ScaleNu {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.ids.is_none() {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.scale.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.scale.origin", Vec::new());
        }
        if self.origin.is_none() {
            let Some(Answer::Point(origin)) = answer else {
                return unexpected();
            };
            self.origin = Some(origin);
            return self.ask_factor(doc);
        }
        match answer {
            Some(Answer::Option(_)) => {
                let flipped = !copying(doc);
                doc.remember(COPY, f64::from(u8::from(flipped)));
                return self.ask_factor(doc);
            }
            Some(Answer::Empty) => self.factors.push(1.0),
            Some(Answer::Number(factor)) if factor.is_finite() && factor >= SMALLEST_FACTOR => {
                self.factors.push(factor);
            }
            Some(Answer::Number(_)) => return Step::Failed(Message::new("error.bad_factor")),
            _ => return unexpected(),
        }
        if self.factors.len() < 3 {
            self.ask_factor(doc)
        } else {
            self.finish(doc)
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(origin)) = (&self.ids, self.origin) else {
            return Vec::new();
        };
        let mut factors = self.factors.clone();
        if let Some(typed) = live
            .number
            .filter(|f| f.is_finite() && *f >= SMALLEST_FACTOR)
        {
            factors.push(typed);
        }
        let axes = axes(doc);
        doc.state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .take(super::MOST_PREVIEW_OBJECTS)
            .flat_map(|o| o.shape.outlines())
            .filter_map(|curve| stretched_curve(&curve, origin, &axes, &factors).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn a_cube_becomes_a_box_of_the_three_factors() {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select(&[id]);
        let before = doc.undo_mark();
        run(&mut doc, &["scalenu", "0,0,0", "2", "1", "3"]);
        let body = doc.state().objects[0].shape.body().unwrap().clone();
        assert_eq!(doc.state().objects[0].id, id);
        assert!((body.volume().unwrap() - 6.0).abs() < 1e-6);
        let corners: Vec<[f64; 3]> = doc.state().objects[0]
            .shape
            .outlines()
            .iter()
            .flat_map(|edge| edge.sample().unwrap())
            .collect();
        let size: Vec<f64> = (0..3)
            .map(|i| {
                let values = corners.iter().map(|p| p[i]);
                values.clone().fold(f64::MIN, f64::max) - values.fold(f64::MAX, f64::min)
            })
            .collect();
        assert!(
            (size[0] - 2.0).abs() < 1e-6
                && (size[1] - 1.0).abs() < 1e-6
                && (size[2] - 3.0).abs() < 1e-6,
            "{size:?}"
        );
        assert_eq!(doc.undo_mark(), before + 1);
    }

    #[test]
    fn enter_keeps_a_factor_at_one_and_copy_keeps_the_original() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 1.0, 0.0], [2.0, 1.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scalenu", "0,0", "copy", "3", "", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        let points = doc.state().objects[1].curve().unwrap().sample().unwrap();
        assert_eq!(points, [[3.0, 1.0, 0.0], [6.0, 1.0, 0.0]]);
    }
}
