//! `Divide`: places points at equal distances along curves.

use crate::commands::{ask_objects, curves_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError, Point};

/// Most segments a curve can be divided into.
const MOST_SEGMENTS: usize = 10_000;

/// The running command.
#[derive(Default)]
pub struct Divide {
    ids: Option<Vec<u64>>,
}

fn valid_segments(typed: f64) -> Option<usize> {
    (typed >= 1.0 && typed.fract() == 0.0 && typed <= MOST_SEGMENTS as f64)
        .then_some(typed as usize)
}

fn points_of(
    doc: &Document,
    ids: &[u64],
    segments: usize,
) -> Result<Vec<(u64, Vec<Point>)>, KernelError> {
    curves_of(doc, ids)
        .into_iter()
        .map(|(id, curve)| Ok((id, curve.divided(segments)?)))
        .collect()
}

impl Command for Divide {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.divide.objects");
            }
            self.ids = Some(chosen);
            return Step::Ask(Request::new("prompt.divide.segments", Kind::Number));
        };
        let Some(Answer::Number(typed)) = answer else {
            return unexpected();
        };
        let Some(segments) = valid_segments(typed) else {
            return Step::Failed(Message::new("error.bad_segments"));
        };
        match points_of(doc, &ids, segments) {
            Ok(found) => {
                let mut made = 0;
                for (id, points) in found {
                    let layer = doc.layer_of(id);
                    for at in points {
                        doc.add_on(Curve::Point { at }, layer);
                        made += 1;
                    }
                }
                Step::Done(Some(Message::with("done.divide", vec![made.to_string()])))
            }
            Err(error) => geometry_failed(&error),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(segments)) = (&self.ids, live.number.and_then(valid_segments)) else {
            return Vec::new();
        };
        points_of(doc, ids, segments)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(_, points)| points)
            .map(|at| Curve::Point { at })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected(curve: Curve) -> Document {
        let mut doc = Document::default();
        let id = doc.add(curve);
        doc.select(&[id]);
        doc
    }

    #[test]
    fn a_line_gets_a_point_at_each_end_and_division() {
        let mut doc = selected(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        run(&mut doc, &["divide", "5"]);
        assert_eq!(doc.state().objects.len(), 7);
        assert_eq!(
            *doc.state().objects[3].curve().unwrap(),
            Curve::Point {
                at: [4.0, 0.0, 0.0]
            }
        );
    }

    #[test]
    fn a_closed_curve_does_not_repeat_its_start() {
        let mut doc = selected(Curve::circle([0.0; 3], 5.0).unwrap());
        run(&mut doc, &["divide", "4"]);
        assert_eq!(doc.state().objects.len(), 5);
    }

    #[test]
    fn a_bad_count_makes_nothing() {
        let mut doc = selected(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        run(&mut doc, &["divide", "0"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn the_preview_follows_a_typed_count() {
        let mut doc = selected(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "divide");
        assert_eq!(processor.preview(&doc, None, "2").len(), 3);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
