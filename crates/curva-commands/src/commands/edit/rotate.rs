//! `Rotate`, as in Rhino: turns objects about a centre on the plane being drawn on, by an angle
//! typed or between two reference points. `Copy` turns copies instead. Rhino's `UseLastCenter` is
//! still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_kernel::{Curve, Point, Similarity, cross, dot, flattened, sub, unit};

const COPY: &str = "Copy";

/// The recipe.
#[derive(Default)]
pub struct Rotate;

/// The signed turn from one reference point to another about `centre`, seen along `axis`.
fn turn(axis: Point, centre: Point, first: Point, second: Point) -> Option<f64> {
    let across = |p: Point| unit(flattened(sub(p, centre), axis));
    let (from, to) = (across(first)?, across(second)?);
    Some(dot(axis, cross(from, to)).atan2(dot(from, to)))
}

/// The turn the answers give, in radians: typed in degrees, or between two reference points.
fn angle(cx: &Context<'_>) -> Option<f64> {
    let centre = cx.values.get(1).and_then(Value::point)?;
    match cx.values.get(2)? {
        Value::Number(degrees) => Some(degrees.to_radians()),
        Value::Point(first) => {
            let [x, y] = cx.plane();
            turn(
                cross(x, y),
                centre,
                *first,
                cx.values.get(3).and_then(Value::point)?,
            )
        }
        _ => None,
    }
}

impl Recipe for Rotate {
    const NAME: &'static str = "rotate";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(COPY, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.rotate.objects").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.rotate.centre"));
        }
        if let Some(centre) = cx.values.get(1).and_then(Value::point) {
            steps.push(StepDef::point_or_number("prompt.rotate.angle").from(centre));
            if cx.values.get(2).and_then(Value::point).is_some() {
                steps.push(StepDef::point("prompt.rotate.second").from(centre));
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.values.get(3).is_some() && angle(cx).is_none() {
            return Err(Message::new("error.reference_point"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(centre)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::point))
        else {
            return Ok(Made::default());
        };
        let mut guides: Vec<Curve> = cx.values[2..]
            .iter()
            .filter_map(Value::point)
            .filter_map(|p| Curve::line(centre, p).ok())
            .collect();
        let Some(angle) = angle(cx) else {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        };
        let [x, y] = cx.plane();
        let change = Change::Map(Similarity::rotation(centre, cross(x, y), angle));
        let mut made = changed(cx, ids, &[change], cx.toggle(COPY), |_, _| None);
        made.guides.append(&mut guides);
        made.message = Some(Message::with("done.rotate", vec![ids.len().to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn start_of_first(doc: &Document) -> Point {
        doc.state().objects[0].curve().unwrap().sample().unwrap()[0]
    }

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-9)
    }

    #[test]
    fn a_typed_angle_turns_counterclockwise_in_degrees() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "90"]);
        assert!(close(start_of_first(&doc), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn two_reference_points_give_the_angle_between_them() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "5,0", "0,-3"]);
        assert!(close(start_of_first(&doc), [0.0, -1.0, 0.0]));
    }

    #[test]
    fn it_turns_about_the_plane_being_drawn_on() {
        let mut doc = selected_line();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["rotate", "0,0,0", "90"]);
        assert!(close(start_of_first(&doc), [0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_reference_point_on_the_centre_changes_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["rotate", "0,0", "0,0", "1,0"]);
        assert!(close(start_of_first(&doc), [1.0, 0.0, 0.0]));
        assert!(!processor.is_running());
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "0,0", "90", "undo"]);
        assert!(close(start_of_first(&doc), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn copy_turns_a_copy_and_keeps_the_original() {
        let mut doc = selected_line();
        run(&mut doc, &["rotate", "copy", "0,0", "90"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(close(start_of_first(&doc), [1.0, 0.0, 0.0]));
    }
}
