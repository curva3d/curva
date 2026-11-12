//! `Move`, as in Rhino: moves objects, or picked control points, from one point to another. With
//! `Vertical`, the move goes straight up or down from the plane being drawn on, following the
//! mouse. Rhino's `Normal` and Enter for the bounding box centre are still to come
//! (`docs/commands/rhino-reference.md`).

use super::{Change, changed};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Point, cross, sub};

const VERTICAL: &str = "Vertical";

/// The recipe.
#[derive(Default)]
pub struct MoveObjects;

/// How many answers come before the point to move from: none when control points are picked.
fn picked_first(cx: &Context<'_>) -> usize {
    usize::from(cx.doc.grip_selection().is_empty())
}

/// Where the move ends: the point given, or the height given straight out of the plane.
pub(super) fn end(cx: &Context<'_>, from: Point, value: Option<&Value>) -> Option<Point> {
    match value? {
        Value::Point(p) => Some(*p),
        Value::Number(height) => {
            let [x, y] = cx.plane();
            let up = cross(x, y);
            Some(std::array::from_fn(|i| from[i] + up[i] * height))
        }
        _ => None,
    }
}

/// The step asking where a move ends, straight up with `Vertical`.
pub(super) fn end_step(cx: &Context<'_>, prompt: &'static str, from: Point) -> StepDef {
    if cx.toggle(VERTICAL) {
        let [x, y] = cx.plane();
        StepDef::along("prompt.move.height", from, cross(x, y))
    } else {
        StepDef::point(prompt).from(from)
    }
}

impl Recipe for MoveObjects {
    const NAME: &'static str = "move";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(VERTICAL, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let first = picked_first(cx);
        let mut steps = Vec::new();
        if first == 1 {
            steps.push(StepDef::objects("prompt.move.objects").offering(&[]));
        }
        if cx.values.len() >= first {
            steps.push(StepDef::point("prompt.move.from"));
        }
        if let Some(from) = cx.values.get(first).and_then(Value::point) {
            steps.push(end_step(cx, "prompt.move.to", from));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let first = picked_first(cx);
        let Some(from) = cx.values.get(first).and_then(Value::point) else {
            return Ok(Made::default());
        };
        let Some(to) = end(cx, from, cx.values.get(first + 1)) else {
            return Ok(Made::default());
        };
        let by = sub(to, from);
        let guides = curva_kernel::Curve::line(from, to)
            .ok()
            .into_iter()
            .collect();
        if first == 0 {
            let grips = cx.doc.grip_selection();
            let moved = cx
                .doc
                .grips_changed(&grips, |p| std::array::from_fn(|i| p[i] + by[i]))
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            return Ok(Made {
                replace: moved
                    .into_iter()
                    .map(|(id, c)| (id, Shape::Curve(c)))
                    .collect(),
                guides,
                message: Some(Message::with(
                    "done.move_points",
                    vec![grips.len().to_string()],
                )),
                ..Made::default()
            });
        }
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = changed(cx, ids, &[Change::Move(by)], false, |_, _| None);
        made.guides.extend::<Vec<_>>(guides);
        made.message = Some(Message::with("done.move", vec![ids.len().to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn one_selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    #[test]
    fn picked_control_points_move_instead_of_the_object() {
        let mut doc = Document::default();
        let id = doc
            .add(Curve::polyline(vec![[0.0; 3], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]], false).unwrap());
        doc.select(&[id]);
        doc.points_on(&[id]);
        doc.select_grips(&[(id, 1)]);
        let mut processor = run(&mut doc, &["move", "0,0", "0,3"]);
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap(),
            [[0.0; 3], [2.0, 3.0, 0.0], [4.0, 0.0, 0.0]]
        );
        processor.submit(&mut doc, "undo");
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()[1],
            [2.0, 0.0, 0.0]
        );
    }

    #[test]
    fn moves_by_the_difference_between_the_points() {
        let mut doc = one_selected_line();
        run(&mut doc, &["move", "1,1,1", "4,5,6"]);
        let expected = Curve::line([3.0, 4.0, 5.0], [4.0, 4.0, 5.0]).unwrap();
        assert_eq!(*doc.state().objects[0].curve().unwrap(), expected);
    }

    #[test]
    fn asks_for_objects_when_nothing_is_selected() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["m"]);
        assert!(processor.prompt().unwrap().contains("Select"));
    }

    #[test]
    fn undo_puts_it_back() {
        let mut doc = one_selected_line();
        let before = doc.state().clone();
        run(&mut doc, &["move", "0,0", "9,9", "undo"]);
        assert_eq!(doc.state(), &before);
    }

    #[test]
    fn vertical_moves_straight_up_by_a_typed_height() {
        let mut doc = one_selected_line();
        run(&mut doc, &["move", "vertical", "0,0", "7"]);
        let start = doc.state().objects[0].curve().unwrap().sample().unwrap()[0];
        assert_eq!(start, [0.0, 0.0, 7.0]);
    }
}
