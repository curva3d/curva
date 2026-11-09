//! `Trim`: cuts curves with chosen cutters and removes the part clicked, as in Rhino.

use super::{ask_objects, curves_of, cut_by, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, gap};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;

/// The running command.
#[derive(Default)]
pub struct Trim {
    cutters: Option<Vec<u64>>,
    trimmed: usize,
}

/// What clicking a point would trim.
struct Cut {
    id: u64,
    pieces: Vec<Curve>,
    doomed: usize,
}

fn ask_part(key: &'static str) -> Step {
    Step::Ask(Request {
        allow_empty: true,
        ..Request::new(key, Kind::PickPart)
    })
}

impl Trim {
    /// The curve under a picked point cut by every cutter but itself, or `None` if no curve is there.
    fn cut_under(&self, doc: &Document, at: Point) -> Result<Option<Cut>, KernelError> {
        let cutter_ids = self.cutters.as_deref().unwrap_or_default();
        let mut under = None;
        for object in &doc.state().objects {
            if gap(object.curve.closest_point(at)?, at) <= ON_CURVE {
                under = Some((object.id, object.curve.clone()));
                break;
            }
        }
        let Some((id, curve)) = under else {
            return Ok(None);
        };
        let others: Vec<u64> = cutter_ids.iter().copied().filter(|c| *c != id).collect();
        let cutters: Vec<Curve> = curves_of(doc, &others)
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let pieces = cut_by(&curve, &cutters)?;
        let mut doomed = 0;
        let mut nearest = f64::INFINITY;
        for (index, piece) in pieces.iter().enumerate() {
            let distance = gap(piece.closest_point(at)?, at);
            if distance < nearest {
                nearest = distance;
                doomed = index;
            }
        }
        Ok(Some(Cut { id, pieces, doomed }))
    }
}

impl Command for Trim {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.cutters.is_none() {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.trim.cutters");
            }
            self.cutters = Some(chosen);
            doc.select(&[]);
            return ask_part("prompt.trim.part");
        }
        match answer {
            Some(Answer::Empty) => Step::Done(Some(Message::with(
                "done.trim",
                vec![self.trimmed.to_string()],
            ))),
            Some(Answer::Point(at)) => {
                let cut = match self.cut_under(doc, at) {
                    Ok(cut) => cut,
                    Err(error) => return geometry_failed(&error),
                };
                let Some(cut) = cut.filter(|cut| cut.pieces.len() >= 2) else {
                    return ask_part("prompt.trim.not_crossed");
                };
                let was_cutter = self.cutters.as_ref().is_some_and(|c| c.contains(&cut.id));
                doc.remove(&[cut.id]);
                let kept: Vec<u64> = cut
                    .pieces
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| *index != cut.doomed)
                    .map(|(_, piece)| doc.add(piece))
                    .collect();
                // A trimmed cutter goes on cutting with whatever is left of it.
                if was_cutter && let Some(cutters) = self.cutters.as_mut() {
                    cutters.retain(|c| *c != cut.id);
                    cutters.extend(kept);
                }
                self.trimmed += 1;
                ask_part("prompt.trim.part")
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(_), Some(at)) = (&self.cutters, live.cursor) else {
            return Vec::new();
        };
        match self.cut_under(doc, at) {
            Ok(Some(mut cut)) if cut.pieces.len() >= 2 => vec![cut.pieces.swap_remove(cut.doomed)],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    /// A horizontal line from 0 to 10 and a vertical cutter across it at 3.
    fn start() -> (Document, Processor, u64) {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let cutter = doc.add(Curve::line([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let mut processor = Processor::default();
        doc.select(&[cutter]);
        processor.submit(&mut doc, "trim");
        (doc, processor, cutter)
    }

    fn length_of_all(doc: &Document) -> f64 {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve.length().unwrap())
            .sum()
    }

    #[test]
    fn the_part_clicked_is_removed() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [8.0, 0.0, 0.0]);
        assert!((length_of_all(&doc) - 13.0).abs() < 1e-9);
    }

    #[test]
    fn the_other_part_can_be_clicked_instead() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        assert!((length_of_all(&doc) - 17.0).abs() < 1e-9);
    }

    #[test]
    fn it_keeps_asking_until_enter() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [8.0, 0.0, 0.0]);
        assert!(processor.is_running());
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
    }

    #[test]
    fn a_whole_run_is_one_undo_step() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [8.0, 0.0, 0.0]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "undo");
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn a_click_on_nothing_changes_nothing_and_keeps_asking() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [50.0, 50.0, 0.0]);
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
        assert!(processor.is_running());
    }

    #[test]
    fn the_only_cutter_cannot_trim_itself() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [3.0, 4.0, 0.0]);
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn cutters_can_trim_each_other_and_keep_cutting() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let mut processor = Processor::default();
        doc.select(&[a, b]);
        processor.submit(&mut doc, "trim");
        processor.click(&mut doc, [8.0, 0.0, 0.0]);
        processor.click(&mut doc, [3.0, 4.0, 0.0]);
        assert!((length_of_all(&doc) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn the_preview_shows_the_part_that_would_go() {
        let (doc, processor, _) = start();
        let shown = processor.preview(&doc, Some([8.0, 0.0, 0.0]), "");
        assert_eq!(shown.len(), 1);
        assert!((shown[0].length().unwrap() - 7.0).abs() < 1e-9);
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn typing_at_the_part_prompt_changes_nothing() {
        let (mut doc, mut processor, _) = start();
        processor.submit(&mut doc, "1,1");
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
        assert!(processor.is_running());
    }
}
