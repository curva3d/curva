//! Solid primitives, as in Rhino: `Sphere`, `Cylinder`, `Cone`, `TruncatedCone`, `Torus` and
//! `Tube`, each on the plane being drawn on, made by spinning or pulling a profile.
//! (`docs/commands/rhino-parity.md` lists the Rhino options still to come.)

use super::round_ways::{
    self, Base, FOUR_POINT, THREE_POINT, TWO_POINT, points_taken, sphere_through,
};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, KernelError, Point, cross};
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
/// While the pointer gives a round solid's last size, it shows as curves round its sides rather
/// than being built at every move of the pointer, as Rhino previews it (#671: a cylinder took
/// 60 ms and a torus 320 ms a move). Each profile is `(out, high)` points spun about `up` through
/// `centre`: a circle at every point off the axis, and the profile at four quarters round.
fn sketched(centre: Point, (x, y, up): (Point, Point, Point), profiles: &[&[(f64, f64)]]) -> Made {
    let mut curves = Vec::new();
    for profile in profiles {
        for (out, high) in profile.iter().filter(|(out, _)| *out > 1e-12) {
            curves.extend(Curve::circle_on(plus(centre, up, *high), *out, x, y));
        }
        for quarter in 0..4 {
            let (s, c) = (f64::from(quarter) * TAU / 4.0).sin_cos();
            let side: Point = std::array::from_fn(|i| x[i] * c + y[i] * s);
            let points = profile
                .iter()
                .map(|(out, high)| plus(plus(centre, side, *out), up, *high))
                .collect();
            curves.extend(Curve::polyline(points, false));
        }
    }
    Made {
        add: curves.into_iter().map(Shape::Curve).collect(),
        ..Made::default()
    }
}

fn circle_guide(centre: Point, r: f64, x: Point, y: Point) -> Made {
    Made {
        guides: Curve::circle_on(centre, r, x, y).ok().into_iter().collect(),
        ..Made::default()
    }
}

/// The base circle of a cylinder or cone, the way picked, and the index of the next answer.
fn round_base(cx: &Context<'_>) -> Option<(Base, usize)> {
    if let Some(n) = points_taken(cx.mode) {
        return round_ways::base(cx, cx.mode?).map(|base| (base, n));
    }
    let [x, y] = cx.plane();
    let (centre, size) = (point(cx, 0)?, number(cx, 1)?);
    let r = radius(cx, size);
    Some((Base { centre, r, x, y }, 2))
}

/// The steps to a round base: its centre and radius, or its points the `2Point` or `3Point` way.
fn base_steps(cx: &Context<'_>, centre: &'static str, size: &'static str) -> Vec<StepDef> {
    if let Some(mode) = cx.mode.filter(|_| points_taken(cx.mode).is_some()) {
        return round_ways::point_steps(cx, mode);
    }
    let mut steps = vec![StepDef::point(centre).offering(&[TWO_POINT, THREE_POINT])];
    steps.extend(point(cx, 0).map(|c| StepDef::number(size).from(c).offering(&[DIAMETER])));
    steps
}

/// The base once it is all in, and the index of the next answer.
fn whole_base(cx: &Context<'_>) -> Option<(Base, usize)> {
    round_base(cx).filter(|(_, next)| cx.values.len() >= *next)
}

/// The steps to a cylinder's or cone's base, then its height (offering `ending`).
fn round_steps(
    cx: &Context<'_>,
    [centre, size, height]: [&'static str; 3],
    ending: &[&'static str],
) -> Vec<StepDef> {
    let mut steps = base_steps(cx, centre, size);
    if let Some((base, _)) = whole_base(cx) {
        steps.push(StepDef::along(height, base.centre, base.up()).offering(ending));
    }
    steps
}

/// `Sphere`: its centre, then its radius; or the ends of a diameter with `2Point`, three
/// points on a great circle with `3Point`, or four points on it with `4Point`.
#[derive(Default)]
pub struct Sphere;

impl Recipe for Sphere {
    const NAME: &'static str = "sphere";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::mode(THREE_POINT),
        OptionDef::mode(FOUR_POINT),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        if let Some(mode) = cx.mode.filter(|_| points_taken(cx.mode).is_some()) {
            return round_ways::point_steps(cx, mode);
        }
        let first = StepDef::point("prompt.sphere.centre").offering(&[
            TWO_POINT,
            THREE_POINT,
            FOUR_POINT,
            DIAMETER,
        ]);
        let mut steps = vec![first];
        steps.extend(point(cx, 0).map(|c| {
            StepDef::number("prompt.sphere.radius")
                .from(c)
                .offering(&[DIAMETER])
        }));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let found = match cx.mode {
            Some(FOUR_POINT) => {
                let picked = [0, 1, 2, 3].map(|k| point(cx, k));
                match picked {
                    [Some(a), Some(b), Some(c), Some(d)] => Some(
                        sphere_through(a, b, c, d)
                            .ok_or_else(|| Message::new("error.bad_radius"))?,
                    ),
                    _ => None,
                }
            }
            Some(mode @ (TWO_POINT | THREE_POINT)) => {
                round_ways::base(cx, mode).map(|base| (base.centre, base.r))
            }
            _ => point(cx, 0)
                .zip(number(cx, 1))
                .map(|(c, size)| (c, radius(cx, size))),
        };
        let Some((centre, r)) = found else {
            return Ok(Made::default());
        };
        if r <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        // The engine's own sphere: one exact spherical face, as drawings need it (a half circle
        // spun round made a general spline face).
        made(Body::sphere(centre, r), "done.sphere")
    }
}

/// `Cylinder`: its base circle (centre and radius, `2Point` or `3Point`), then its height,
/// following the mouse. `Solid` off leaves the ends open; `BothSides` goes as far each way.
#[derive(Default)]
pub struct Cylinder;

impl Recipe for Cylinder {
    const NAME: &'static str = "cylinder";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::mode(THREE_POINT),
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        OptionDef::toggle(BOTH_SIDES, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let prompts = [
            "prompt.cylinder.centre",
            "prompt.cylinder.radius",
            "prompt.cylinder.height",
        ];
        round_steps(cx, prompts, &[SOLID, BOTH_SIDES])
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((base, next)) = round_base(cx) else {
            return Ok(Made::default());
        };
        let up = base.up();
        let Some(high) = number(cx, next).filter(|h| h.abs() > 1e-9) else {
            return Ok(circle_guide(base.centre, base.r, base.x, base.y));
        };
        let (start, length) = if cx.toggle(BOTH_SIDES) {
            (plus(base.centre, up, -high), 2.0 * high)
        } else {
            (base.centre, high)
        };
        if cx.previewing {
            let frame = (base.x, base.y, up);
            return Ok(sketched(
                start,
                frame,
                &[&[(base.r, 0.0), (base.r, length)]],
            ));
        }
        let circle = Curve::circle_on(start, base.r, base.x, base.y).map_err(|e| failed(&e))?;
        made(
            Body::extruded(&[circle], up, length, cx.toggle(SOLID)),
            "done.cylinder",
        )
    }
}

/// `Cone`: its base circle (centre and radius, `2Point` or `3Point`), then the height of its
/// apex.
#[derive(Default)]
pub struct Cone;

impl Recipe for Cone {
    const NAME: &'static str = "cone";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::mode(THREE_POINT),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let prompts = [
            "prompt.cone.centre",
            "prompt.cone.radius",
            "prompt.cone.apex",
        ];
        round_steps(cx, prompts, &[])
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((base, next)) = round_base(cx) else {
            return Ok(Made::default());
        };
        let Some(high) = number(cx, next).filter(|h| h.abs() > 1e-9) else {
            return Ok(circle_guide(base.centre, base.r, base.x, base.y));
        };
        let profile = [(0.0, 0.0), (base.r, 0.0), (0.0, high)];
        if cx.previewing {
            let frame = (base.x, base.y, base.up());
            return Ok(sketched(base.centre, frame, &[&profile[1..]]));
        }
        made(spun(base.centre, base.x, base.up(), &profile), "done.cone")
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
        if cx.previewing {
            return Ok(sketched(c, (x, y, up), &[&[(r, 0.0), (top, high)]]));
        }
        made(
            spun(c, x, up, &[(0.0, 0.0), (r, 0.0), (top, high), (0.0, high)]),
            "done.cone",
        )
    }
}

/// `Torus`: the circle through the middle of its tube (centre and radius, `2Point` or
/// `3Point`), then the tube's radius.
#[derive(Default)]
pub struct Torus;

impl Recipe for Torus {
    const NAME: &'static str = "torus";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::mode(THREE_POINT),
        OptionDef::toggle(DIAMETER, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = base_steps(cx, "prompt.torus.centre", "prompt.torus.radius");
        if let Some((base, _)) = whole_base(cx) {
            let on_ring = plus(base.centre, base.x, base.r);
            steps.push(
                StepDef::number("prompt.torus.second")
                    .from(on_ring)
                    .offering(&[]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((base, next)) = round_base(cx) else {
            return Ok(Made::default());
        };
        let Some(tube) = number(cx, next) else {
            return Ok(circle_guide(base.centre, base.r, base.x, base.y));
        };
        if tube <= 1e-9 || tube >= base.r {
            return Err(Message::new("error.torus"));
        }
        let (up, ring_at) = (base.up(), plus(base.centre, base.x, base.r));
        if cx.previewing {
            // The tube's inside and outside, its top and bottom, and its section at four quarters.
            let (r, t) = (base.r, tube);
            let rings: [&[(f64, f64)]; 4] =
                [&[(r - t, 0.0)], &[(r + t, 0.0)], &[(r, t)], &[(r, -t)]];
            let mut shown = sketched(base.centre, (base.x, base.y, up), &rings);
            shown
                .add
                .retain(|shape| matches!(shape, Shape::Curve(Curve::Circle { .. })));
            for quarter in 0..4 {
                let (s, c) = (f64::from(quarter) * TAU / 4.0).sin_cos();
                let side: Point = std::array::from_fn(|i| base.x[i] * c + base.y[i] * s);
                let at = plus(base.centre, side, r);
                shown
                    .add
                    .extend(Curve::circle_on(at, t, side, up).ok().map(Shape::Curve));
            }
            return Ok(shown);
        }
        let ring = Curve::circle_on(ring_at, tube, base.x, up).map_err(|e| failed(&e))?;
        made(
            Body::revolved(&[ring], base.centre, up, TAU, true),
            "done.torus",
        )
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
        if cx.previewing {
            let walls: [&[(f64, f64)]; 2] = [
                &[(outer, 0.0), (outer, length)],
                &[(inner, 0.0), (inner, length)],
            ];
            return Ok(sketched(start, (x, y, up), &walls));
        }
        let rings = [
            Curve::circle_on(start, outer, x, y).map_err(|e| failed(&e))?,
            Curve::circle_on(start, inner, x, y).map_err(|e| failed(&e))?,
        ];
        made(Body::extruded(&rings, up, length, true), "done.tube")
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use std::f64::consts::PI;

    #[test]
    fn round_solids_preview_as_curves_and_are_built_when_the_size_is_given() {
        for (command, answers) in [
            ("cylinder", vec!["0,0,0", "3"]),
            ("cone", vec!["0,0,0", "3"]),
            ("tube", vec!["0,0,0", "3", "2"]),
            ("torus", vec!["0,0,0", "5"]),
        ] {
            let mut doc = Document::default();
            let mut steps = vec![command];
            steps.extend(answers);
            let mut processor = run(&mut doc, &steps);
            assert!(
                processor.preview_bodies(&doc, None, "2").is_empty(),
                "{command}"
            );
            assert!(!processor.preview(&doc, None, "2").is_empty(), "{command}");
            processor.submit(&mut doc, "2");
            let body = doc.state().objects[0].shape.body().unwrap();
            assert!(body.is_solid(), "{command}");
        }
    }

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

    #[test]
    fn the_other_ways_measure_as_rhino_does() {
        // Rhino 8's volumes for the same picks (#448).
        for (lines, expected) in [
            (
                vec!["sphere", "3point", "0,0,0", "10,0,0", "5,5,0"],
                523.599,
            ),
            (
                vec!["sphere", "4point", "0,0,0", "10,0,0", "5,5,0", "5,2,4"],
                535.918,
            ),
            (vec!["cylinder", "2point", "0,0,0", "4,0,0", "5"], 62.832),
            (
                vec!["cylinder", "3point", "0,0,0", "4,0,0", "2,2,0", "5"],
                62.832,
            ),
            (vec!["cone", "2point", "0,0,0", "4,0,0", "5"], 20.944),
            (vec!["torus", "2point", "0,0,0", "20,0,0", "2"], 789.568),
            (
                vec!["torus", "3point", "0,0,0", "20,0,0", "10,10,0", "2"],
                789.568,
            ),
            (vec!["torus", "0,0,0", "diameter", "20", "2"], 789.568),
            (
                vec!["cone", "3point", "0,0,0", "4,0,0", "2,2,0", "5"],
                20.944,
            ),
        ] {
            let mut doc = Document::default();
            run(&mut doc, &lines);
            let made = volume(&doc);
            assert!(
                (made - expected).abs() < 1e-2,
                "{lines:?}: {made} against {expected}"
            );
        }
    }
}
