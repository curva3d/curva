//! `Offset`: a copy of curves at a distance, on the side clicked.

use super::{ask_objects, curves_of, geometry_failed, plane_normal, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// The distance used until another is typed, in millimetres.
const STARTING_DISTANCE: f64 = 1.0;
/// Where the document remembers the last distance.
const MEMORY: &str = "offset.distance";

/// The running command.
#[derive(Default)]
pub struct Offset {
    ids: Option<Vec<u64>>,
}

fn distance_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(STARTING_DISTANCE)
}

fn ask_side(doc: &Document) -> Step {
    Step::Ask(Request {
        prompt: Message::with("prompt.offset.side", vec![distance_of(doc).to_string()]),
        ..Request::new("prompt.offset.side", Kind::PointOrNumber)
    })
}

fn offsets(
    doc: &Document,
    ids: &[u64],
    distance: f64,
    toward: Point,
) -> Result<Vec<Curve>, curva_kernel::KernelError> {
    let normal = plane_normal(doc);
    curves_of(doc, ids)
        .iter()
        .map(|(_, curve)| curve.offset(normal, distance, toward))
        .collect()
}

impl Command for Offset {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.offset.objects");
            }
            self.ids = Some(chosen);
            return ask_side(doc);
        };
        match answer {
            Some(Answer::Number(distance)) if distance > 0.0 => {
                doc.remember(MEMORY, distance);
                ask_side(doc)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            Some(Answer::Point(toward)) => match offsets(doc, &ids, distance_of(doc), toward) {
                Ok(made) => {
                    let count = made.len();
                    for curve in made {
                        doc.add(curve);
                    }
                    Step::Done(Some(Message::with("done.offset", vec![count.to_string()])))
                }
                Err(error) => geometry_failed(&error),
            },
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(cursor)) = (&self.ids, live.cursor) else {
            return Vec::new();
        };
        let distance = live
            .number
            .filter(|n| *n > 0.0)
            .unwrap_or_else(|| distance_of(doc));
        offsets(doc, ids, distance, cursor).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn start() -> (Document, Processor) {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offset");
        (doc, processor)
    }

    fn last_start(doc: &Document) -> [f64; 3] {
        doc.state().objects.last().unwrap().curve.sample().unwrap()[0]
    }

    #[test]
    fn a_click_offsets_to_that_side_at_the_current_distance() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [5.0, -9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2);
        assert_eq!(last_start(&doc), [0.0, -1.0, 0.0]);
    }

    #[test]
    fn a_typed_distance_is_used_and_the_prompt_asks_again() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "4");
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(last_start(&doc), [0.0, 4.0, 0.0]);
    }

    #[test]
    fn the_distance_is_remembered_for_the_next_offset() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "4");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        processor.submit(&mut doc, "offset");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
        assert_eq!(last_start(&doc), [0.0, 4.0, 0.0]);
    }

    #[test]
    fn a_distance_too_large_makes_nothing() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offset");
        processor.submit(&mut doc, "5");
        processor.click(&mut doc, [0.5, 0.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }

    #[test]
    fn the_preview_follows_the_side_and_a_typed_distance() {
        let (doc, processor) = start();
        let shown = processor.preview(&doc, Some([5.0, 9.0, 0.0]), "3");
        assert_eq!(shown[0].sample().unwrap()[0], [0.0, 3.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
