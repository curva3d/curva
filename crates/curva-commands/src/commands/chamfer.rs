//! `Chamfer`: cuts the corner between two lines with a straight line.

use super::{geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Chamfered, Curve, KernelError, Point, chamfer_lines, gap};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;
/// The distance used until another is typed, in millimetres.
const STARTING_DISTANCE: f64 = 1.0;
/// Where the document remembers the last distance.
const MEMORY: &str = "chamfer.distance";

/// The running command.
#[derive(Default)]
pub struct Chamfer {
    first: Option<(u64, Point)>,
}

fn distance_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(STARTING_DISTANCE)
}

fn ask(doc: &Document, key: &'static str) -> Step {
    Step::Ask(Request {
        prompt: Message::with(key, vec![distance_of(doc).to_string()]),
        ..Request::new(key, Kind::PickPart)
    })
}

fn under(doc: &Document, at: Point) -> Result<Option<(u64, Curve)>, KernelError> {
    for object in &doc.state().objects {
        if gap(object.curve.closest_point(at)?, at) <= ON_CURVE {
            return Ok(Some((object.id, object.curve.clone())));
        }
    }
    Ok(None)
}

impl Chamfer {
    fn result(
        &self,
        doc: &Document,
        at: Point,
        distance: f64,
    ) -> Result<Option<(u64, u64, Chamfered)>, KernelError> {
        let Some((first_id, first_pick)) = self.first else {
            return Ok(None);
        };
        let first = doc
            .state()
            .objects
            .iter()
            .find(|object| object.id == first_id)
            .map(|object| object.curve.clone());
        let (Some((second_id, second)), Some(first)) = (under(doc, at)?, first) else {
            return Ok(None);
        };
        if second_id == first_id {
            return Ok(None);
        }
        chamfer_lines(&first, first_pick, &second, at, distance, distance)
            .map(|done| Some((first_id, second_id, done)))
    }
}

impl Command for Chamfer {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let again = if self.first.is_some() {
            "prompt.chamfer.second"
        } else {
            "prompt.chamfer.first"
        };
        match answer {
            None => ask(doc, again),
            Some(Answer::Number(distance)) if distance > 0.0 => {
                doc.remember(MEMORY, distance);
                ask(doc, again)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            Some(Answer::Point(at)) if self.first.is_none() => match under(doc, at) {
                Ok(Some((id, _))) => {
                    self.first = Some((id, at));
                    ask(doc, "prompt.chamfer.second")
                }
                Ok(None) => ask(doc, again),
                Err(error) => geometry_failed(&error),
            },
            Some(Answer::Point(at)) => match self.result(doc, at, distance_of(doc)) {
                Ok(Some((first, second, done))) => {
                    let layer = doc.layer_of(first);
                    doc.remove(&[first, second]);
                    for curve in [done.first, done.second, done.across] {
                        doc.add_on(curve, layer);
                    }
                    Step::Done(Some(Message::new("done.chamfer")))
                }
                Ok(None) => ask(doc, again),
                Err(error) => geometry_failed(&error),
            },
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(at) = live.cursor else {
            return Vec::new();
        };
        let distance = live
            .number
            .filter(|n| *n > 0.0)
            .unwrap_or_else(|| distance_of(doc));
        match self.result(doc, at, distance) {
            Ok(Some((_, _, done))) => vec![done.first, done.second, done.across],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn start() -> (Document, Processor) {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "chamfer");
        (doc, processor)
    }

    fn total(doc: &Document) -> f64 {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve.length().unwrap())
            .sum()
    }

    #[test]
    fn two_clicks_cut_the_corner_by_the_remembered_distance() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "3");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
        let expected = 14.0 + 18.0_f64.sqrt();
        assert!((total(&doc) - expected).abs() < 1e-6);
        assert_eq!(doc.remembered("chamfer.distance"), Some(3.0));
    }

    #[test]
    fn it_is_one_undo_step() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn the_preview_shows_the_cut_before_the_second_click() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        assert_eq!(processor.preview(&doc, Some([10.0, 9.0, 0.0]), "").len(), 3);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
