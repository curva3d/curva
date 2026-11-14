//! `Copy`, as in Rhino: places copies of objects at each point given, until Enter. `InPlace` adds
//! a copy where the objects are and ends; `Vertical` places copies straight up or down from the
//! plane being drawn on. Rhino's `FromLastPoint`, `UseLastDistance` and `UseLastDirection` are
//! still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::move_objects::{end, end_step};
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_kernel::sub;

const VERTICAL: &str = "Vertical";
const IN_PLACE: &str = "InPlace";

/// The recipe.
#[derive(Default)]
pub struct CopyObjects;

impl Recipe for CopyObjects {
    const NAME: &'static str = "copy";
    const PLACES_AS_IT_GOES: bool = true;
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(VERTICAL, false),
        OptionDef::finish(IN_PLACE),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.copy.objects").offering(&[])];
        if cx.values.is_empty() {
            return steps;
        }
        steps.push(StepDef::point("prompt.copy.from").offering(&[IN_PLACE]));
        let Some(from) = cx.values.get(1).and_then(Value::point) else {
            return steps;
        };
        // One step per copy placed, then one more, which Enter answers to end.
        for _ in 2..cx.values.len() {
            steps.push(end_step(cx, "prompt.copy.to", from).offering(&[VERTICAL]));
        }
        if cx.values.last() != Some(&Value::Empty) {
            steps.push(
                end_step(cx, "prompt.copy.to", from)
                    .offering(&[VERTICAL])
                    .optional(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut changes: Vec<Change> = Vec::new();
        let mut guides = Vec::new();
        if let Some(from) = cx.values.get(1).and_then(Value::point) {
            for value in &cx.values[2..] {
                if let Some(to) = end(cx, from, Some(value)) {
                    changes.push(Change::Move(sub(to, from)));
                    guides.extend(curva_kernel::Curve::line(from, to).ok());
                }
            }
        }
        if cx.mode == Some(IN_PLACE) {
            changes.push(Change::Move([0.0; 3]));
        }
        let mut made = changed(cx, ids, &changes, true, |_, _| None);
        let count = made.derived.len();
        made.guides.extend(guides);
        made.message = Some(Message::with("done.copy", vec![count.to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn one_selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn places_a_copy_at_each_point_and_keeps_the_original() {
        let mut doc = one_selected_line();
        run(&mut doc, &["copy", "0,0", "5,0", "0,5", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 3);
        assert_eq!(
            *objects[0].curve().unwrap(),
            Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap()
        );
        assert_eq!(
            *objects[1].curve().unwrap(),
            Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap()
        );
        assert_eq!(
            *objects[2].curve().unwrap(),
            Curve::line([0.0, 5.0, 0.0], [1.0, 5.0, 0.0]).unwrap()
        );
    }

    #[test]
    fn undo_removes_all_the_copies_at_once() {
        let mut doc = one_selected_line();
        let before = doc.state().clone();
        run(&mut doc, &["cp", "0,0", "5,0", "9,0", "", "undo"]);
        assert_eq!(doc.state(), &before);
    }

    #[test]
    fn in_place_adds_a_copy_where_the_objects_are() {
        let mut doc = one_selected_line();
        run(&mut doc, &["copy", "inplace"]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].shape, objects[1].shape);
    }
}
