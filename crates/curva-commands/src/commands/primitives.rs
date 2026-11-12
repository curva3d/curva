//! Solid primitives, as in Rhino: `Sphere`, `Cylinder`, `Cone`, `TruncatedCone`, `Torus` and
//! `Tube`, each on the plane being drawn on, made by spinning or pulling a profile.
//! (`docs/commands/rhino-parity.md` lists the Rhino options still to come.)

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, KernelError, Point, cross, gap};
use std::f64::consts::TAU;

const DIAMETER: &str = "Diameter";
const SOLID: &str = "Solid";
const BOTH_SIDES: &str = "BothSides";

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn number(cx: &Context<'_>, index: usize) -> Option<f64> {
    cx.values.get(index).and_then(Value::number)
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

/// The plane's X direction and its normal.
fn frame(cx: &Context<'_>) -> (Point, Point, Point) {
    let [x, y] = cx.plane();
    (x, y, cross(x, y))
}

fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

/// A size given as a radius, or as a diameter with `Diameter` on.
fn radius(cx: &Context<'_>, size: f64) -> f64 {
    if cx.toggle(DIAMETER) {
        size / 2.0
    } else {
        size
    }
}

/// A solid of revolution about the line through `centre` along `up`, from a closed profile
/// drawn in the plane of `x` and `up` as (distance out, height) pairs.
fn spun(centre: Point, x: Point, up: Point, profile: &[(f64, f64)]) -> Result<Body, KernelError> {
    let points: Vec<Point> = profile
        .iter()
        .map(|(out, high)| plus(plus(centre, x, *out), up, *high))
        .collect();
    let outline = Curve::polyline(points, true)?;
    Body::revolved(&[outline], centre, up, TAU, true)
}

fn made(body: Result<Body, KernelError>, done: &'static str) -> Result<Made, Message> {
    let body = body.map_err(|e| failed(&e))?;
    Ok(Made {
        add: vec![Shape::Body(body)],
        message: Some(Message::new(done)),
        ..Made::default()
    })
}

/// A circle's guide while its centre and radius are being picked.
fn circle_guide(centre: Point, r: f64, x: Point, y: Point) -> Made {
    Made {
        guides: Curve::circle_on(centre, r, x, y).ok().into_iter().collect(),
        ..Made::default()
    }
}

/// `Sphere`: its centre, then its radius, or the ends of a diameter with `2Point`.
#[derive(Default)]
pub struct Sphere;

impl Recipe for Sphere {
    const NAME: &'static str = "sphere";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("2Point"),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        if cx.mode == Some("2Point") {
            let mut steps = vec![StepDef::point("prompt.sphere.start").offering(&[])];
            steps.extend(
                point(cx, 0).map(|a| StepDef::point("prompt.sphere.end").from(a).offering(&[])),
            );
            return steps;
        }
        let mut steps = vec![StepDef::point("prompt.sphere.centre").offering(&["2Point"])];
        steps.extend(point(cx, 0).map(|c| {
            StepDef::number("prompt.sphere.radius")
                .from(c)
                .offering(&[DIAMETER])
        }));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, _, up) = frame(cx);
        let (centre, r) = if cx.mode == Some("2Point") {
            let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
                return Ok(Made::default());
            };
            (
                std::array::from_fn(|i| (a[i] + b[i]) / 2.0),
                gap(a, b) / 2.0,
            )
        } else {
            let (Some(c), Some(size)) = (point(cx, 0), number(cx, 1)) else {
                return Ok(Made::default());
            };
            (c, radius(cx, size))
        };
        if r <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        // A half circle closed along its axis, spun a whole turn.
        let bottom = plus(centre, up, -r);
        let top = plus(centre, up, r);
        let arc = Curve::arc(centre, bottom, top, x, up).map_err(|e| failed(&e))?;
        let axis = Curve::line(top, bottom).map_err(|e| failed(&e))?;
        let half = Curve::compound(vec![arc, axis]).map_err(|e| failed(&e))?;
        made(
            Body::revolved(&[half], centre, up, TAU, true),
            "done.sphere",
        )
    }
}

/// `Cylinder`: the centre of its base, its radius, then its height, following the mouse.
/// `Solid` off leaves the ends open; `BothSides` goes as far each way.
#[derive(Default)]
pub struct Cylinder;

impl Recipe for Cylinder {
    const NAME: &'static str = "cylinder";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        OptionDef::toggle(BOTH_SIDES, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.cylinder.centre").offering(&[])];
        if let Some(c) = point(cx, 0) {
            steps.push(
                StepDef::number("prompt.cylinder.radius")
                    .from(c)
                    .offering(&[DIAMETER]),
            );
            if cx.values.len() >= 2 {
                let (_, _, up) = frame(cx);
                steps.push(
                    StepDef::along("prompt.cylinder.height", c, up).offering(&[SOLID, BOTH_SIDES]),
                );
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, y, up) = frame(cx);
        let (Some(c), Some(size)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let r = radius(cx, size);
        let Some(high) = number(cx, 2).filter(|h| h.abs() > 1e-9) else {
            return Ok(circle_guide(c, r, x, y));
        };
        let (start, length) = if cx.toggle(BOTH_SIDES) {
            (plus(c, up, -high), 2.0 * high)
        } else {
            (c, high)
        };
        let circle = Curve::circle_on(start, r, x, y).map_err(|e| failed(&e))?;
        made(
            Body::extruded(&[circle], up, length, cx.toggle(SOLID)),
            "done.cylinder",
        )
    }
}

/// `Cone`: the centre of its base, its radius, then the height of its apex.
#[derive(Default)]
pub struct Cone;

impl Recipe for Cone {
    const NAME: &'static str = "cone";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DIAMETER, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.cone.centre").offering(&[])];
        if let Some(c) = point(cx, 0) {
            steps.push(
                StepDef::number("prompt.cone.radius")
                    .from(c)
                    .offering(&[DIAMETER]),
            );
            if cx.values.len() >= 2 {
                let (_, _, up) = frame(cx);
                steps.push(StepDef::along("prompt.cone.apex", c, up).offering(&[]));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, y, up) = frame(cx);
        let (Some(c), Some(size)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let r = radius(cx, size);
        let Some(high) = number(cx, 2).filter(|h| h.abs() > 1e-9) else {
            return Ok(circle_guide(c, r, x, y));
        };
        made(
            spun(c, x, up, &[(0.0, 0.0), (r, 0.0), (0.0, high)]),
            "done.cone",
        )
    }
}

/// `TruncatedCone`: the centre of its base, its radius, its height, then the radius at the top.
#[derive(Default)]
pub struct TruncatedCone;

impl Recipe for TruncatedCone {
    const NAME: &'static str = "truncatedcone";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.cone.centre")];
        if let Some(c) = point(cx, 0) {
            let (_, _, up) = frame(cx);
            steps.push(StepDef::number("prompt.cone.radius").from(c));
            if cx.values.len() >= 2 {
                steps.push(StepDef::along("prompt.truncatedcone.height", c, up));
            }
            if let Some(high) = number(cx, 2) {
                steps.push(StepDef::number("prompt.truncatedcone.top").from(plus(c, up, high)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, y, up) = frame(cx);
        let (Some(c), Some(r)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let (Some(high), Some(top)) = (number(cx, 2), number(cx, 3)) else {
            return Ok(circle_guide(c, r, x, y));
        };
        if high.abs() <= 1e-9 {
            return Err(Message::new("error.bad_distance"));
        }
        made(
            spun(c, x, up, &[(0.0, 0.0), (r, 0.0), (top, high), (0.0, high)]),
            "done.cone",
        )
    }
}

/// `Torus`: its centre, the radius to the middle of the tube, then the tube's radius.
#[derive(Default)]
pub struct Torus;

impl Recipe for Torus {
    const NAME: &'static str = "torus";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.torus.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::number("prompt.torus.radius").from(c));
            if let Some(r) = number(cx, 1) {
                let (x, _, _) = frame(cx);
                steps.push(StepDef::number("prompt.torus.second").from(plus(c, x, r)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, y, up) = frame(cx);
        let (Some(c), Some(r)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(tube) = number(cx, 2) else {
            return Ok(circle_guide(c, r, x, y));
        };
        if tube <= 1e-9 || tube >= r {
            return Err(Message::new("error.torus"));
        }
        let ring = Curve::circle_on(plus(c, x, r), tube, x, up).map_err(|e| failed(&e))?;
        made(Body::revolved(&[ring], c, up, TAU, true), "done.torus")
    }
}

/// `Tube`: the centre of its base, the outer radius, the inner radius, then its height.
#[derive(Default)]
pub struct Tube;

impl Recipe for Tube {
    const NAME: &'static str = "tube";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(BOTH_SIDES, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.tube.centre").offering(&[])];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::number("prompt.tube.outer").from(c).offering(&[]));
            if cx.values.len() >= 2 {
                steps.push(StepDef::number("prompt.tube.inner").from(c).offering(&[]));
            }
            if cx.values.len() >= 3 {
                let (_, _, up) = frame(cx);
                steps.push(StepDef::along("prompt.tube.height", c, up));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (x, y, up) = frame(cx);
        let (Some(c), Some(outer)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let (Some(inner), Some(high)) = (number(cx, 2), number(cx, 3)) else {
            return Ok(circle_guide(c, outer, x, y));
        };
        if inner <= 1e-9 || inner >= outer || high.abs() <= 1e-9 {
            return Err(Message::new("error.tube"));
        }
        let (start, length) = if cx.toggle(BOTH_SIDES) {
            (plus(c, up, -high), 2.0 * high)
        } else {
            (c, high)
        };
        let rings = [
            Curve::circle_on(start, outer, x, y).map_err(|e| failed(&e))?,
            Curve::circle_on(start, inner, x, y).map_err(|e| failed(&e))?,
        ];
        made(Body::extruded(&rings, up, length, true), "done.tube")
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use std::f64::consts::PI;

    fn volume(doc: &Document) -> f64 {
        doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn the_primitives_have_their_volumes() {
        for (lines, expected) in [
            (vec!["sphere", "0,0,0", "5"], 4.0 / 3.0 * PI * 125.0),
            (
                vec!["sphere", "2point", "-5,0,0", "5,0,0"],
                4.0 / 3.0 * PI * 125.0,
            ),
            (vec!["cylinder", "0,0,0", "3", "10"], PI * 9.0 * 10.0),
            (vec!["cone", "0,0,0", "4", "6"], PI * 16.0 * 6.0 / 3.0),
            (
                vec!["truncatedcone", "0,0,0", "4", "6", "2"],
                PI * 6.0 / 3.0 * (16.0 + 8.0 + 4.0),
            ),
            (vec!["torus", "0,0,0", "8", "2"], 2.0 * PI * PI * 8.0 * 4.0),
            (vec!["tube", "0,0,0", "3", "1", "10"], PI * 8.0 * 10.0),
        ] {
            let mut doc = Document::default();
            run(&mut doc, &lines);
            let made = volume(&doc);
            assert!(
                (made - expected).abs() < 1e-3 * expected,
                "{lines:?}: {made} against {expected}"
            );
        }
    }

    #[test]
    fn options_change_them() {
        let mut doc = Document::default();
        run(&mut doc, &["sphere", "0,0,0", "diameter", "10"]);
        assert!((volume(&doc) - 4.0 / 3.0 * PI * 125.0).abs() < 1e-3);
        let mut doc = Document::default();
        run(&mut doc, &["cylinder", "0,0,0", "3", "bothsides", "5"]);
        assert!((volume(&doc) - PI * 9.0 * 10.0).abs() < 1e-3);
    }
}
