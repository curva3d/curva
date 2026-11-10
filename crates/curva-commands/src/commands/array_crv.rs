//! `ArrayCrv`: copies objects at equal distances along a curve, keeping their direction.

use super::{ask_objects, curve_at, geometry_failed, ghosts, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, gap, sub};

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
}

fn valid_items(typed: f64) -> Option<usize> {
    (typed >= 2.0 && typed.fract() == 0.0 && typed <= MOST_ITEMS as f64).then_some(typed as usize)
}

/// The moves from the first place on the path to each later one.
fn moves(path: &Curve, items: usize) -> Result<Vec<Point>, KernelError> {
    let closed = {
        let samples = path.sample()?;
        gap(samples[0], samples[samples.len() - 1]) < ON_CURVE
    };
    let places = path.divided(if closed { items } else { items - 1 })?;
    let first = places[0];
    Ok(places.into_iter().skip(1).map(|p| sub(p, first)).collect())
}

impl Command for ArrayCrv {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.arraycrv.objects");
            }
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
                Step::Ask(Request::new("prompt.arraypolar.items", Kind::Number))
            }
            (Some(path), Some(Answer::Number(typed))) => {
                let Some(items) = valid_items(typed) else {
                    return Step::Failed(Message::new("error.bad_items"));
                };
                match moves(path, items) {
                    Ok(list) => {
                        let mut made = 0;
                        for by in list {
                            made += doc.copy(&ids, by).len();
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
        moves(path, items)
            .unwrap_or_default()
            .into_iter()
            .take(MOST_PREVIEWED)
            .flat_map(|by| ghosts(doc, ids, |curve| curve.translated(by)))
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
