//! More of Rhino's commands (spec `docs/specs/rhino-parity.md`): curves (`Polygon`, `Helix`,
//! `Spiral`), surfaces (`Plane`, `SrfPt`, `ExtrudeSrf`), `ArrayLinear`, measuring (`Angle`,
//! `Radius`, `EvaluatePt`, `What`), selecting curves by kind, and `HideSwap`.

use super::{Change, bodies_of, changed};
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use crate::{Answer, Command, Message, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve, Point, cross, dot, gap, sub, unit};
use std::f64::consts::TAU;

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn number(cx: &Context<'_>, index: usize) -> Option<f64> {
    cx.values.get(index).and_then(Value::number)
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn failed(error: &curva_kernel::KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

fn added(
    curve: Result<Curve, curva_kernel::KernelError>,
    done: &'static str,
) -> Result<Made, Message> {
    let curve = curve.map_err(|e| failed(&e))?;
    Ok(Made {
        add: vec![Shape::Curve(curve)],
        message: Some(Message::new(done)),
        ..Made::default()
    })
}

const SIDES: &str = "NumSides";
const CIRCUMSCRIBED: &str = "Circumscribed";

/// `Polygon`: a regular polygon from its centre and a corner (or, with `Circumscribed`, the
/// middle of a side); `NumSides` sets how many.
#[derive(Default)]
pub struct Polygon;

impl Recipe for Polygon {
    const NAME: &'static str = "polygon";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SIDES, 5, 3, 100),
        OptionDef::toggle(CIRCUMSCRIBED, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.polygon.centre")];
        steps.extend(point(cx, 0).map(|c| StepDef::point("prompt.polygon.corner").from(c)));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(corner)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sides = cx.number(SIDES) as usize;
        let to = sub(corner, c);
        let (u, v) = (dot(to, x), dot(to, y));
        let mut reach = u.hypot(v);
        let mut start = v.atan2(u);
        if cx.toggle(CIRCUMSCRIBED) {
            // The point picked is the middle of a side.
            reach /= (std::f64::consts::PI / sides as f64).cos();
            start += std::f64::consts::PI / sides as f64;
        }
        if reach <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        let points = (0..sides)
            .map(|i| {
                let angle = start + TAU * i as f64 / sides as f64;
                plus(plus(c, x, reach * angle.cos()), y, reach * angle.sin())
            })
            .collect();
        added(Curve::polyline(points, true), "done.polygon")
    }
}

const TURNS: &str = "Turns";

/// Points along a turning path from `start` to `end` about that axis, its radius going from
/// `from` to `to`, `turns` times round; with `flat`, all on the plane at `start`.
fn turning(
    start: Point,
    end: Point,
    x: Point,
    from: f64,
    to: f64,
    turns: f64,
    flat: bool,
) -> Option<Vec<Point>> {
    let axis = unit(sub(end, start))?;
    let side = unit(sub(x, plus([0.0; 3], axis, dot(x, axis))))
        .or_else(|| unit(cross(axis, [0.0, 0.0, 1.0])))
        .or_else(|| unit(cross(axis, [0.0, 1.0, 0.0])))?;
    let other = cross(axis, side);
    let length = gap(start, end);
    let steps = (turns * 16.0).ceil().max(8.0) as usize;
    Some(
        (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                let angle = TAU * turns * t;
                let r = from + (to - from) * t;
                let along = if flat { 0.0 } else { length * t };
                plus(
                    plus(plus(start, axis, along), side, r * angle.cos()),
                    other,
                    r * angle.sin(),
                )
            })
            .collect(),
    )
}

/// `Helix`: the start and end of its axis, then its radius; `Turns` sets how many times round.
#[derive(Default)]
pub struct Helix;

impl Recipe for Helix {
    const NAME: &'static str = "helix";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::number(TURNS, 10.0)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.helix.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.helix.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.helix.radius").from(b));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(r)) = (point(cx, 0), point(cx, 1), number(cx, 2)) else {
            return Ok(Made::default());
        };
        let [x, _] = cx.plane();
        let turns = cx.number(TURNS).max(0.1);
        let points = turning(a, b, x, r, r, turns, false)
            .ok_or_else(|| Message::new("error.reference_point"))?;
        added(Curve::interpolated(points, false), "done.helix")
    }
}

/// `Spiral`: the start and end of its axis, its first radius, then its second; `Turns` sets how
/// many times round, and `Flat` lays it on the plane at the axis's start.
#[derive(Default)]
pub struct Spiral;

impl Recipe for Spiral {
    const NAME: &'static str = "spiral";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number(TURNS, 10.0),
        OptionDef::toggle("Flat", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.helix.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.helix.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.spiral.first").from(a));
                if cx.values.len() >= 3 {
                    steps.push(StepDef::number("prompt.spiral.second").from(b));
                }
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(r1), Some(r2)) =
            (point(cx, 0), point(cx, 1), number(cx, 2), number(cx, 3))
        else {
            return Ok(Made::default());
        };
        let [x, _] = cx.plane();
        let turns = cx.number(TURNS).max(0.1);
        let points = turning(a, b, x, r1, r2, turns, cx.toggle("Flat"))
            .ok_or_else(|| Message::new("error.reference_point"))?;
        added(Curve::interpolated(points, false), "done.spiral")
    }
}

/// `Plane`: a flat rectangular surface from two opposite corners, on the plane being drawn on.
#[derive(Default)]
pub struct Plane;

impl Recipe for Plane {
    const NAME: &'static str = "plane";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.rectangle.first")];
        steps.extend(
            point(cx, 0).map(|a| StepDef::point("prompt.rectangle.other").from(a).corner()),
        );
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let across = sub(b, a);
        let (w, h) = (dot(across, x), dot(across, y));
        if w.abs() < 1e-9 || h.abs() < 1e-9 {
            return Err(Message::new("error.flat_rectangle"));
        }
        // As Rhino's Plane: an untrimmed surface, ruled between two opposite sides.
        let near = Curve::line(a, plus(a, x, w)).map_err(|e| failed(&e))?;
        let far = Curve::line(plus(a, y, h), plus(plus(a, x, w), y, h)).map_err(|e| failed(&e))?;
        let body = Body::ruled(&near, &far, false).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.planarsrf_one")),
            ..Made::default()
        })
    }
}

/// `SrfPt`: a surface from three or four corner points; Enter after three makes a triangle.
#[derive(Default)]
pub struct SrfPt;

impl Recipe for SrfPt {
    const NAME: &'static str = "srfpt";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.srfpt.first")];
        for i in 0..3 {
            if let Some(p) = point(cx, i) {
                let step = StepDef::point("prompt.srfpt.next").from(p);
                steps.push(if i == 2 { step.optional() } else { step });
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points: Vec<Point> = cx.values.iter().filter_map(Value::point).collect();
        let finished = cx.values.len() >= 4;
        let body = match points.as_slice() {
            [a, b, c] if finished => {
                let outline = Curve::polyline(vec![*a, *b, *c], true).map_err(|e| failed(&e))?;
                Body::planar(&[outline])
            }
            [a, b, c, d] => {
                // A ruled surface between the first side and the opposite one.
                let first = Curve::line(*a, *b).map_err(|e| failed(&e))?;
                let other = Curve::line(*d, *c).map_err(|e| failed(&e))?;
                Body::lofted(&[first, other], false)
            }
            _ => {
                let guides = points
                    .windows(2)
                    .filter_map(|pair| Curve::line(pair[0], pair[1]).ok())
                    .collect();
                return Ok(Made {
                    guides,
                    ..Made::default()
                });
            }
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.srfpt")),
            ..Made::default()
        })
    }
}

/// `ExtrudeSrf`, as in Rhino: pulls surfaces straight out of the plane being drawn on into
/// solids, the sides swept by their open edges; curved surfaces too. The distance follows the
/// mouse, or is typed; Enter uses the last one. `BothSides` goes as far each way; `DeleteInput`
/// takes the surfaces away.
#[derive(Default)]
pub struct ExtrudeSrf;

impl ExtrudeSrf {
    fn up(cx: &Context<'_>) -> Point {
        let [x, y] = cx.plane();
        curva_kernel::unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
    }
}

impl Recipe for ExtrudeSrf {
    const NAME: &'static str = "extrudesrf";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle("BothSides", false),
        OptionDef::toggle("DeleteInput", true),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudesrf.objects")
                .bodies()
                .offering(&[]),
        ];
        let base = match cx.values.first() {
            Some(Value::Objects(ids)) => bodies_of(cx.doc, ids)
                .first()
                .and_then(|(_, body)| body.bounds().ok())
                .map(|(low, high)| std::array::from_fn(|i| (low[i] + high[i]) / 2.0)),
            _ => None,
        };
        if let Some(base) = base {
            steps.push(
                StepDef::along("prompt.extrudecrv.distance", base, Self::up(cx))
                    .remembered("Distance", 10.0),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(distance)) = (cx.values.first(), number(cx, 1)) else {
            return Ok(Made::default());
        };
        if distance.abs() < 1e-9 {
            return Err(Message::new("error.bad_distance"));
        }
        let up = Self::up(cx);
        let (lift, length) = if cx.toggle("BothSides") {
            (-distance, 2.0 * distance)
        } else {
            (0.0, distance)
        };
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, ids) {
            if body.is_solid() {
                continue;
            }
            let start = body.translated(up.map(|v| v * lift));
            let solid = start
                .extruded_straight(up, length)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(solid),
                from: id,
                history: None,
            });
            if cx.toggle("DeleteInput") {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.extrudesrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `ArrayLinear`: copies objects in a row: how many, then two points for the spacing and way.
#[derive(Default)]
pub struct ArrayLinear;

impl Recipe for ArrayLinear {
    const NAME: &'static str = "arraylinear";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.arraylinear.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.arraylinear.count"));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::point("prompt.arraylinear.first"));
        }
        if let Some(a) = point(cx, 2) {
            steps.push(StepDef::point("prompt.arraylinear.second").from(a));
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(count) = number(cx, 1)
            && !(count >= 2.0 && count.fract() == 0.0 && count <= 10_000.0)
        {
            return Err(Message::new("error.bad_items"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(count), Some(a), Some(b)) =
            (cx.values.first(), number(cx, 1), point(cx, 2), point(cx, 3))
        else {
            return Ok(Made::default());
        };
        let step = sub(b, a);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let changes: Vec<Change> = (1..count as usize)
            .map(|i| Change::Move(step.map(|v| v * i as f64)))
            .collect();
        let mut made = changed(cx, ids, &changes, true, |_, _| None);
        made.message = Some(Message::with(
            "done.array",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `Angle`: the angle at a corner, from the corner and a point along each side.
#[derive(Default)]
pub struct Angle;

impl Recipe for Angle {
    const NAME: &'static str = "angle";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.angle.corner")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.angle.first").from(c));
            if cx.values.len() >= 2 {
                steps.push(StepDef::point("prompt.angle.second").from(c));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(a)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let mut guides: Vec<Curve> = Curve::line(c, a).ok().into_iter().collect();
        let Some(b) = point(cx, 2) else {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        };
        guides.extend(Curve::line(c, b).ok());
        let (u, v) = (sub(a, c), sub(b, c));
        let lengths = dot(u, u).sqrt() * dot(v, v).sqrt();
        if lengths < 1e-12 {
            return Err(Message::new("error.reference_point"));
        }
        let degrees = (dot(u, v) / lengths).clamp(-1.0, 1.0).acos().to_degrees();
        Ok(Made {
            guides,
            message: Some(Message::with("done.angle", vec![format!("{degrees:.4}")])),
            ..Made::default()
        })
    }
}

/// `EvaluatePt`: the coordinates of a point.
#[derive(Default)]
pub struct EvaluatePt;

impl Recipe for EvaluatePt {
    const NAME: &'static str = "evaluatept";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point("prompt.evaluatept.at")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(p) = point(cx, 0) else {
            return Ok(Made::default());
        };
        Ok(Made {
            message: Some(Message::with(
                "done.evaluatept",
                vec![format!("{:.4}, {:.4}, {:.4}", p[0], p[1], p[2])],
            )),
            ..Made::default()
        })
    }
}

/// `Radius`: the radius of a circle or an arc, clicked.
#[derive(Default)]
pub struct Radius;

impl Recipe for Radius {
    const NAME: &'static str = "radius";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::pick("prompt.radius.curve")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(at) = point(cx, 0) else {
            return Ok(Made::default());
        };
        let found = super::curve_at(cx.doc, at, crate::recipe::ON_CURVE).map_err(|e| failed(&e))?;
        match found {
            Some((_, Curve::Circle { radius, .. } | Curve::Arc { radius, .. })) => Ok(Made {
                message: Some(Message::with("done.radius", vec![format!("{radius:.4}")])),
                ..Made::default()
            }),
            _ => Err(Message::new("error.radius")),
        }
    }
}

/// `What`: what the chosen objects are, with their sizes.
#[derive(Default)]
pub struct What;

impl Recipe for What {
    const NAME: &'static str = "what";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.what.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let unit = cx.doc.state().unit.symbol();
        let described: Vec<String> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .map(|o| match &o.shape {
                Shape::Curve(curve) => {
                    let kind = match curve {
                        Curve::Point { at } => {
                            return format!("point at {:.3}, {:.3}, {:.3}", at[0], at[1], at[2]);
                        }
                        Curve::Line { .. } => "line",
                        Curve::Polyline { closed: true, .. } => "closed polyline",
                        Curve::Polyline { .. } => "polyline",
                        Curve::Circle { .. } => "circle",
                        Curve::Arc { .. } => "arc",
                        Curve::Nurbs { .. } => "control-point curve",
                        Curve::Compound { .. } => "joined curve",
                        Curve::Interpolated { .. } => "interpolated curve",
                    };
                    let length = curve
                        .length()
                        .map(|l| format!(", {l:.3} {unit} long"))
                        .unwrap_or_default();
                    format!("{kind}{length}")
                }
                Shape::Body(body) if body.is_solid() => {
                    let volume = body
                        .volume()
                        .map(|v| format!(", volume {v:.3} {unit}³"))
                        .unwrap_or_default();
                    format!("solid{volume}")
                }
                Shape::Body(body) => {
                    let area = body
                        .area()
                        .map(|a| format!(", area {a:.3} {unit}²"))
                        .unwrap_or_default();
                    format!("surface{area}")
                }
            })
            .collect();
        Ok(Made {
            message: Some(Message::with("done.what", vec![described.join("; ")])),
            ..Made::default()
        })
    }
}

/// Which curves a `Sel` command for one kind of curve picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurveKind {
    Line,
    Polyline,
    Circle,
    Arc,
    Closed,
    Open,
}

impl CurveKind {
    fn takes(self, curve: &Curve) -> bool {
        let closed = curve.is_closed().unwrap_or(false);
        match self {
            Self::Line => matches!(curve, Curve::Line { .. }),
            Self::Polyline => matches!(curve, Curve::Polyline { .. }),
            Self::Circle => matches!(curve, Curve::Circle { .. }),
            Self::Arc => matches!(curve, Curve::Arc { .. }),
            Self::Closed => !matches!(curve, Curve::Point { .. }) && closed,
            Self::Open => !matches!(curve, Curve::Point { .. }) && !closed,
        }
    }
}

/// Selects every selectable curve of one kind: `SelLine`, `SelPolyline`, `SelCircle`, `SelArc`,
/// `SelClosedCrv`, `SelOpenCrv`.
#[derive(Default)]
pub struct SelKind<const KIND: u8>;

impl<const KIND: u8> Command for SelKind<KIND> {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let kind = [
            CurveKind::Line,
            CurveKind::Polyline,
            CurveKind::Circle,
            CurveKind::Arc,
            CurveKind::Closed,
            CurveKind::Open,
        ][usize::from(KIND)];
        let open = doc.selectable();
        let found: Vec<u64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| open.contains(&o.id) && o.curve().is_some_and(|c| kind.takes(c)))
            .map(|o| o.id)
            .collect();
        doc.select(&found);
        Step::Done(Some(Message::with(
            "done.select",
            vec![found.len().to_string()],
        )))
    }
}

/// `SelLine`.
pub type SelLine = SelKind<0>;
/// `SelPolyline`.
pub type SelPolyline = SelKind<1>;
/// `SelCircle`.
pub type SelCircle = SelKind<2>;
/// `SelArc`.
pub type SelArc = SelKind<3>;
/// `SelClosedCrv`.
pub type SelClosedCrv = SelKind<4>;
/// `SelOpenCrv`.
pub type SelOpenCrv = SelKind<5>;

/// `HideSwap`: shows what was hidden and hides what was shown.
#[derive(Default)]
pub struct HideSwap;

impl Command for HideSwap {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let shown: Vec<u64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| !o.hidden)
            .map(|o| o.id)
            .collect();
        doc.show_all();
        doc.hide(&shown);
        Step::Done(Some(Message::with(
            "done.hideswap",
            vec![shown.len().to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn polygon_helix_and_spiral() {
        let mut doc = Document::default();
        run(&mut doc, &["polygon", "numsides", "6", "0,0", "10,0"]);
        let Some(Curve::Polyline {
            points,
            closed: true,
        }) = doc.state().objects[0].curve()
        else {
            panic!("no polygon");
        };
        assert_eq!(points.len(), 6);
        let mut doc = Document::default();
        run(&mut doc, &["helix", "0,0,0", "0,0,10", "3"]);
        let sample = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(sample.iter().all(|p| (p[0].hypot(p[1]) - 3.0).abs() < 0.2));
        let mut doc = Document::default();
        run(&mut doc, &["spiral", "flat", "0,0,0", "0,0,10", "1", "5"]);
        assert!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .sample()
                .unwrap()
                .iter()
                .all(|p| p[2].abs() < 1e-6)
        );
    }

    #[test]
    fn plane_srfpt_and_extrudesrf() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,5"]);
        let body = doc.state().objects[0].shape.body().unwrap().clone();
        assert!((body.area().unwrap() - 20.0).abs() < 1e-6);
        doc.select(&[doc.state().objects[0].id]);
        // Its preview shows the solid before the distance is given.
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extrudesrf");
        assert!(!processor.preview_bodies(&doc, None, "2").is_empty());
        processor.cancel(&mut doc);
        run(&mut doc, &["extrudesrf", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!((solid.volume().unwrap() - 40.0).abs() < 1e-3);
        let mut doc = Document::default();
        run(&mut doc, &["srfpt", "0,0", "4,0", "4,4", "0,4"]);
        assert!((doc.state().objects[0].shape.body().unwrap().area().unwrap() - 16.0).abs() < 1e-6);
    }

    #[test]
    fn extrudesrf_goes_straight_up_as_rhino_does_not_along_the_surface() {
        // A tilted square, 4 by 4 on the ground, rising 3 along y: pulled straight up by 2, its
        // volume is its footprint times 2, and the top is 2 above the highest corner.
        let mut doc = Document::default();
        run(&mut doc, &["srfpt", "0,0,0", "4,0,0", "4,4,3", "0,4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["extrudesrf", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!(
            (solid.volume().unwrap() - 32.0).abs() < 1e-3,
            "{}",
            solid.volume().unwrap()
        );
        let (low, high) = solid.bounds().unwrap();
        assert!(
            (high[2] - 5.0).abs() < 1e-6 && low[2].abs() < 1e-6,
            "{low:?} {high:?}"
        );
        // BothSides goes as far down as up.
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,5"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["extrudesrf", "BothSides", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!((solid.volume().unwrap() - 80.0).abs() < 1e-3);
    }

    #[test]
    fn measuring_and_selecting() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["angle", "0,0", "10,0", "0,10"]);
        assert!(processor.log().last().unwrap().contains("90"));
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::circle([5.0, 0.0, 0.0], 1.0).unwrap());
        run(&mut doc, &["selline"]);
        assert_eq!(doc.selection(), vec![line]);
        run(&mut doc, &["selclosedcrv"]);
        assert_eq!(doc.selection().len(), 1);
        assert_ne!(doc.selection(), vec![line]);
    }

    #[test]
    fn array_linear_makes_a_row() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        run(&mut doc, &["arraylinear", "4", "0,0", "0,5"]);
        assert_eq!(doc.state().objects.len(), 4);
    }
}
