//! `Ellipse`, as in Rhino: its centre, the end of one axis, then the end of the other. With
//! `Diameter`, the ends of the first axis, then the end of the second; with `Corner`, two opposite
//! corners of the rectangle around it. Rhino's `Deformable`, `Vertical`, `FromFoci`, `MarkFoci` and
//! `AroundCurve` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, KernelError, Point, cross, dot, sub, unit};

const DIAMETER: &str = "Diameter";
const CORNER: &str = "Corner";

/// The recipe.
#[derive(Default)]
pub struct Ellipse;

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn scaled(v: Point, by: f64) -> Point {
    v.map(|c| c * by)
}

/// The ellipse about `centre` with its first axis reaching `first`, and its second as far across
/// as `toward` is, on the plane with normal `normal`.
fn through(
    centre: Point,
    first: Point,
    toward: Point,
    normal: Point,
) -> Result<Curve, KernelError> {
    let a = sub(first, centre);
    let a = sub(a, scaled(normal, dot(a, normal)));
    let side = unit(cross(normal, a))
        .ok_or_else(|| KernelError("the axes need some length".to_owned()))?;
    let reach = dot(sub(toward, centre), side).abs();
    Curve::ellipse(centre, a, scaled(side, reach))
}

impl Recipe for Ellipse {
    const NAME: &'static str = "ellipse";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(DIAMETER), OptionDef::mode(CORNER)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let prompts: [&'static str; 3] = match cx.mode {
            Some(DIAMETER) => [
                "prompt.ellipse.axis_start",
                "prompt.ellipse.axis_end",
                "prompt.ellipse.second",
            ],
            Some(CORNER) => ["prompt.ellipse.corner", "prompt.ellipse.other_corner", ""],
            _ => [
                "prompt.ellipse.centre",
                "prompt.ellipse.first",
                "prompt.ellipse.second",
            ],
        };
        let mut steps = vec![StepDef::point(prompts[0])];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point(prompts[1]).from(a).offering(&[]));
            if cx.mode != Some(CORNER) {
                // The second axis is measured from the centre.
                let centre = match (cx.mode, point(cx, 1)) {
                    (Some(DIAMETER), Some(b)) => Some(std::array::from_fn(|i| (a[i] + b[i]) / 2.0)),
                    (_, Some(_)) => Some(a),
                    _ => None,
                };
                steps.extend(centre.map(|c| StepDef::point(prompts[2]).from(c).offering(&[])));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let [x, y] = cx.plane();
        let normal = cross(x, y);
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let failed = |e: KernelError| Message::with("error.geometry", vec![e.to_string()]);
        let (ellipse, guides) = match cx.mode {
            Some(CORNER) => {
                let across = sub(b, a);
                let centre = std::array::from_fn(|i| (a[i] + b[i]) / 2.0);
                let (wide, high) = (dot(across, x) / 2.0, dot(across, y) / 2.0);
                let curve =
                    Curve::ellipse(centre, scaled(x, wide), scaled(y, high)).map_err(failed)?;
                (curve, Vec::new())
            }
            mode => {
                let (centre, first) = if mode == Some(DIAMETER) {
                    (std::array::from_fn(|i| (a[i] + b[i]) / 2.0), b)
                } else {
                    (a, b)
                };
                let Some(toward) = point(cx, 2) else {
                    return Ok(Made {
                        guides: Curve::line(if mode == Some(DIAMETER) { a } else { centre }, first)
                            .ok()
                            .into_iter()
                            .collect(),
                        ..Made::default()
                    });
                };
                let curve = through(centre, first, toward, normal).map_err(failed)?;
                (curve, Curve::line(centre, first).ok().into_iter().collect())
            }
        };
        Ok(Made {
            add: vec![Shape::Curve(ellipse)],
            guides,
            message: Some(Message::new("done.ellipse")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn makes_an_ellipse_with_the_axes_picked() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "0,0", "5,0", "9,3"]);
        for point in doc.state().objects[0].curve().unwrap().sample().unwrap() {
            let value = (point[0] / 5.0).powi(2) + (point[1] / 3.0).powi(2);
            assert!((value - 1.0).abs() < 1e-6, "{point:?}");
        }
    }

    #[test]
    fn it_goes_on_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["ellipse", "0,0", "5,0", "0,3"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
    }

    #[test]
    fn axes_with_no_length_make_nothing() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "0,0", "0,0", "1,1"]);
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn diameter_and_corner() {
        let on_ellipse = |doc: &Document| {
            doc.state().objects[0]
                .curve()
                .unwrap()
                .sample()
                .unwrap()
                .iter()
                .all(|p| ((p[0] / 5.0).powi(2) + (p[1] / 3.0).powi(2) - 1.0).abs() < 1e-6)
        };
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "diameter", "-5,0", "5,0", "2,3"]);
        assert!(on_ellipse(&doc));
        let mut doc = Document::default();
        run(&mut doc, &["ellipse", "corner", "-5,-3", "5,3"]);
        assert!(on_ellipse(&doc));
    }
}
