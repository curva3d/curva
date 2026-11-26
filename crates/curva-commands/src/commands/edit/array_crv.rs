//! `ArrayCrv`, as in Rhino: copies objects at equal distances along a curve, each turned with the
//! curve from its start (Rhino's Freeform, the default, #738); `NoRotation` only moves them.

use crate::commands::{
    ask_objects, curve_at, geometry_failed, ghosts, objects_or_selection, unexpected,
};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, Similarity, Spline, cross, dot, gap, sub, unit};

/// Most items along a curve.
const MOST_ITEMS: usize = 10_000;
/// Most items previewed, so that a large array does not stall the window.
const MOST_PREVIEWED: usize = 200;
/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;

/// The running command.
#[derive(Default)]
pub struct ArrayCrv {
    ids: Option<Vec<u64>>,
    path: Option<Curve>,
    /// Only moved, not turned with the curve (Rhino's NoRotation).
    no_rotation: bool,
}

const NO_ROTATION: &str = "NoRotation";

fn valid_items(typed: f64) -> Option<usize> {
    (typed >= 2.0 && typed.fract() == 0.0 && typed <= MOST_ITEMS as f64).then_some(typed as usize)
}

/// For each later place on the path: the move from the first place to it, and the turn about the
/// first place that takes the path's direction there to its direction here (none when only
/// moving, or where they are the same).
fn moves(
    path: &Curve,
    items: usize,
    turned: bool,
) -> Result<Vec<(Point, Option<Similarity>)>, KernelError> {
    let closed = {
        let samples = path.sample()?;
        gap(samples[0], samples[samples.len() - 1]) < ON_CURVE
    };
    let places = path.divided(if closed { items } else { items - 1 })?;
    let first = places[0];
    let spline = Spline::of(path)?;
    let direction = |p: Point| unit(spline.derivative(spline.closest(p)));
    let start = direction(first);
    Ok(places
        .into_iter()
        .skip(1)
        .map(|p| {
            let turn = match (turned, start, direction(p)) {
                (true, Some(a), Some(b)) => {
                    let axis = cross(a, b);
                    let angle = dot(axis, axis).sqrt().atan2(dot(a, b));
                    unit(axis).map(|axis| Similarity::rotation(first, axis, angle))
                }
                _ => None,
            };
            (sub(p, first), turn)
        })
        .collect())
}

impl ArrayCrv {
    /// How many items, with `NoRotation` offered.
    fn ask_items(&self) -> Step {
        Step::Ask(Request {
            options: vec![NO_ROTATION],
            ..Request::new("prompt.arraypolar.items", Kind::Number)
        })
    }
}

impl Command for ArrayCrv {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let Some(chosen) = objects_or_selection(doc, answer) else {
                return ask_objects("prompt.arraycrv.objects");
            };
            self.ids = Some(chosen);
            return Step::Ask(Request::new("prompt.arraycrv.path", Kind::PickPart));
        };
        match (&self.path, answer) {
            (None, Some(Answer::Point(at))) => {
                let on = curve_at(doc, at, ON_CURVE).ok().flatten();
                let Some((_, path)) = on else {
                    return Step::Ask(Request::new("prompt.arraycrv.path", Kind::PickPart));
                };
                self.path = Some(path);
                self.ask_items()
            }
            (Some(_), Some(Answer::Option(_))) => {
                self.no_rotation = !self.no_rotation;
                self.ask_items()
            }
            (Some(path), Some(Answer::Number(typed))) => {
                let Some(items) = valid_items(typed) else {
                    return Step::Failed(Message::new("error.bad_items"));
                };
                match moves(path, items, !self.no_rotation) {
                    Ok(list) => {
                        let mut made = 0;
                        for (by, turn) in list {
                            let copies = match turn {
                                Some(turn) => {
                                    let turned = doc.copy_transformed(&ids, &turn);
                                    doc.translate(&turned, by);
                                    turned
                                }
                                None => doc.copy(&ids, by),
                            };
                            made += copies.len();
                        }
                        Step::Done(Some(Message::with("done.array", vec![made.to_string()])))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(path), Some(items)) =
            (&self.ids, &self.path, live.number.and_then(valid_items))
        else {
            return Vec::new();
        };
        moves(path, items, !self.no_rotation)
            .unwrap_or_default()
            .into_iter()
            .take(MOST_PREVIEWED)
            .flat_map(|(by, turn)| {
                ghosts(doc, ids, |curve| match &turn {
                    Some(turn) => curve.mapped(turn).translated(by),
                    None => curve.translated(by),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn start() -> (Document, Processor) {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0, 5.0, 0.0], [1.0, 5.0, 0.0]).unwrap());
        doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "arraycrv");
        processor.click(&mut doc, [3.0, 0.0, 0.0]);
        (doc, processor)
    }

    fn starts(doc: &Document) -> Vec<Point> {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve().unwrap().sample().unwrap()[0])
            .collect()
    }

    #[test]
    fn copies_spread_evenly_to_the_end_of_the_path() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "3");
        let found = starts(&doc);
        assert_eq!(found.len(), 4);
        assert!(found.contains(&[5.0, 5.0, 0.0]));
        assert!(found.contains(&[10.0, 5.0, 0.0]));
    }

    #[test]
    fn a_bad_count_makes_nothing() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "1");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn the_preview_follows_a_typed_count() {
        let (doc, processor) = start();
        assert_eq!(processor.preview(&doc, None, "4").len(), 3);
    }
}

#[cfg(test)]
mod freeform {
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn copies_turn_with_the_curve_as_in_rhino() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.add(
            Curve::arc(
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .unwrap(),
        );
        doc.select(&[cube]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "arraycrv");
        processor.click(&mut doc, [10.0, 0.0, 0.0]);
        processor.submit(&mut doc, "5");
        // Rhino 8: the copy at the arc's end is turned a quarter turn, at (-1, 0, 0)-(0, 1, 1).
        let (low, high) = doc
            .state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4);
        assert!(
            near(low, [-1.0, 0.0, 0.0]) && near(high, [0.0, 1.0, 1.0]),
            "{low:?} {high:?}"
        );
    }
}
