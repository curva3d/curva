//! The `Twist` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Point, cross, dot, sub, unit};

/// `Twist`: turns objects about an axis, more the further along it: the axis, then the angle at
/// its end.
#[derive(Default)]
pub struct Twist;

impl Recipe for Twist {
    const NAME: &'static str = "twist";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        axis_steps(
            cx,
            "prompt.twist.objects",
            Some(StepDef::number("prompt.twist.angle").remembered("Angle", 90.0)),
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(angle)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        let e = unit(sub(b, a)).ok_or_else(|| Message::new("error.reference_point"))?;
        let map = move |p: Point| {
            let (t, foot, r) = along_axis(p, a, b);
            let theta = (angle * t).to_radians();
            let (c, s) = (theta.cos(), theta.sin());
            let turned: Point = std::array::from_fn(|i| {
                r[i] * c + cross(e, r)[i] * s + e[i] * dot(e, r) * (1.0 - c)
            });
            plus(foot, turned, 1.0)
        };
        deform_all(cx, objects(cx, 0), &map, "done.twist")
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    fn last_curve(doc: &Document) -> Curve {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.curve().cloned())
            .unwrap()
    }

    #[test]
    fn deformations_move_objects() {
        let mut doc = Document::default();
        let id = doc.add(line([1.0, 0.0, 0.0], [1.0, 0.0, 10.0]));
        doc.select(&[id]);
        run(&mut doc, &["twist", "0,0,0", "0,0,10", "90"]);
        let top = last_curve(&doc).sample().unwrap().last().copied().unwrap();
        assert!(
            top[0].abs() < 1e-6 && (top[1] - 1.0).abs() < 1e-6,
            "{top:?}"
        );
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([-1.0, -1.0, 0.0], [2.0, 2.0, 4.0]).unwrap());
        doc.select(&[cube]);
        run(&mut doc, &["taper", "0,0,0", "0,0,4", "0.5"]);
        let tapered = last_body(&doc);
        assert!(tapered.is_solid());
        // A frustum from 2 × 2 to 1 × 1, 4 high: 4/3 (4 + 1 + 2).
        assert!(
            (tapered.volume().unwrap() - 4.0 / 3.0 * 7.0).abs() < 0.05,
            "{}",
            tapered.volume().unwrap()
        );
        let mut doc = Document::default();
        let id = doc.add(line([0.0; 3], [10.0, 0.0, 0.0]));
        doc.select(&[id]);
        run(&mut doc, &["bend", "0,0", "10,0", "5,3"]);
        assert!(
            last_curve(&doc)
                .sample()
                .unwrap()
                .iter()
                .any(|p| p[1].abs() > 0.5)
        );
        let mut doc = Document::default();
        let id = doc.add(line([2.0, 1.0, 0.0], [4.0, 1.0, 0.0]));
        let base = doc.add(line([0.0; 3], [10.0, 0.0, 0.0]));
        let target = doc.add(line([0.0; 3], [0.0, 10.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[id]);
        processor.submit(&mut doc, "flow");
        doc.select(&[base]);
        processor.submit(&mut doc, "");
        doc.select(&[target]);
        processor.submit(&mut doc, "");
        let moved = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(moved.iter().all(|p| p[0] < 0.0 && p[1] > 1.0), "{moved:?}");
        let mut doc = Document::default();
        let id = doc.add(line([0.0; 3], [2.0, 2.0, 2.0]));
        doc.select(&[id]);
        run(&mut doc, &["cageedit", "2,2,2", "4,4,4", ""]);
        let end = doc.state().objects[0]
            .curve()
            .unwrap()
            .sample()
            .unwrap()
            .last()
            .copied()
            .unwrap();
        assert!(curva_kernel::gap(end, [4.0, 4.0, 4.0]) < 1e-6, "{end:?}");
    }
}
