//! `Offset`, as in Rhino: a copy of curves at a distance, on the side clicked. A number typed at
//! the side prompt sets `Distance`; `ThroughPoint` makes the copy pass through the point clicked;
//! `BothSides` makes one on each side. Each copy follows its curve when that changes (`History`).
//! Rhino's `Loose`, `Corner`, `Trim`, `Tolerance`, `InCPlane`, `Cap` and `OutputLayer` are still to
//! come (`docs/commands/rhino-reference.md`).

use super::curves_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{cross, gap};

const DISTANCE: &str = "Distance";
const THROUGH_POINT: &str = "ThroughPoint";
const BOTH_SIDES: &str = "BothSides";

/// The recipe.
#[derive(Default)]
pub struct Offset;

impl Recipe for Offset {
    const NAME: &'static str = "offset";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::distance(DISTANCE, 1.0, false),
        OptionDef::mode(THROUGH_POINT),
        OptionDef::toggle(BOTH_SIDES, false),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.offset.objects").offering(&[]),
            StepDef::point("prompt.offset.side").number_sets(DISTANCE),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(toward)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::point))
        else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let normal = cross(x, y);
        let failed =
            |e: curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
        let mut derived = Vec::new();
        for (source, curve) in curves_of(cx.doc, ids) {
            let distance = if cx.mode == Some(THROUGH_POINT) {
                gap(curve.closest_point(toward).map_err(failed)?, toward)
            } else {
                cx.number(DISTANCE)
            };
            let side = curve.side_of_point(normal, toward).map_err(failed)?;
            let sides: &[f64] = if cx.toggle(BOTH_SIDES) {
                &[1.0, -1.0]
            } else {
                &[side]
            };
            for side in sides {
                let copy = curve
                    .offset_on_side(normal, distance, *side)
                    .map_err(failed)?;
                derived.push(Derived {
                    shape: Shape::Curve(copy),
                    from: source,
                    history: Some(curva_document::Recipe::Offset {
                        source,
                        normal,
                        distance,
                        side: *side,
                    }),
                });
            }
        }
        let count = derived.len();
        Ok(Made {
            derived,
            message: Some(Message::with("done.offset", vec![count.to_string()])),
            ..Made::default()
        })
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
        doc.state()
            .objects
            .last()
            .unwrap()
            .curve()
            .unwrap()
            .sample()
            .unwrap()[0]
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

    #[test]
    fn through_point_and_both_sides() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "throughpoint");
        processor.click(&mut doc, [5.0, 2.5, 0.0]);
        assert_eq!(last_start(&doc), [0.0, 2.5, 0.0]);
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "bothsides");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn a_distance_of_zero_is_refused() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "0");
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
