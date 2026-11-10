//! `Fillet`: rounds the corner between two lines, keeping the sides clicked.

use super::{curve_at, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point, fillet_curves, gap};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;
/// The radius used until another is typed, in millimetres.
const STARTING_RADIUS: f64 = 1.0;
/// Where the document remembers the last radius.
const MEMORY: &str = "fillet.radius";

/// The running command.
#[derive(Default)]
pub struct Fillet {
    first: Option<(u64, Point)>,
}

fn radius_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(STARTING_RADIUS)
}

fn ask(doc: &Document, key: &'static str) -> Step {
    Step::Ask(Request {
        prompt: Message::with(key, vec![radius_of(doc).to_string()]),
        ..Request::new(key, Kind::PickPart)
    })
}

/// The object under a picked point.
fn under(doc: &Document, at: Point) -> Result<Option<(u64, Curve)>, KernelError> {
    curve_at(doc, at, ON_CURVE)
}

fn curve_of(doc: &Document, id: u64) -> Option<Curve> {
    doc.state()
        .objects
        .iter()
        .find(|object| object.id == id)
        .and_then(|object| object.curve().cloned())
}

impl Fillet {
    fn result(
        &self,
        doc: &Document,
        at: Point,
        radius: f64,
    ) -> Result<Option<(u64, u64, Vec<Curve>)>, KernelError> {
        let Some((first_id, first_pick)) = self.first else {
            return Ok(None);
        };
        let (Some((second_id, second)), Some(first)) = (under(doc, at)?, curve_of(doc, first_id))
        else {
            return Ok(None);
        };
        let same = second_id == first_id;
        if same && gap(first_pick, at) <= ON_CURVE {
            return Ok(None);
        }
        fillet_curves(&first, first_pick, &second, at, radius, same)
            .map(|made| Some((first_id, second_id, made)))
    }
}

impl Command for Fillet {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let again = if self.first.is_some() {
            "prompt.fillet.second"
        } else {
            "prompt.fillet.first"
        };
        match answer {
            None => ask(doc, again),
            Some(Answer::Number(radius)) if radius >= 0.0 => {
                doc.remember(MEMORY, radius);
                ask(doc, again)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_radius")),
            Some(Answer::Point(at)) if self.first.is_none() => match under(doc, at) {
                Ok(Some((id, _))) => {
                    self.first = Some((id, at));
                    ask(doc, "prompt.fillet.second")
                }
                Ok(None) => ask(doc, again),
                Err(error) => geometry_failed(&error),
            },
            Some(Answer::Point(at)) => match self.result(doc, at, radius_of(doc)) {
                Ok(Some((first, second, made))) => {
                    let layer = doc.layer_of(first);
                    doc.remove(&[first, second]);
                    for curve in made {
                        doc.add_on(curve, layer);
                    }
                    Step::Done(Some(Message::new("done.fillet")))
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
        let radius = live
            .number
            .filter(|n| *n >= 0.0)
            .unwrap_or_else(|| radius_of(doc));
        match self.result(doc, at, radius) {
            Ok(Some((_, _, made))) => made,
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
        processor.submit(&mut doc, "fillet");
        (doc, processor)
    }

    fn total(doc: &Document) -> f64 {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve().unwrap().length().unwrap())
            .sum()
    }

    #[test]
    fn two_clicks_round_the_corner_at_the_current_radius() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 3);
        let expected = 18.0 + std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
    }

    #[test]
    fn a_typed_radius_is_used_and_remembered() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "3");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        let expected = 14.0 + 3.0 * std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
        assert_eq!(doc.remembered("fillet.radius"), Some(3.0));
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
    fn two_segments_of_one_polyline_round_into_one_curve() {
        let mut doc = Document::default();
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        doc.add(Curve::polyline(path, false).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "fillet");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
        let expected = 18.0 + std::f64::consts::FRAC_PI_2;
        assert!((total(&doc) - expected).abs() < 1e-6);
    }

    #[test]
    fn a_line_and_an_arc_round_together() {
        let mut doc = Document::default();
        doc.add(Curve::line([30.0, 0.0, 0.0], [10.0, 0.0, 0.0]).unwrap());
        let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        doc.add(Curve::arc([0.0; 3], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], x, y).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "fillet");
        processor.click(&mut doc, [28.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0 * 0.5_f64.cos(), 10.0 * 0.5_f64.sin(), 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
    }
    #[test]
    fn a_click_on_nothing_keeps_asking() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [50.0, 50.0, 0.0]);
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn a_radius_too_large_changes_nothing() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "50");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!((total(&doc) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn the_preview_shows_the_rounded_corner_before_the_second_click() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        let shown = processor.preview(&doc, Some([10.0, 9.0, 0.0]), "");
        assert_eq!(shown.len(), 3);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
