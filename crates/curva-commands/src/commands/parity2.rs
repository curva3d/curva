//! A second batch of Rhino's commands (spec `docs/specs/rhino-parity.md`): `Points` and `Lines`
//! (many at once), `Slot`, `Ellipsoid`, `BooleanSplit` and `DupEdge`.

use super::bodies_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Combine, Curve, Point, cross, dot, gap, sub, unit};
use std::f64::consts::TAU;

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn failed(error: &curva_kernel::KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

/// The points answered so far, up to an Enter.
fn picked(cx: &Context<'_>) -> Vec<Point> {
    cx.values.iter().filter_map(Value::point).collect()
}

/// Steps for one point after another until Enter.
fn many_points(cx: &Context<'_>, prompt: &'static str) -> Vec<StepDef> {
    let mut steps: Vec<StepDef> = cx.values.iter().map(|_| StepDef::point(prompt)).collect();
    if cx.values.last() != Some(&Value::Empty) {
        let mut step = StepDef::point(prompt);
        if let Some(last) = picked(cx).last() {
            step = step.from(*last);
        }
        steps.push(if picked(cx).is_empty() {
            step
        } else {
            step.optional()
        });
    }
    steps
}

/// `Points`: places points one after another until Enter.
#[derive(Default)]
pub struct Points;

impl Recipe for Points {
    const NAME: &'static str = "points";
    const PLACES_AS_IT_GOES: bool = true;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        many_points(cx, "prompt.point.at")
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = picked(cx);
        let count = points.len();
        Ok(Made {
            add: points
                .into_iter()
                .map(|at| Shape::Curve(Curve::Point { at }))
                .collect(),
            message: Some(Message::with("done.points", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

/// `Lines`: separate lines, each from its own two points, until Enter.
#[derive(Default)]
pub struct Lines;

impl Recipe for Lines {
    const NAME: &'static str = "lines";
    const PLACES_AS_IT_GOES: bool = true;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let points = picked(cx);
        let mut steps: Vec<StepDef> = Vec::new();
        for (i, _) in cx.values.iter().enumerate() {
            steps.push(StepDef::point(if i % 2 == 0 {
                "prompt.line.start"
            } else {
                "prompt.line.end"
            }));
        }
        if cx.values.last() == Some(&Value::Empty) {
            return steps;
        }
        if points.len() % 2 == 1 {
            steps.push(StepDef::point("prompt.line.end").from(points[points.len() - 1]));
        } else {
            let step = StepDef::point("prompt.line.start");
            steps.push(if points.is_empty() {
                step
            } else {
                step.optional()
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = picked(cx);
        let lines: Vec<Shape> = points
            .chunks(2)
            .filter(|pair| pair.len() == 2)
            .filter_map(|pair| Curve::line(pair[0], pair[1]).ok().map(Shape::Curve))
            .collect();
        let count = lines.len();
        Ok(Made {
            add: lines,
            message: Some(Message::with("done.lines", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

/// `Slot`: a slot from the centres of its two ends, then its width.
#[derive(Default)]
pub struct Slot;

impl Recipe for Slot {
    const NAME: &'static str = "slot";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.slot.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.slot.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.slot.width").from(b));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(width) = cx.values.get(2).and_then(Value::number) else {
            return Ok(Made {
                guides: Curve::line(a, b).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, y] = cx.plane();
        let normal = cross(x, y);
        let along = unit(sub(b, a)).ok_or_else(|| Message::new("error.reference_point"))?;
        let side = cross(normal, along);
        let r = width / 2.0;
        if r <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        let parts = vec![
            Curve::line(plus(a, side, -r), plus(b, side, -r)),
            Curve::arc(b, plus(b, side, -r), plus(b, side, r), along, side),
            Curve::line(plus(b, side, r), plus(a, side, r)),
            Curve::arc(
                a,
                plus(a, side, r),
                plus(a, side, -r),
                plus([0.0; 3], along, -1.0),
                plus([0.0; 3], side, -1.0),
            ),
        ]
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| failed(&e))?;
        let slot = Curve::compound(parts).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(slot)],
            message: Some(Message::new("done.slot")),
            ..Made::default()
        })
    }
}

/// `Ellipsoid`: its centre, the end of its first axis, the end of its second, then the third's
/// length straight out of the plane.
#[derive(Default)]
pub struct Ellipsoid;

impl Recipe for Ellipsoid {
    const NAME: &'static str = "ellipsoid";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.ellipse.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.ellipse.first").from(c));
            if cx.values.len() >= 2 {
                steps.push(StepDef::point("prompt.ellipse.second").from(c));
            }
            if cx.values.len() >= 3 {
                steps.push(StepDef::number("prompt.ellipsoid.third").from(c));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(first)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let up = cross(x, y);
        let a = unit(sub(first, c)).ok_or_else(|| Message::new("error.reference_point"))?;
        let b = cross(up, a);
        let ra = gap(first, c);
        let (Some(second), Some(rc)) = (point(cx, 2), cx.values.get(3).and_then(Value::number))
        else {
            return Ok(Made {
                guides: Curve::line(c, first).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let rb = dot(sub(second, c), b).abs();
        if ra <= 1e-9 || rb <= 1e-9 || rc <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        // A sphere of the first radius, stretched along the other two axes.
        let bottom = plus(c, up, -ra);
        let top = plus(c, up, ra);
        let arc = Curve::arc(c, bottom, top, a, up).map_err(|e| failed(&e))?;
        let axis = Curve::line(top, bottom).map_err(|e| failed(&e))?;
        let half = Curve::compound(vec![arc, axis]).map_err(|e| failed(&e))?;
        let body = Body::revolved(&[half], c, up, TAU, true)
            .and_then(|s| s.stretched(c, b, rb / ra))
            .and_then(|s| s.stretched(c, up, rc / ra))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.ellipsoid")),
            ..Made::default()
        })
    }
}

/// `BooleanSplit`: splits solids with others into the part inside and the part outside.
#[derive(Default)]
pub struct BooleanSplit;

impl Recipe for BooleanSplit {
    const NAME: &'static str = "booleansplit";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("DeleteInput", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.booleansplit.split").bodies()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.booleansplit.with")
                    .bodies()
                    .apart(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(split)), Some(Value::Objects(with))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let cutters = bodies_of(cx.doc, with);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, split) {
            for (_, cutter) in &cutters {
                for how in [Combine::Difference, Combine::Intersection] {
                    if let Some(part) = body.combined(cutter, how).map_err(|e| failed(&e))? {
                        made.derived.push(Derived {
                            shape: Shape::Body(part),
                            from: id,
                            history: None,
                        });
                    }
                }
            }
            if cx.toggle("DeleteInput") {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.booleansplit",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `DupEdge`: copies picked edges of surfaces and solids as curves.
#[derive(Default)]
pub struct DupEdge;

impl Recipe for DupEdge {
    const NAME: &'static str = "dupedge";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::edges("prompt.dupedge.edges")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Edges(edges)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, edge) in edges {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let curves = body.edge_curves().map_err(|e| failed(&e))?;
            if let Some(curve) = curves.get(*edge) {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve.clone()),
                    from: *id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupedge",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use std::f64::consts::PI;

    #[test]
    fn points_lines_and_slot() {
        let mut doc = Document::default();
        run(&mut doc, &["points", "0,0", "1,0", "2,0", ""]);
        assert_eq!(doc.state().objects.len(), 3);
        let mut doc = Document::default();
        run(&mut doc, &["lines", "0,0", "1,0", "0,1", "1,1", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        let mut doc = Document::default();
        run(&mut doc, &["slot", "0,0", "10,0", "4"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - (20.0 + 2.0 * PI * 2.0)).abs() < 1e-3, "{length}");
    }

    #[test]
    fn ellipsoid_and_boolean_split() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipsoid", "0,0,0", "5,0,0", "0,3,0", "2"]);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 4.0 / 3.0 * PI * 30.0).abs() < 1e-2, "{volume}");
        let mut doc = Document::default();
        let a = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let b = doc.add_body(curva_kernel::Body::cuboid([2.0, 0.0, 0.0], [4.0, 4.0, 4.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "booleansplit");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        assert_eq!(doc.state().objects.len(), 3);
    }
}
