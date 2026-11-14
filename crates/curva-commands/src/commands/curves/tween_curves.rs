//! `TweenCurves`: adds curves evenly spaced between two curves.

use crate::commands::{ask_objects, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, tween_curves};

/// Where the document remembers the last number of curves.
const MEMORY: &str = "tweencurves.count";
/// The most curves one run makes, to keep a typing slip from freezing the program.
const MOST: f64 = 1000.0;

fn count_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(1.0)
}

/// A whole number of curves from a typed value, if it is sensible.
fn whole(count: f64) -> Option<usize> {
    (count.fract() == 0.0 && (1.0..=MOST).contains(&count)).then_some(count as usize)
}

fn tweens(doc: &Document, ids: &[u64], count: usize) -> Result<Vec<Curve>, KernelError> {
    let curves = curves_of(doc, ids);
    tween_curves(&curves[0].1, &curves[1].1, count)
}

/// The running command.
#[derive(Default)]
pub struct TweenCurves {
    ids: Option<Vec<u64>>,
}

impl Command for TweenCurves {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.tweencurves.objects");
            }
            if curves_of(doc, &chosen).len() != 2 {
                return Step::Failed(Message::new("error.tween_two"));
            }
            self.ids = Some(chosen);
            return Step::Ask(Request {
                prompt: Message::with("prompt.tweencurves.count", vec![count_of(doc).to_string()]),
                allow_empty: true,
                ..Request::new("prompt.tweencurves.count", Kind::Number)
            });
        };
        let typed = match answer {
            Some(Answer::Number(count)) => count,
            Some(Answer::Empty) => count_of(doc),
            _ => return unexpected(),
        };
        let Some(count) = whole(typed) else {
            return Step::Failed(Message::new("error.bad_tween_count"));
        };
        match tweens(doc, &ids, count) {
            Ok(made) => {
                doc.remember(MEMORY, typed);
                for curve in made {
                    doc.add(curve);
                }
                Step::Done(Some(Message::with(
                    "done.tweencurves",
                    vec![count.to_string()],
                )))
            }
            Err(error) => geometry_failed(&error),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        whole(live.number.unwrap_or_else(|| count_of(doc)))
            .and_then(|count| tweens(doc, ids, count).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn two_lines() -> Document {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 9.0, 0.0], [10.0, 9.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        doc
    }

    #[test]
    fn it_adds_evenly_spaced_curves_and_is_one_undo_step() {
        let mut doc = two_lines();
        let mut processor = run(&mut doc, &["tweencurves", "2"]);
        assert_eq!(doc.state().objects.len(), 4);
        assert_eq!(
            *doc.state().objects[2].curve().unwrap(),
            Curve::line([0.0, 3.0, 0.0], [10.0, 3.0, 0.0]).unwrap()
        );
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn exactly_two_curves_are_needed() {
        let mut doc = two_lines();
        let extra = doc.add(Curve::line([0.0; 3], [0.0, 0.0, 1.0]).unwrap());
        let mut chosen = doc.selection();
        chosen.push(extra);
        doc.select(&chosen);
        run(&mut doc, &["tweencurves", "1"]);
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn a_fraction_is_refused_and_the_preview_follows_the_count() {
        let mut doc = two_lines();
        run(&mut doc, &["tweencurves", "1.5"]);
        assert_eq!(doc.state().objects.len(), 2);
        let processor = run(&mut doc, &["tweencurves"]);
        assert_eq!(processor.preview(&doc, None, "3").len(), 3);
    }
}
