//! `SetHatchBasePoint`: moves where hatches' patterns start, as Rhino's command does (spec
//! `annotation.md` §5). The outlines stay; the pattern lines shift to pass through the point.

use crate::commands::ask_objects;
use crate::commands::support::*;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::annotation::Annotation;
use curva_document::{Document, Shape};

/// The running command.
#[derive(Default)]
pub struct SetHatchBasePoint {
    hatches: Vec<u64>,
}

impl Command for SetHatchBasePoint {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Point(at)) if !self.hatches.is_empty() => {
                for id in &self.hatches {
                    let Some(Shape::Annotation(Annotation::Hatch(hatch))) = doc
                        .state()
                        .objects
                        .iter()
                        .find(|o| o.id == *id)
                        .map(|o| &o.shape)
                    else {
                        continue;
                    };
                    let mut moved = hatch.clone();
                    moved.base = at;
                    doc.replace_shape(*id, Shape::Annotation(Annotation::Hatch(moved)));
                }
                Step::Done(Some(Message::with(
                    "done.hatch_base",
                    vec![self.hatches.len().to_string()],
                )))
            }
            other => {
                let state = doc.state();
                self.hatches = chosen(doc, &other)
                    .into_iter()
                    .filter(|id| {
                        state.objects.iter().any(|o| {
                            o.id == *id
                                && matches!(o.shape, Shape::Annotation(Annotation::Hatch(_)))
                        })
                    })
                    .collect();
                if self.hatches.is_empty() {
                    return ask_objects("prompt.hatch_base.hatches");
                }
                Step::Ask(Request::new("prompt.hatch_base.point", Kind::Point))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};

    #[test]
    fn a_hatchs_pattern_starts_at_the_new_base_point() {
        let mut doc = Document::default();
        let square = curva_kernel::Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let line = doc.add(curva_kernel::Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let hatch = doc.add_annotation(Annotation::Hatch(curva_document::annotation::Hatch {
            loops: vec![square],
            pattern: "Hatch1".to_owned(),
            scale: 1.0,
            rotation: 0.0,
            base: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
        }));
        // A curve chosen along with the hatch is left alone.
        doc.select(&[hatch, line]);
        let before = doc.state().objects.len();
        run(&mut doc, &["SetHatchBasePoint", "2,3"]);
        let state = doc.state();
        assert_eq!(
            state.objects.len(),
            before,
            "the same hatch, moved in place"
        );
        let Some(Shape::Annotation(Annotation::Hatch(h))) = state
            .objects
            .iter()
            .find(|o| o.id == hatch)
            .map(|o| &o.shape)
        else {
            panic!("the hatch");
        };
        assert_eq!(h.base, [2.0, 3.0, 0.0]);
    }
}
