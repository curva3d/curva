//! `FilletCorners`: rounds every corner of polylines with one radius.

use super::{ask_objects, curves_of, geometry_failed, ghost_results, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, KernelError};

/// Where the document remembers the last radius.
const MEMORY: &str = "filletcorners.radius";
/// The radius offered the first time.
const STARTING_RADIUS: f64 = 1.0;

fn radius_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(STARTING_RADIUS)
}

fn rounded(doc: &Document, ids: &[u64], radius: f64) -> Result<Vec<(u64, Curve)>, KernelError> {
    curves_of(doc, ids)
        .into_iter()
        .map(|(id, curve)| Ok((id, curve.fillet_corners(radius)?)))
        .collect()
}

/// The running command.
#[derive(Default)]
pub struct FilletCorners {
    ids: Option<Vec<u64>>,
}

impl Command for FilletCorners {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.filletcorners.objects");
            }
            self.ids = Some(chosen);
            return Step::Ask(Request {
                prompt: Message::with(
                    "prompt.filletcorners.radius",
                    vec![radius_of(doc).to_string()],
                ),
                allow_empty: true,
                ..Request::new("prompt.filletcorners.radius", Kind::Number)
            });
        };
        let radius = match answer {
            Some(Answer::Number(radius)) => radius,
            Some(Answer::Empty) => radius_of(doc),
            _ => return unexpected(),
        };
        match rounded(doc, &ids, radius) {
            Ok(made) => {
                doc.remember(MEMORY, radius);
                for (id, curve) in made {
                    doc.replace(id, curve);
                }
                Step::Done(Some(Message::with(
                    "done.filletcorners",
                    vec![ids.len().to_string()],
                )))
            }
            Err(error) => geometry_failed(&error),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let radius = live.number.unwrap_or_else(|| radius_of(doc));
        rounded(doc, ids, radius)
            .map(ghost_results)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected_square() -> Document {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::polyline(
                vec![
                    [0.0, 0.0, 0.0],
                    [10.0, 0.0, 0.0],
                    [10.0, 10.0, 0.0],
                    [0.0, 10.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        doc
    }

    #[test]
    fn every_corner_is_rounded_and_the_radius_remembered() {
        let mut doc = selected_square();
        run(&mut doc, &["filletcorners", "2"]);
        assert!(matches!(
            doc.state().objects[0].curve,
            Curve::Compound { .. }
        ));
        assert_eq!(doc.remembered("filletcorners.radius"), Some(2.0));
    }

    #[test]
    fn enter_uses_the_last_radius_and_a_bad_one_changes_nothing() {
        let mut doc = selected_square();
        let before = doc.state().objects[0].curve.clone();
        run(&mut doc, &["filletcorners", "50"]);
        assert_eq!(doc.state().objects[0].curve, before);
        run(&mut doc, &["filletcorners", ""]);
        assert!(matches!(
            doc.state().objects[0].curve,
            Curve::Compound { .. }
        ));
    }

    #[test]
    fn the_preview_follows_the_typed_radius() {
        let mut doc = selected_square();
        let processor = run(&mut doc, &["filletcorners"]);
        let shown = processor.preview(&doc, None, "3");
        assert!(!shown.is_empty());
    }
}
