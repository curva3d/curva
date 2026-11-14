//! `Mirror`, as in Rhino: a mirror image of objects across a line on the plane being drawn on.
//! `Copy` (on, as in Rhino) keeps the originals, and each curve's image then follows it
//! (`History`). `XAxis` and `YAxis` mirror across the plane's own axes at once. Rhino's `3Point`,
//! `ZAxis` and `Object` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_kernel::{Curve, Point, Similarity, cross, sub, unit};

const COPY: &str = "Copy";
const X_AXIS: &str = "XAxis";
const Y_AXIS: &str = "YAxis";

/// The recipe.
#[derive(Default)]
pub struct Mirror;

/// The mirror image across the line from `start` to `end` on a plane with normal `axis`, or
/// `None` if the line has no length.
fn reflection(axis: Point, start: Point, end: Point) -> Option<Similarity> {
    let normal = unit(cross(sub(end, start), axis))?;
    Some(Similarity::mirror(start, normal))
}

impl Recipe for Mirror {
    const NAME: &'static str = "mirror";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(COPY, true),
        OptionDef::finish(X_AXIS),
        OptionDef::finish(Y_AXIS),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.mirror.objects").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.mirror.start"));
        }
        if let Some(start) = cx.values.get(1).and_then(Value::point) {
            steps.push(
                StepDef::point("prompt.mirror.end")
                    .from(start)
                    .offering(&[COPY]),
            );
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let (Some(start), Some(end)) = (
            cx.values.get(1).and_then(Value::point),
            cx.values.get(2).and_then(Value::point),
        ) {
            let [x, y] = cx.plane();
            if reflection(cross(x, y), start, end).is_none() {
                return Err(Message::new("error.reference_point"));
            }
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let (origin, _) = cx.doc.work_frame();
        let (start, end): (Point, Point) = match cx.mode {
            Some(X_AXIS) => (origin, std::array::from_fn(|i| origin[i] + x[i])),
            Some(Y_AXIS) => (origin, std::array::from_fn(|i| origin[i] + y[i])),
            _ => match (
                cx.values.get(1).and_then(Value::point),
                cx.values.get(2).and_then(Value::point),
            ) {
                (Some(start), Some(end)) => (start, end),
                _ => return Ok(Made::default()),
            },
        };
        let Some(change) = reflection(cross(x, y), start, end) else {
            return Ok(Made::default());
        };
        let copy = cx.toggle(COPY);
        let mut made = changed(cx, ids, &[Change::Map(change)], copy, |source, _| {
            let is_curve = cx
                .doc
                .state()
                .objects
                .iter()
                .any(|o| o.id == source && o.curve().is_some());
            is_curve.then_some(curva_document::Recipe::Mirror { source, change })
        });
        made.guides.extend(Curve::line(start, end).ok());
        let count = if copy { made.derived.len() } else { ids.len() };
        made.message = Some(Message::with("done.mirror", vec![count.to_string()]));
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
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document, index: usize) -> Vec<Point> {
        doc.state().objects[index]
            .curve()
            .unwrap()
            .sample()
            .unwrap()
    }

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-9)
    }

    #[test]
    fn it_adds_a_mirror_image_and_keeps_the_original() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "0,0", "0,10"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert_eq!(ends(&doc, 0)[0], [1.0, 2.0, 0.0]);
        assert!(close(ends(&doc, 1)[0], [-1.0, 2.0, 0.0]));
        assert!(close(ends(&doc, 1)[1], [-3.0, 2.0, 0.0]));
    }

    #[test]
    fn it_mirrors_across_a_slanted_line() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "0,0", "5,5"]);
        assert!(close(ends(&doc, 1)[0], [2.0, 1.0, 0.0]));
    }

    #[test]
    fn it_uses_the_plane_being_drawn_on() {
        let mut doc = selected_line();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["mirror", "0,0,0", "1,0,0"]);
        assert!(close(ends(&doc, 1)[0], [1.0, 2.0, 0.0]));
    }

    #[test]
    fn a_line_with_no_length_makes_nothing() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "1,1", "1,1"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn x_axis_mirrors_at_once_and_copy_off_moves_the_original() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "xaxis"]);
        assert!(close(ends(&doc, 1)[0], [1.0, -2.0, 0.0]));
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "0,0", "copy", "1,0"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(close(ends(&doc, 0)[0], [1.0, -2.0, 0.0]));
    }
}
