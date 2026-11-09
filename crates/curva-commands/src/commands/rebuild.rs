//! `Rebuild`: remakes smooth curves through a chosen number of evenly spaced points.

use super::{ask_objects, curves_of, geometry_failed, ghost_results, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError};

/// Most points a rebuilt curve can have.
const MOST_POINTS: usize = 1_000;

/// The running command.
#[derive(Default)]
pub struct Rebuild {
    ids: Option<Vec<u64>>,
}

fn valid_points(typed: f64) -> Option<usize> {
    (typed >= 2.0 && typed.fract() == 0.0 && typed <= MOST_POINTS as f64).then_some(typed as usize)
}

fn rebuilt(curve: &Curve, points: usize) -> Result<Option<Curve>, KernelError> {
    if !matches!(curve, Curve::Interpolated { .. } | Curve::Nurbs { .. }) {
        return Ok(None);
    }
    let path = curve.sample()?;
    let closed = curva_kernel::gap(path[0], path[path.len() - 1]) < 1e-6;
    let through = curve.divided(if closed { points } else { points - 1 })?;
    Curve::interpolated(through, closed).map(Some)
}

fn all_rebuilt(
    doc: &Document,
    ids: &[u64],
    points: usize,
) -> Result<Vec<(u64, Curve)>, KernelError> {
    let mut made = Vec::new();
    for (id, curve) in curves_of(doc, ids) {
        if let Some(new) = rebuilt(&curve, points)? {
            made.push((id, new));
        }
    }
    Ok(made)
}

impl Command for Rebuild {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.rebuild.objects");
            }
            self.ids = Some(chosen);
            return Step::Ask(Request::new("prompt.rebuild.points", Kind::Number));
        };
        let Some(Answer::Number(typed)) = answer else {
            return unexpected();
        };
        let Some(points) = valid_points(typed) else {
            return Step::Failed(Message::new("error.bad_points"));
        };
        match all_rebuilt(doc, &ids, points) {
            Ok(made) if made.is_empty() => Step::Failed(Message::new("error.nothing_to_rebuild")),
            Ok(made) => {
                let count = made.len();
                for (id, curve) in made {
                    doc.replace(id, curve);
                }
                Step::Done(Some(Message::with("done.rebuild", vec![count.to_string()])))
            }
            Err(error) => geometry_failed(&error),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(points)) = (&self.ids, live.number.and_then(valid_points)) else {
            return Vec::new();
        };
        all_rebuilt(doc, ids, points).map_or_else(|_| Vec::new(), ghost_results)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected_smooth() -> Document {
        let path = vec![
            [0.0; 3],
            [10.0, 5.0, 0.0],
            [20.0, -3.0, 0.0],
            [30.0, 0.0, 0.0],
        ];
        let mut doc = Document::default();
        let id = doc.add(Curve::interpolated(path, false).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn a_smooth_curve_gets_the_number_of_points_asked_for_and_keeps_its_ends() {
        let mut doc = selected_smooth();
        run(&mut doc, &["rebuild", "12"]);
        let Curve::Interpolated { points, .. } = &doc.state().objects[0].curve else {
            panic!("expected a smooth curve");
        };
        assert_eq!(points.len(), 12);
        assert!(curva_kernel::gap(points[0], [0.0; 3]) < 1e-6);
        assert!(curva_kernel::gap(points[11], [30.0, 0.0, 0.0]) < 1e-6);
    }

    #[test]
    fn a_line_is_left_alone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let processor = run(&mut doc, &["rebuild", "5"]);
        assert!(!processor.is_running());
        assert!(matches!(doc.state().objects[0].curve, Curve::Line { .. }));
    }

    #[test]
    fn a_bad_count_changes_nothing() {
        let mut doc = selected_smooth();
        let before = doc.state().clone();
        run(&mut doc, &["rebuild", "1"]);
        assert_eq!(doc.state(), &before);
    }

    #[test]
    fn the_preview_follows_a_typed_count() {
        let mut doc = selected_smooth();
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "rebuild");
        assert_eq!(processor.preview(&doc, None, "8").len(), 1);
    }
}
