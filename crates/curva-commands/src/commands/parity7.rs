//! The last commands that waited on the geometry engine (spec `docs/specs/engine-gaps.md`), on
//! Curva's flagged surface operations: `ExtractIsocurve`, `ShrinkTrimmedSrf`, `BlendSrf`,
//! `MatchSrf`, `Sweep2`, `Twist`, `Bend`, `Taper`, `Flow`, `CageEdit`, `Pull`, `InterpCrvOnSrf`,
//! `OffsetCrvOnSrf`, `Silhouette`, `UnrollSrf`, `ExtrudeFace`, `FilletSrf`, `ChamferSrf` and
//! `SplitSrf`.

use super::{bodies_of, curves_of};
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value, run_points};
use curva_document::Shape;
use curva_kernel::{Body, Curve, KernelError, Point, Spline, cross, dot, gap, sub, unit};

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn number(cx: &Context<'_>, index: usize) -> Option<f64> {
    cx.values.get(index).and_then(Value::number)
}

fn objects<'a>(cx: &'a Context<'_>, index: usize) -> &'a [u64] {
    match cx.values.get(index) {
        Some(Value::Objects(ids)) => ids,
        _ => &[],
    }
}

fn edge(cx: &Context<'_>, index: usize) -> Option<(u64, usize)> {
    match cx.values.get(index) {
        Some(Value::Edges(edges)) => edges.first().copied(),
        _ => None,
    }
}

fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn body_of(cx: &Context<'_>, id: u64) -> Option<Body> {
    bodies_of(cx.doc, &[id]).into_iter().next().map(|(_, b)| b)
}

fn first_body(cx: &Context<'_>, index: usize) -> Option<(u64, Body)> {
    bodies_of(cx.doc, objects(cx, index)).into_iter().next()
}

fn first_curve(cx: &Context<'_>, index: usize) -> Option<(u64, Curve)> {
    curves_of(cx.doc, objects(cx, index)).into_iter().next()
}

/// Every chosen object deformed by `map`: surfaces and solids face by face, curves by their
/// control points.
fn deform_all(
    cx: &Context<'_>,
    ids: &[u64],
    map: &dyn Fn(Point) -> Point,
    done: &'static str,
) -> Result<Made, Message> {
    let mut made = Made::default();
    for object in cx
        .doc
        .state()
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id))
    {
        let shape = match &object.shape {
            Shape::Curve(c) => Shape::Curve(c.deformed(map).map_err(|e| failed(&e))?),
            Shape::Body(b) => Shape::Body(b.deformed(map).map_err(|e| failed(&e))?),
            // A block is deformed by exploding it first, as in Rhino.
            Shape::Instance(_) => continue,
        };
        made.replace.push((object.id, shape));
    }
    made.message = Some(Message::with(done, vec![made.replace.len().to_string()]));
    Ok(made)
}

// ---------------------------------------------------------------------------------------------

/// `ExtractIsocurve`: copies a surface's curve through a point, along `U`, `V` or `Both` (the
/// default).
#[derive(Default)]
pub struct ExtractIsocurve;

impl Recipe for ExtractIsocurve {
    const NAME: &'static str = "extractisocurve";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("U"),
        OptionDef::mode("V"),
        OptionDef::mode("Both"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extractisocurve.surface")
                .bodies()
                .one(),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.extractisocurve.point"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((id, body)), Some(at)) = (first_body(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let (face, _, _) = body.nearest_face(at).map_err(|e| failed(&e))?;
        let ways: &[bool] = match cx.mode {
            Some("U") => &[false],
            Some("V") => &[true],
            _ => &[false, true],
        };
        let mut made = Made::default();
        for along_v in ways {
            let iso = body
                .isocurve_through(face, at, *along_v)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(iso),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extractisocurve",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `ShrinkTrimmedSrf`: cuts trimmed surfaces down to the box of their trims.
#[derive(Default)]
pub struct ShrinkTrimmedSrf;

impl Recipe for ShrinkTrimmedSrf {
    const NAME: &'static str = "shrinktrimmedsrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.shrinktrimmedsrf.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            made.replace
                .push((id, Shape::Body(body.shrunk().map_err(|e| failed(&e))?)));
        }
        made.message = Some(Message::with(
            "done.shrinktrimmedsrf",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

const CONTINUITY: [OptionDef; 4] = [
    OptionDef::mode("Position"),
    OptionDef::mode("Tangency"),
    OptionDef::mode("Curvature"),
    OptionDef::number("Bulge", 1.0),
];

fn continuity(cx: &Context<'_>, default: usize) -> usize {
    match cx.mode {
        Some("Position") => 0,
        Some("Tangency") => 1,
        Some("Curvature") => 2,
        _ => default,
    }
}

/// `BlendSrf`: a surface between two surface edges, meeting them in position, tangency or
/// curvature (the default); `Bulge` sets how far it leaves each edge.
#[derive(Default)]
pub struct BlendSrf;

impl Recipe for BlendSrf {
    const NAME: &'static str = "blendsrf";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.blendsrf.first").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::edges("prompt.blendsrf.second").one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((a_id, a_edge)), Some((b_id, b_edge))) = (edge(cx, 0), edge(cx, 1)) else {
            return Ok(Made::default());
        };
        let (Some(a), Some(b)) = (body_of(cx, a_id), body_of(cx, b_id)) else {
            return Ok(Made::default());
        };
        let blend = a
            .blend_to(a_edge, &b, b_edge, continuity(cx, 2), cx.number("Bulge"))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(blend),
                from: a_id,
                history: None,
            }],
            message: Some(Message::new("done.blendsrf")),
            ..Made::default()
        })
    }
}

/// `MatchSrf`: changes a surface along an edge to meet another surface's edge, in position or
/// tangency (the default).
#[derive(Default)]
pub struct MatchSrf;

impl Recipe for MatchSrf {
    const NAME: &'static str = "matchsrf";
    const OPTIONS: &'static [OptionDef] =
        &[OptionDef::mode("Position"), OptionDef::mode("Tangency")];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.matchsrf.change").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::edges("prompt.matchsrf.target").one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((a_id, a_edge)), Some((b_id, b_edge))) = (edge(cx, 0), edge(cx, 1)) else {
            return Ok(Made::default());
        };
        let (Some(a), Some(b)) = (body_of(cx, a_id), body_of(cx, b_id)) else {
            return Ok(Made::default());
        };
        let matched = a
            .matched_to(a_edge, &b, b_edge, continuity(cx, 1).min(1))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(a_id, Shape::Body(matched))],
            message: Some(Message::new("done.matchsrf")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Sweep2`: a profile swept along two rails, its ends on them.
#[derive(Default)]
pub struct Sweep2;

impl Recipe for Sweep2 {
    const NAME: &'static str = "sweep2";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.sweep2.first").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.sweep2.second").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::objects("prompt.sweep2.profile").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, a)), Some((_, b)), Some((id, profile))) =
            (first_curve(cx, 0), first_curve(cx, 1), first_curve(cx, 2))
        else {
            return Ok(Made::default());
        };
        let body = Body::swept_two_rails(&a, &b, &profile).map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.sweep2")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// Steps shared by the axis deformations: objects, the axis's start and end.
fn axis_steps(
    cx: &Context<'_>,
    objects_prompt: &'static str,
    last: Option<StepDef>,
) -> Vec<StepDef> {
    let mut steps = vec![StepDef::objects(objects_prompt)];
    if !cx.values.is_empty() {
        steps.push(StepDef::point("prompt.deform.axis_start"));
    }
    if let Some(a) = point(cx, 1) {
        steps.push(StepDef::point("prompt.deform.axis_end").from(a));
    }
    if cx.values.len() >= 3
        && let Some(last) = last
    {
        steps.push(last);
    }
    steps
}

/// Where a point sits along an axis: from 0 at its start to 1 at its end, clamped, and the
/// pieces of the point along and around it.
fn along_axis(p: Point, start: Point, end: Point) -> (f64, Point, Point) {
    let axis = sub(end, start);
    let length = dot(axis, axis).sqrt().max(1e-12);
    let e = axis.map(|v| v / length);
    let d = sub(p, start);
    let s = dot(d, e);
    let foot = plus(start, e, s);
    ((s / length).clamp(0.0, 1.0), foot, sub(p, foot))
}

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

/// `Taper`: narrows or widens objects along an axis: the axis, then how much bigger they are at
/// its end (1 keeps them).
#[derive(Default)]
pub struct Taper;

impl Recipe for Taper {
    const NAME: &'static str = "taper";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        axis_steps(
            cx,
            "prompt.taper.objects",
            Some(StepDef::number("prompt.taper.factor").remembered("Factor", 0.5)),
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(factor)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if factor <= 0.0 {
            return Err(Message::new("error.taper_factor"));
        }
        let map = move |p: Point| {
            let (t, foot, r) = along_axis(p, a, b);
            plus(foot, r, 1.0 + (factor - 1.0) * t)
        };
        deform_all(cx, objects(cx, 0), &map, "done.taper")
    }
}

/// `Bend`: bends objects along a spine: its start and end, then a point the bend passes
/// through.
#[derive(Default)]
pub struct Bend;

impl Recipe for Bend {
    const NAME: &'static str = "bend";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        axis_steps(
            cx,
            "prompt.bend.objects",
            Some(StepDef::point("prompt.bend.through")),
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(start), Some(end), Some(through)) = (point(cx, 1), point(cx, 2), point(cx, 3))
        else {
            return Ok(Made::default());
        };
        let e = unit(sub(end, start)).ok_or_else(|| Message::new("error.reference_point"))?;
        let to = sub(through, start);
        let side = sub(to, e.map(|v| v * dot(to, e)));
        let n = unit(side).ok_or_else(|| Message::new("error.bend_straight"))?;
        // The circle tangent to the spine at its start, through the bend point.
        let radius = dot(to, to) / (2.0 * dot(to, n));
        let length = gap(start, end);
        let centre = plus(start, n, radius);
        let map = move |p: Point| {
            let d = sub(p, start);
            let (s, off) = (dot(d, e), dot(d, n));
            let rest = sub(d, plus(e.map(|v| v * s), n, off));
            let s_in = s.clamp(0.0, length);
            let theta = s_in / radius;
            let on_arc = plus(
                plus(centre, n, -radius * theta.cos()),
                e,
                radius * theta.sin(),
            );
            let inward = unit(sub(centre, on_arc)).unwrap_or(n);
            let tangent: Point = std::array::from_fn(|i| e[i] * theta.cos() + n[i] * theta.sin());
            let bent = plus(plus(on_arc, inward, off), tangent, s - s_in);
            plus(bent, rest, 1.0)
        };
        deform_all(cx, objects(cx, 0), &map, "done.bend")
    }
}

/// `Flow`: moves objects from along one curve to along another, keeping where they sit along
/// and beside it.
#[derive(Default)]
pub struct Flow;

impl Recipe for Flow {
    const NAME: &'static str = "flow";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.flow.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.flow.base").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::objects("prompt.flow.target").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, base)), Some((_, target))) = (first_curve(cx, 1), first_curve(cx, 2)) else {
            return Ok(Made::default());
        };
        let (base, target) = (
            Spline::of(&base).map_err(|e| failed(&e))?,
            Spline::of(&target).map_err(|e| failed(&e))?,
        );
        let normal = up(cx);
        let frame = |s: &Spline, t: f64| -> (Point, Point, Point, Point) {
            let p = s.point(t);
            let tangent = unit(s.derivative(t)).unwrap_or([1.0, 0.0, 0.0]);
            let side = unit(cross(normal, tangent)).unwrap_or([0.0, 1.0, 0.0]);
            (p, tangent, side, cross(tangent, side))
        };
        let (b0, b1) = base.domain();
        let (t0, t1) = target.domain();
        let map = move |p: Point| {
            let t = base.closest(p);
            let (bp, bt, bs, bu) = frame(&base, t);
            let d = sub(p, bp);
            let (along, beside, above) = (dot(d, bt), dot(d, bs), dot(d, bu));
            let f = (t - b0) / (b1 - b0);
            let (tp, tt, ts, tu) = frame(&target, t0 + (t1 - t0) * f);
            plus(plus(plus(tp, tt, along), ts, beside), tu, above)
        };
        deform_all(cx, objects(cx, 0), &map, "done.flow")
    }
}

/// `CageEdit`: moves the corners of a box around objects, the objects following inside it:
/// pick a corner (the nearest is taken) and where it goes, as often as wanted; Enter applies it.
#[derive(Default)]
pub struct CageEdit;

impl Recipe for CageEdit {
    const NAME: &'static str = "cageedit";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.cageedit.objects")];
        if cx.values.is_empty() {
            return steps;
        }
        let clicks = &cx.values[1..];
        let prompt = |i: usize| {
            if i.is_multiple_of(2) {
                "prompt.cageedit.corner"
            } else {
                "prompt.cageedit.to"
            }
        };
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point(prompt(i));
            steps.push(if i.is_multiple_of(2) && i >= 2 {
                step.optional()
            } else {
                step
            });
        }
        if clicks.last() != Some(&Value::Empty) {
            let i = clicks.len();
            let mut step = StepDef::point(prompt(i));
            if let Some(Value::Point(last)) = clicks.last() {
                step = step.from(*last);
            }
            steps.push(if i.is_multiple_of(2) && i >= 2 {
                step.optional()
            } else {
                step
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        // The box around the objects.
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
        {
            let points: Vec<Point> = match &object.shape {
                Shape::Curve(c) => c.sample().unwrap_or_default(),
                Shape::Body(b) => b.bounds().map(|(l, h)| vec![l, h]).unwrap_or_default(),
                Shape::Instance(_) => Vec::new(),
            };
            for p in points {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        if low[0] > high[0] {
            return Ok(Made::default());
        }
        let size: Point = std::array::from_fn(|k| (high[k] - low[k]).max(1e-9));
        let corner_at = |c: usize| -> Point {
            std::array::from_fn(|k| if c >> k & 1 == 1 { high[k] } else { low[k] })
        };
        let mut corners: Vec<Point> = (0..8).map(corner_at).collect();
        let clicks = run_points(cx);
        for pair in clicks.chunks(2).filter(|p| p.len() == 2) {
            let nearest = (0..8)
                .min_by(|a, b| gap(corner_at(*a), pair[0]).total_cmp(&gap(corner_at(*b), pair[0])))
                .unwrap_or(0);
            corners[nearest] = pair[1];
        }
        let map = move |p: Point| {
            let t: Point = std::array::from_fn(|k| (p[k] - low[k]) / size[k]);
            let mut out = [0.0; 3];
            for (c, corner) in corners.iter().enumerate() {
                let w: f64 = (0..3)
                    .map(|k| if c >> k & 1 == 1 { t[k] } else { 1.0 - t[k] })
                    .product();
                for k in 0..3 {
                    out[k] += w * corner[k];
                }
            }
            out
        };
        if cx.values.last() != Some(&Value::Empty) && cx.previewing {
            let guides = (0..8)
                .flat_map(|c| {
                    (0..3)
                        .filter(move |k| c >> k & 1 == 0)
                        .map(move |k| (c, c | 1 << k))
                })
                .filter_map(|(a, b)| Curve::line(map(corner_at(a)), map(corner_at(b))).ok())
                .collect();
            return Ok(Made {
                guides,
                ..Made::default()
            });
        }
        deform_all(cx, ids, &map, "done.cageedit")
    }
}

// ---------------------------------------------------------------------------------------------

/// `Pull`: curves pulled onto surfaces, to their nearest points.
#[derive(Default)]
pub struct Pull;

impl Recipe for Pull {
    const NAME: &'static str = "pull";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.pull.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.pull.onto").bodies().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 1) else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (_, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let pulled = body.pulled(&curve).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(pulled),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.pull",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `InterpCrvOnSrf`: a curve through points on a surface, lying on it.
#[derive(Default)]
pub struct InterpCrvOnSrf;

impl Recipe for InterpCrvOnSrf {
    const NAME: &'static str = "interpcrvonsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.interpcrvonsrf.surface")
                .bodies()
                .one(),
        ];
        if cx.values.is_empty() {
            return steps;
        }
        let clicks = &cx.values[1..];
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point("prompt.interpcrvonsrf.point");
            steps.push(if i >= 2 { step.optional() } else { step });
        }
        if clicks.last() != Some(&Value::Empty) {
            let step = StepDef::point("prompt.interpcrvonsrf.point");
            steps.push(if clicks.len() >= 2 {
                step.optional()
            } else {
                step
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 0) else {
            return Ok(Made::default());
        };
        let points = run_points(cx);
        if points.len() < 2 {
            return Ok(Made::default());
        }
        let (face, _, _) = body.nearest_face(points[0]).map_err(|e| failed(&e))?;
        let curve = body.curve_on_face(face, &points).map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Curve(curve),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.interpcrvonsrf")),
            ..Made::default()
        })
    }
}

/// `OffsetCrvOnSrf`: a curve on a surface moved along it by a distance, to the side of a point.
#[derive(Default)]
pub struct OffsetCrvOnSrf;

impl Recipe for OffsetCrvOnSrf {
    const NAME: &'static str = "offsetcrvonsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.offsetcrvonsrf.curve")
                .curves()
                .one(),
        ];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.offsetcrvonsrf.surface")
                    .bodies()
                    .one(),
            );
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.offset.distance").remembered("Distance", 1.0));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::point("prompt.offset.side"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, curve)), Some((id, body)), Some(distance), Some(side)) = (
            first_curve(cx, 0),
            first_body(cx, 1),
            number(cx, 2),
            point(cx, 3),
        ) else {
            return Ok(Made::default());
        };
        let moved = body
            .offset_on_surface(&curve, distance, side)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Curve(moved),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.offsetcrvonsrf")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Silhouette`: the outlines of surfaces and solids seen straight down on the construction plane.
#[derive(Default)]
pub struct Silhouette;

impl Recipe for Silhouette {
    const NAME: &'static str = "silhouette";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.silhouette.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let view = up(cx).map(|v| -v);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            for curve in body.silhouette(view).map_err(|e| failed(&e))? {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.silhouette",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `UnrollSrf`: lays developable surfaces flat on the world XY plane.
#[derive(Default)]
pub struct UnrollSrf;

impl Recipe for UnrollSrf {
    const NAME: &'static str = "unrollsrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.unrollsrf.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let flat = body.unrolled().map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(flat),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.unrollsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `ExtrudeFace`: pulls a flat face of a solid out (or in, for a negative distance) and joins it.
#[derive(Default)]
pub struct ExtrudeFace;

impl Recipe for ExtrudeFace {
    const NAME: &'static str = "extrudeface";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::faces("prompt.extrudeface.face").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.extrudecrv.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Faces(picked)), Some(distance)) = (cx.values.first(), number(cx, 1))
        else {
            return Ok(Made::default());
        };
        let Some((id, face)) = picked.first().copied() else {
            return Ok(Made::default());
        };
        let Some(body) = body_of(cx, id) else {
            return Ok(Made::default());
        };
        let grown = body.face_extruded(face, distance).map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(id, Shape::Body(grown))],
            message: Some(Message::new("done.extrudeface")),
            ..Made::default()
        })
    }
}

/// `FilletSrf` (or, with `CHAMFER`, `ChamferSrf`): a fillet (or chamfer) surface of a radius
/// between two surfaces, on the sides facing each other.
#[derive(Default)]
pub struct FilletSrf<const CHAMFER: bool>;

impl<const CHAMFER: bool> Recipe for FilletSrf<CHAMFER> {
    const NAME: &'static str = if CHAMFER { "chamfersrf" } else { "filletsrf" };

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.filletsrf.first").bodies().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.filletsrf.second").bodies().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.filletsrf.radius").remembered("Radius", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((id, a)), Some((_, b)), Some(radius)) =
            (first_body(cx, 0), first_body(cx, 1), number(cx, 2))
        else {
            return Ok(Made::default());
        };
        if radius <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let made = a
            .filleted_with(&b, radius, CHAMFER)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(made),
                from: id,
                history: None,
            }],
            message: Some(Message::new(if CHAMFER {
                "done.chamfersrf"
            } else {
                "done.filletsrf"
            })),
            ..Made::default()
        })
    }
}

/// `SplitSrf`: cuts a surface with curves lying on it.
#[derive(Default)]
pub struct SplitSrf;

impl Recipe for SplitSrf {
    const NAME: &'static str = "splitsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.splitsrf.surface").bodies().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.splitsrf.curves").curves());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((id, body)) = first_body(cx, 0) else {
            return Ok(Made::default());
        };
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 1))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        let faces = body.face_count().map_err(|e| failed(&e))?;
        let mut cut = body.clone();
        let mut any = false;
        for face in 0..faces {
            if let Ok(next) = cut.split_by_curves(face, &curves) {
                cut = next;
                any = true;
                break;
            }
        }
        if !any {
            return Err(Message::new("error.splitsrf_missed"));
        }
        // Each piece its own surface.
        let mut made = Made::default();
        let count = cut.face_count().map_err(|e| failed(&e))?;
        for face in 0..count {
            let piece = cut.faces_only(&[face]).map_err(|e| failed(&e))?;
            if face == 0 {
                made.replace.push((id, Shape::Body(piece)));
            } else {
                made.derived.push(Derived {
                    shape: Shape::Body(piece),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with("done.splitsrf", vec![count.to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn sheet(doc: &mut Document, y0: f64, y1: f64, z: f64) -> u64 {
        doc.add_body(
            Body::ruled(
                &line([0.0, y0, z], [4.0, y0, z]),
                &line([0.0, y1, z], [4.0, y1, z]),
                false,
            )
            .unwrap(),
        )
    }

    fn edge_along_y(doc: &Document, id: u64, y: f64) -> (u64, usize) {
        let body = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .shape
            .body()
            .unwrap()
            .clone();
        let edge = (0..body.record_edge_count())
            .find(|e| {
                body.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - y).abs() < 1e-9)
            })
            .unwrap();
        (id, edge)
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
    fn isocurves_shrink_and_split() {
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 0.0);
        doc.select(&[s]);
        run(&mut doc, &["extractisocurve", "U", "1,2"]);
        assert!((last_curve(&doc).length().unwrap() - 4.0).abs() < 1e-6);
        let mut doc = Document::default();
        let disc =
            doc.add_body(Body::planar(&[Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap()]).unwrap());
        doc.select(&[disc]);
        run(&mut doc, &["shrinktrimmedsrf"]);
        assert!((last_body(&doc).area().unwrap() - std::f64::consts::PI).abs() < 1e-3);
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 0.0);
        let cut = doc.add(line([2.0, -1.0, 0.0], [2.0, 5.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[s]);
        processor.submit(&mut doc, "splitsrf");
        doc.select(&[cut]);
        processor.submit(&mut doc, "");
        let pieces = doc
            .state()
            .objects
            .iter()
            .filter(|o| o.shape.body().is_some())
            .count();
        assert_eq!(pieces, 2);
    }

    #[test]
    fn blend_and_match_surfaces() {
        let mut doc = Document::default();
        let a = sheet(&mut doc, 0.0, 4.0, 0.0);
        let b = sheet(&mut doc, 7.0, 11.0, 3.0);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "blendsrf");
        doc.select_edges(&[edge_along_y(&doc, a, 4.0)]);
        processor.submit(&mut doc, "");
        doc.select_edges(&[edge_along_y(&doc, b, 7.0)]);
        processor.submit(&mut doc, "");
        assert_eq!(doc.state().objects.len(), 3, "{:?}", processor.log());
        let mut doc = Document::default();
        let a = sheet(&mut doc, 0.0, 4.0, 0.0);
        let b = sheet(&mut doc, 4.5, 8.5, 1.0);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "matchsrf");
        doc.select_edges(&[edge_along_y(&doc, a, 4.0)]);
        processor.submit(&mut doc, "");
        doc.select_edges(&[edge_along_y(&doc, b, 4.5)]);
        processor.submit(&mut doc, "");
        let changed = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((changed.1[1] - 4.5).abs() < 1e-6, "{changed:?}");
    }

    #[test]
    fn sweep_along_two_rails() {
        let mut doc = Document::default();
        let a = doc.add(line([0.0; 3], [0.0, 10.0, 0.0]));
        let b = doc.add(line([4.0, 0.0, 0.0], [8.0, 10.0, 0.0]));
        let p = doc.add(line([0.0; 3], [4.0, 0.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "sweep2");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        doc.select(&[p]);
        processor.submit(&mut doc, "");
        // A flat quadrilateral: (4 + 8) / 2 × 10.
        assert!(
            (last_body(&doc).area().unwrap() - 60.0).abs() < 0.1,
            "{}",
            last_body(&doc).area().unwrap()
        );
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

    #[test]
    fn curves_on_surfaces() {
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 2.0);
        let wire = doc.add(line([1.0, 1.0, 5.0], [3.0, 3.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[wire]);
        processor.submit(&mut doc, "pull");
        doc.select(&[s]);
        processor.submit(&mut doc, "");
        assert!(
            last_curve(&doc)
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        doc.select(&[s]);
        run(&mut doc, &["interpcrvonsrf", "1,1,9", "2,3,9", "3,1,9", ""]);
        assert!(
            last_curve(&doc)
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let base = doc.add(line([1.0, 1.0, 2.0], [3.0, 1.0, 2.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[base]);
        processor.submit(&mut doc, "offsetcrvonsrf");
        doc.select(&[s]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "1");
        processor.submit(&mut doc, "2,4,2");
        let p = last_curve(&doc).sample().unwrap()[0];
        assert!((p[1] - 2.0).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn silhouettes_unrolling_extruding_faces_and_fillets() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap());
        doc.select(&[cube]);
        run(&mut doc, &["silhouette"]);
        assert!(doc.state().objects.len() > 1);
        let mut doc = Document::default();
        let low = Curve::arc(
            [0.0; 3],
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let high = Curve::arc(
            [0.0, 0.0, 3.0],
            [2.0, 0.0, 3.0],
            [0.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let strip = doc.add_body(Body::ruled(&low, &high, false).unwrap());
        doc.select(&[strip]);
        run(&mut doc, &["unrollsrf"]);
        let flat = last_body(&doc);
        assert!(flat.bounds().unwrap().1[2].abs() < 1e-9);
        let mut doc = Document::default();
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let mesh = cube.mesh().unwrap();
        let top = mesh
            .triangle_faces
            .iter()
            .zip(&mesh.triangles)
            .find(|(_, t)| {
                t.iter()
                    .all(|i| (mesh.positions[*i as usize][2] - 1.0).abs() < 1e-9)
            })
            .map(|(f, _)| *f)
            .unwrap();
        let id = doc.add_body(cube);
        doc.select_faces(&[(id, top)]);
        run(&mut doc, &["extrudeface", "", "0.5"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        let mut doc = Document::default();
        let floor = doc.add_body(
            Body::ruled(
                &line([0.0; 3], [10.0, 0.0, 0.0]),
                &line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]),
                false,
            )
            .unwrap(),
        );
        let wall = doc.add_body(
            Body::ruled(
                &line([0.0; 3], [10.0, 0.0, 0.0]),
                &line([0.0, 0.0, 10.0], [10.0, 0.0, 10.0]),
                false,
            )
            .unwrap(),
        );
        let mut processor = crate::Processor::default();
        doc.select(&[floor]);
        processor.submit(&mut doc, "filletsrf");
        doc.select(&[wall]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "2");
        let area = last_body(&doc).area().unwrap();
        assert!(
            (area - std::f64::consts::PI * 10.0).abs() < 0.02 * std::f64::consts::PI * 10.0,
            "{area}"
        );
    }
}
