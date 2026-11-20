//! `Trim`: cuts curves with chosen cutters and removes the part clicked, as in Rhino. Each click
//! trims at once, so later clicks see the curves as they now are; Enter ends it.

use crate::Message;
use crate::commands::{curve_at, curves_of, cut_by};
use crate::recipe::{Context, Derived, Made, ON_CURVE, Recipe, StepDef, Value};
use curva_document::{Document, Shape};
use curva_kernel::{Curve, Point, gap};
use std::cell::RefCell;

/// The command. It keeps its cutters as curves, so a cutter that is itself trimmed goes on
/// cutting with whatever is left of it.
#[derive(Default)]
pub struct Trim {
    cutters: RefCell<Option<Vec<Curve>>>,
}

/// What clicking a point would trim.
struct Cut {
    id: u64,
    curve: Curve,
    pieces: Vec<Curve>,
    doomed: usize,
}

impl Trim {
    /// The cutters: the curves chosen at the first step, as they now are.
    fn cutters(&self, cx: &Context<'_>) -> Vec<Curve> {
        let mut kept = self.cutters.borrow_mut();
        kept.get_or_insert_with(|| match cx.values.first() {
            Some(Value::Objects(ids)) => {
                curves_of(cx.doc, ids).into_iter().map(|(_, c)| c).collect()
            }
            _ => Vec::new(),
        })
        .clone()
    }

    /// The curve under `at` cut by every cutter but itself, if it is crossed.
    fn cut_under(
        &self,
        cx: &Context<'_>,
        doc: &Document,
        at: Point,
    ) -> Result<Option<Cut>, Message> {
        let failed =
            |e: &curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
        let Some((id, curve)) = curve_at(doc, at, ON_CURVE).map_err(|e| failed(&e))? else {
            return Ok(None);
        };
        let others: Vec<Curve> = self
            .cutters(cx)
            .into_iter()
            .filter(|c| *c != curve)
            .collect();
        let pieces = cut_by(&curve, &others).map_err(|e| failed(&e))?;
        if pieces.len() < 2 {
            return Ok(None);
        }
        let mut doomed = 0;
        let mut nearest = f64::INFINITY;
        for (index, piece) in pieces.iter().enumerate() {
            let distance = gap(piece.closest_point(at).map_err(|e| failed(&e))?, at);
            if distance < nearest {
                nearest = distance;
                doomed = index;
            }
        }
        Ok(Some(Cut {
            id,
            curve,
            pieces,
            doomed,
        }))
    }
}

impl Recipe for Trim {
    const NAME: &'static str = "Trim";
    const APPLIES_EACH_ANSWER: bool = true;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.trim.cutters").curves()];
        steps.extend(
            cx.values
                .iter()
                .skip(1)
                .map(|_| StepDef::pick("prompt.trim.part").optional()),
        );
        if !cx.values.is_empty() && cx.values.last() != Some(&Value::Empty) {
            steps.push(StepDef::pick("prompt.trim.part").optional());
        }
        steps
    }

    /// A click trims only where it lands on a crossed curve; anything else asks again.
    fn accepts(&self, cx: &Context<'_>) -> bool {
        match cx.values.last() {
            Some(Value::Point(at)) if cx.values.len() > 1 => {
                matches!(self.cut_under(cx, cx.doc, *at), Ok(Some(_)))
            }
            Some(Value::Objects(_) | Value::Empty) => true,
            _ => false,
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        match cx.values.last() {
            Some(Value::Empty) => {
                let trimmed = cx.values.iter().filter(|v| v.point().is_some()).count();
                Ok(Made {
                    message: Some(Message::with("done.trim", vec![trimmed.to_string()])),
                    ..Made::default()
                })
            }
            // The answers already given are in the model; the preview shows only the part the
            // pointer would remove.
            Some(Value::Point(at)) if cx.values.len() > 1 => {
                if cx.previewing && cx.answered == cx.values.len() {
                    return Ok(Made::default());
                }
                let Some(mut cut) = self.cut_under(cx, cx.doc, *at)? else {
                    return Ok(Made::default());
                };
                if cx.previewing {
                    return Ok(Made {
                        guides: vec![cut.pieces.swap_remove(cut.doomed)],
                        ..Made::default()
                    });
                }
                let kept: Vec<Curve> = cut
                    .pieces
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| *index != cut.doomed)
                    .map(|(_, piece)| piece)
                    .collect();
                if let Some(cutters) = self.cutters.borrow_mut().as_mut()
                    && let Some(at) = cutters.iter().position(|c| *c == cut.curve)
                {
                    cutters.splice(at..=at, kept.iter().cloned());
                }
                Ok(Made {
                    remove: vec![cut.id],
                    derived: kept
                        .into_iter()
                        .map(|piece| Derived {
                            shape: Shape::Curve(piece),
                            from: cut.id,
                            history: None,
                        })
                        .collect(),
                    ..Made::default()
                })
            }
            _ => Ok(Made::default()),
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
            .map(|object| object.curve().unwrap().length().unwrap())
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
    fn escape_keeps_what_was_trimmed_as_one_undo_step() {
        let (mut doc, mut processor, _) = start();
        processor.click(&mut doc, [8.0, 0.0, 0.0]);
        processor.cancel(&mut doc);
        assert!((length_of_all(&doc) - 13.0).abs() < 1e-9);
        processor.submit(&mut doc, "undo");
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn typing_at_the_part_prompt_changes_nothing() {
        let (mut doc, mut processor, _) = start();
        processor.submit(&mut doc, "1,1");
        assert!((length_of_all(&doc) - 20.0).abs() < 1e-9);
        assert!(processor.is_running());
    }
}
