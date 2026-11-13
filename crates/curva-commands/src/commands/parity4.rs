//! The curve commands still missing for Rhino parity (spec `docs/specs/rhino-parity-0.4.1.md`):
//! `HandleCurve`, `Sketch`, `Conic`, `Hyperbola`, `BlendCrv`, `OffsetMultiple`, `DupBorder`,
//! `Extend`, `Fair`, `CurveBoolean`, `Match`, `SimplifyCrv`, `Reparameterize`, `MakeUniform` and
//! `Weight`.

use super::{bodies_of, curves_of};
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value, run_points};
use curva_document::Shape;
use curva_kernel::{Curve, KernelError, Point, Spline, cross, dot, gap, sub, unit};

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

fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn scaled(d: Point, by: f64) -> Point {
    d.map(|v| v * by)
}

fn length(d: Point) -> f64 {
    dot(d, d).sqrt()
}

/// The normal of the plane being drawn on.
fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}

/// A Bézier piece as a clamped spline.
fn bezier(points: &[Point], weights: Option<&[f64]>) -> Spline {
    let degree = points.len() - 1;
    let mut knots = vec![0.0; degree + 1];
    knots.extend(std::iter::repeat_n(1.0, degree + 1));
    Spline {
        degree,
        knots,
        points: points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let w = weights.map_or(1.0, |w| w[i]);
                [p[0] * w, p[1] * w, p[2] * w, w]
            })
            .collect(),
    }
}

/// The steps of a run of points ended by Enter, each prompt chosen by its place in the run.
fn run_steps(
    cx: &Context<'_>,
    prompt: impl Fn(usize) -> &'static str,
    enough: impl Fn(usize) -> bool,
) -> Vec<StepDef> {
    let mut steps: Vec<StepDef> = (0..cx.values.len())
        .map(|i| StepDef::point(prompt(i)))
        .collect();
    if cx.values.last() == Some(&Value::Empty) {
        return steps;
    }
    let placed = run_points(cx);
    let mut step = StepDef::point(prompt(cx.values.len()));
    if let Some(last) = placed.last() {
        step = step.from(*last);
    }
    if enough(placed.len()) {
        step = step.optional();
    }
    steps.push(step);
    steps
}

// ---------------------------------------------------------------------------------------------

/// `HandleCurve`: a curve by points with handles, as a pen tool draws: click a point, then where
/// its handle goes; the curve leaves each point towards its handle and arrives from the mirror of
/// it. Enter ends it.
#[derive(Default)]
pub struct HandleCurve;

impl Recipe for HandleCurve {
    const NAME: &'static str = "handlecurve";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        run_steps(
            cx,
            |i| {
                if i % 2 == 0 {
                    "prompt.handlecurve.point"
                } else {
                    "prompt.handlecurve.handle"
                }
            },
            |placed| placed >= 4 && placed % 2 == 0,
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let placed = run_points(cx);
        // Pairs of a point and its handle; a point still without one uses itself.
        let pairs: Vec<(Point, Point)> = placed
            .chunks(2)
            .map(|pair| (pair[0], pair.get(1).copied().unwrap_or(pair[0])))
            .collect();
        let guides: Vec<Curve> = pairs
            .iter()
            .filter_map(|(at, handle)| {
                let mirror = plus(*at, sub(*at, *handle), 1.0);
                Curve::line(mirror, *handle).ok()
            })
            .collect();
        if pairs.len() < 2 {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        }
        let pieces: Vec<Spline> = pairs
            .windows(2)
            .map(|w| {
                let ((a, ha), (b, hb)) = (w[0], w[1]);
                let arrive = plus(b, sub(b, hb), 1.0);
                bezier(&[a, ha, arrive, b], None)
            })
            .collect();
        let curve = Spline::joined(&pieces)
            .and_then(|s| s.to_curve())
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            guides,
            message: Some(Message::new("done.handlecurve")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Sketch`: a freehand curve. Hold the mouse button and draw (the window feeds the points as
/// the pointer moves), or click points; Enter or letting go ends it. The points are thinned and
/// a smooth curve is passed through them.
#[derive(Default)]
pub struct Sketch;

impl Recipe for Sketch {
    const NAME: &'static str = "sketch";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        run_steps(cx, |_| "prompt.sketch.point", |placed| placed >= 2)
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let placed = run_points(cx);
        if placed.len() < 2 {
            return Ok(Made::default());
        }
        let total: f64 = placed.windows(2).map(|w| gap(w[0], w[1])).sum();
        // Keep a point every fiftieth of the length, and the last.
        let step = total / 50.0;
        let mut kept = vec![placed[0]];
        for p in &placed[1..] {
            if gap(*p, *kept.last().unwrap_or(&placed[0])) >= step {
                kept.push(*p);
            }
        }
        let last = placed[placed.len() - 1];
        if gap(last, *kept.last().unwrap_or(&last)) > 1e-9 {
            kept.push(last);
        }
        let curve = if kept.len() == 2 {
            Curve::line(kept[0], kept[1])
        } else {
            Curve::interpolated(kept, false)
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.sketch")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Conic`: a conic section from its start, its end, the apex where its end tangents meet, and
/// its rho (0 to 1: under 0.5 an ellipse's arc, 0.5 a parabola, over 0.5 a hyperbola). Exact,
/// as a weighted quadratic.
#[derive(Default)]
pub struct Conic;

impl Recipe for Conic {
    const NAME: &'static str = "conic";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.conic.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.conic.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::point("prompt.conic.apex").from(b));
                if point(cx, 2).is_some() {
                    steps.push(StepDef::number("prompt.conic.rho").remembered("Rho", 0.5));
                }
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        match number(cx, 3) {
            Some(rho) if !(rho > 0.0 && rho < 1.0) => Err(Message::new("error.conic_rho")),
            _ => Ok(()),
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(apex) = point(cx, 2) else {
            return Ok(Made {
                guides: Curve::line(a, b).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let rho = number(cx, 3).unwrap_or(0.5).clamp(1e-6, 1.0 - 1e-6);
        let weight = rho / (1.0 - rho);
        let curve = bezier(&[a, apex, b], Some(&[1.0, weight, 1.0]))
            .to_curve()
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            guides: [Curve::line(a, apex), Curve::line(apex, b)]
                .into_iter()
                .filter_map(Result::ok)
                .collect(),
            message: Some(Message::new("done.conic")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Hyperbola`: one branch of a hyperbola, exact: its centre, the vertex the branch passes
/// through, the focus beyond the vertex, then how far the branch reaches.
#[derive(Default)]
pub struct Hyperbola;

impl Recipe for Hyperbola {
    const NAME: &'static str = "hyperbola";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.hyperbola.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.hyperbola.vertex").from(c));
            if let Some(v) = point(cx, 1) {
                steps.push(StepDef::along("prompt.hyperbola.focus", c, sub(v, c)));
                if number(cx, 2).is_some() {
                    steps.push(StepDef::point("prompt.hyperbola.end").from(v));
                }
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(v)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let a = gap(c, v);
        let axis = unit(sub(v, c)).ok_or_else(|| Message::new("error.reference_point"))?;
        let Some(focal) = number(cx, 2) else {
            return Ok(Made {
                guides: Curve::line(c, v).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        if focal.abs() <= a {
            return Err(Message::new("error.hyperbola_focus"));
        }
        let b = (focal * focal - a * a).sqrt();
        let side =
            unit(cross(up(cx), axis)).ok_or_else(|| Message::new("error.reference_point"))?;
        // How far the branch reaches across its axis: at least a little.
        let reach = point(cx, 3)
            .map(|end| dot(sub(end, c), side).abs())
            .unwrap_or(b)
            .max(1e-6 * a);
        let x_end = a * (1.0 + reach * reach / (b * b)).sqrt();
        let at = |x: f64, y: f64| plus(plus(c, axis, x), side, y);
        // The end tangents meet on the axis at a² / x; the vertex is the curve's middle, which
        // makes the middle weight x / a.
        let curve = bezier(
            &[at(x_end, -reach), at(a * a / x_end, 0.0), at(x_end, reach)],
            Some(&[1.0, x_end / a, 1.0]),
        )
        .to_curve()
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.hyperbola")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// Which end of `a` and which end of `b` are nearest each other: `(a at its end, b at its end)`.
fn nearest_ends(a: &Spline, b: &Spline) -> (bool, bool) {
    let (a0, a1) = (a.point(a.domain().0), a.point(a.domain().1));
    let (b0, b1) = (b.point(b.domain().0), b.point(b.domain().1));
    [
        (gap(a1, b0), (true, false)),
        (gap(a1, b1), (true, true)),
        (gap(a0, b0), (false, false)),
        (gap(a0, b1), (false, true)),
    ]
    .into_iter()
    .min_by(|x, y| x.0.total_cmp(&y.0))
    .map_or((true, false), |(_, ends)| ends)
}

/// The point and the first two derivatives at one end of a curve, pointing away from it: going
/// out of the curve past that end.
fn leaving(curve: &Spline, at_end: bool) -> (Point, Point, Point) {
    if at_end {
        curve.end_derivatives()
    } else {
        curve.reversed().end_derivatives()
    }
}

/// How a blend or match joins: in position only, along the tangent too, or in curvature as well.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Continuity {
    Position,
    Tangency,
    Curvature,
}

fn continuity(cx: &Context<'_>, default: Continuity) -> Continuity {
    match cx.mode {
        Some("Position") => Continuity::Position,
        Some("Tangency") => Continuity::Tangency,
        Some("Curvature") => Continuity::Curvature,
        _ => default,
    }
}

const CONTINUITY: [OptionDef; 3] = [
    OptionDef::mode("Position"),
    OptionDef::mode("Tangency"),
    OptionDef::mode("Curvature"),
];

/// `BlendCrv`: a curve joining the nearest ends of two curves, meeting them in position, along
/// their tangents, or in curvature (the default), as Rhino's BlendCrv.
#[derive(Default)]
pub struct BlendCrv;

impl Recipe for BlendCrv {
    const NAME: &'static str = "blendcrv";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.blendcrv.first").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.blendcrv.second").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (first, second) = (
            curves_of(cx.doc, objects(cx, 0)),
            curves_of(cx.doc, objects(cx, 1)),
        );
        let (Some((_, a)), Some((_, b))) = (first.first(), second.first()) else {
            return Ok(Made::default());
        };
        let (a, b) = (
            Spline::of(a).map_err(|e| failed(&e))?,
            Spline::of(b).map_err(|e| failed(&e))?,
        );
        let (a_end, b_end) = nearest_ends(&a, &b);
        let (p0, d0, dd0) = leaving(&a, a_end);
        let (p3, d3, dd3) = leaving(&b, b_end);
        let chord = gap(p0, p3);
        if chord <= 1e-9 {
            return Err(Message::new("error.blend_touching"));
        }
        let curve = match continuity(cx, Continuity::Curvature) {
            Continuity::Position => Curve::line(p0, p3),
            Continuity::Tangency => {
                let (t0, t3) = (unit(d0).unwrap_or([0.0; 3]), unit(d3).unwrap_or([0.0; 3]));
                bezier(
                    &[p0, plus(p0, t0, chord / 3.0), plus(p3, t3, chord / 3.0), p3],
                    None,
                )
                .to_curve()
            }
            Continuity::Curvature => {
                // A quintic whose first and second derivatives at each end are those of the
                // curve it meets, scaled so the blend runs about as fast as its chord.
                let (s0, s3) = (chord / length(d0).max(1e-12), chord / length(d3).max(1e-12));
                let q1 = plus(p0, d0, s0 / 5.0);
                let q2: Point =
                    std::array::from_fn(|k| 2.0 * q1[k] - p0[k] + s0 * s0 * dd0[k] / 20.0);
                // At the far end the blend arrives, so its derivative is the other curve's leaving
                // derivative turned round, and its second derivative is unchanged.
                let q4 = plus(p3, d3, s3 / 5.0);
                let q3: Point =
                    std::array::from_fn(|k| 2.0 * q4[k] - p3[k] + s3 * s3 * dd3[k] / 20.0);
                bezier(&[p0, q1, q2, q3, q4, p3], None).to_curve()
            }
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.blendcrv")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `OffsetMultiple`: offsets several curves at once by one distance, each to the side of a
/// point.
#[derive(Default)]
pub struct OffsetMultiple;

impl Recipe for OffsetMultiple {
    const NAME: &'static str = "offsetmultiple";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.offsetmultiple.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.offset.distance").remembered("Distance", 1.0));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::point("prompt.offsetmultiple.side"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(distance), Some(side)) = (number(cx, 1), point(cx, 2)) else {
            return Ok(Made::default());
        };
        if distance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let normal = up(cx);
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let moved = curve
                .offset(normal, distance, side)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(moved),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.offsetmultiple",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `DupBorder`: copies the open border of surfaces and polysurfaces as curves, joined where they
/// meet.
#[derive(Default)]
pub struct DupBorder;

impl Recipe for DupBorder {
    const NAME: &'static str = "dupborder";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.dupborder.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let edges = body.naked_edges().map_err(|e| failed(&e))?;
            let curves = edges
                .into_iter()
                .map(|edge| body.edge_curve(edge))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| failed(&e))?;
            let joined = curva_kernel::join(&curves).map_err(|e| failed(&e))?;
            for (_, curve) in joined {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupborder",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// How Extend carries a curve on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Line,
    Arc,
    Smooth,
}

/// The curve carried on past its end by about `by` (a length), and the added piece alone.
fn extended(curve: &Curve, by: f64, reach: Reach) -> Result<(Curve, Curve), KernelError> {
    let spline = Spline::of(curve)?;
    let (end, d1, d2) = spline.end_derivatives();
    let t =
        unit(d1).ok_or_else(|| KernelError("the curve has no direction at its end".to_owned()))?;
    let line = || Curve::line(end, plus(end, t, by));
    let piece = match reach {
        Reach::Line => line()?,
        Reach::Arc => {
            let speed = length(d1);
            let bend = cross(d1, d2);
            let curvature = length(bend) / speed.powi(3);
            if curvature < 1e-9 {
                line()?
            } else {
                let radius = 1.0 / curvature;
                let inward = unit(cross(bend, d1)).unwrap_or([0.0; 3]);
                let centre = plus(end, inward, radius);
                let x = unit(sub(end, centre)).unwrap_or([1.0, 0.0, 0.0]);
                let angle = (by / radius).min(std::f64::consts::PI * 1.9);
                let tip = plus(
                    plus(centre, x, radius * angle.cos()),
                    t,
                    radius * angle.sin(),
                );
                Curve::arc(centre, end, tip, x, t)?
            }
        }
        Reach::Smooth => {
            if spline.degree == 1 {
                line()?
            } else {
                // Carry the parameter on in proportion to the length wanted.
                let (a, b) = spline.domain();
                let along = by * (b - a) / curve.length()?.max(1e-12);
                let longer = spline.extended_end(along);
                let (_, piece) = longer
                    .split(b)
                    .ok_or_else(|| KernelError("could not extend the curve".to_owned()))?;
                let whole = longer.to_curve()?;
                return Ok((whole, piece.to_curve()?));
            }
        }
    };
    Ok((Curve::compound(vec![curve.clone(), piece.clone()])?, piece))
}

/// The curve extended at its end or, when `at_start`, at its start.
fn extended_at(
    curve: &Curve,
    at_start: bool,
    by: f64,
    reach: Reach,
) -> Result<(Curve, Curve), KernelError> {
    if at_start {
        let (whole, piece) = extended(&curve.reversed(), by, reach)?;
        Ok((whole.reversed(), piece.reversed()))
    } else {
        extended(curve, by, reach)
    }
}

/// `Extend`: lengthens curves to boundary curves, or, when Enter is pressed instead of choosing
/// boundaries, by a length. `Type` is a line (the default), an arc keeping the end's curvature,
/// or smooth (the curve's own shape carried on). With a length, `AtStart` extends the start.
#[derive(Default)]
pub struct Extend;

impl Extend {
    fn reach(cx: &Context<'_>) -> Reach {
        match cx.mode {
            Some("Arc") => Reach::Arc,
            Some("Smooth") => Reach::Smooth,
            _ => Reach::Line,
        }
    }
}

impl Recipe for Extend {
    const NAME: &'static str = "extend";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("Line"),
        OptionDef::mode("Arc"),
        OptionDef::mode("Smooth"),
        OptionDef::toggle("AtStart", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extend.boundary")
                .curves()
                .optional(),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.extend.curves").curves());
        }
        if cx.values.len() >= 2 && cx.values[0] == Value::Empty {
            steps.push(StepDef::number("prompt.extend.length").remembered("Length", 10.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves = curves_of(cx.doc, objects(cx, 1));
        if curves.is_empty() {
            return Ok(Made::default());
        }
        let reach = Self::reach(cx);
        let mut made = Made::default();
        if cx.values[0] == Value::Empty {
            let Some(by) = number(cx, 2) else {
                return Ok(Made::default());
            };
            if by <= 0.0 {
                return Err(Message::new("error.bad_distance"));
            }
            for (id, curve) in curves {
                let (whole, _) =
                    extended_at(&curve, cx.toggle("AtStart"), by, reach).map_err(|e| failed(&e))?;
                made.replace.push((id, Shape::Curve(whole)));
            }
        } else {
            let boundaries: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
                .into_iter()
                .map(|(_, c)| c)
                .collect();
            for (id, curve) in curves {
                if let Some(whole) = to_boundary(&curve, &boundaries, reach)? {
                    made.replace.push((id, Shape::Curve(whole)));
                }
            }
        }
        made.message = Some(Message::with(
            "done.extend",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

/// The curve extended at whichever end reaches a boundary soonest, up to that boundary.
fn to_boundary(
    curve: &Curve,
    boundaries: &[Curve],
    reach: Reach,
) -> Result<Option<Curve>, Message> {
    // Far enough to cross any boundary: twenty times the size of everything involved.
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for points in std::iter::once(curve)
        .chain(boundaries)
        .filter_map(|c| c.sample().ok())
    {
        for p in points {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    let far = (gap(low, high) * 20.0).max(1.0);
    let mut best: Option<(f64, Curve)> = None;
    for at_start in [false, true] {
        let Ok((_, piece)) = extended_at(curve, at_start, far, reach) else {
            continue;
        };
        for boundary in boundaries {
            let Ok(hits) = curva_kernel::crossings(&piece, boundary) else {
                continue;
            };
            for hit in hits {
                let Ok(parts) = piece.cut_at(&[hit]) else {
                    continue;
                };
                let Some(first) = parts.first() else { continue };
                let reached = first.length().unwrap_or(f64::INFINITY);
                if reached < 1e-9 || best.as_ref().is_some_and(|(d, _)| *d <= reached) {
                    continue;
                }
                let whole = if at_start {
                    Curve::compound(vec![first.reversed().clone(), curve.clone()])
                } else {
                    Curve::compound(vec![curve.clone(), first.clone()])
                };
                if let Ok(whole) = whole {
                    best = Some((reached, whole));
                }
            }
        }
    }
    Ok(best.map(|(_, whole)| whole))
}

// ---------------------------------------------------------------------------------------------

/// The furthest the curve `b` strays from `a`, by points along `b`.
fn deviation(a: &Curve, b: &Curve) -> f64 {
    let Ok(points) = b.divided(64) else {
        return f64::INFINITY;
    };
    points
        .iter()
        .map(|p| a.closest_point(*p).map_or(f64::INFINITY, |q| gap(*p, q)))
        .fold(0.0, f64::max)
}

/// `Fair`: smooths curves' bends, moving their inner control points no further than the
/// tolerance from the shape, their ends fixed.
#[derive(Default)]
pub struct Fair;

impl Recipe for Fair {
    const NAME: &'static str = "fair";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.fair.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.fair.tolerance").remembered("Tolerance", 0.01));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(tolerance) = number(cx, 1) else {
            return Ok(Made::default());
        };
        if tolerance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let original = match curve {
                Curve::Nurbs { .. } | Curve::Interpolated { .. } | Curve::Polyline { .. } => {
                    curve.clone()
                }
                _ => continue,
            };
            let count = original.control_points().map(|p| p.len()).unwrap_or(0);
            if count < 3 {
                continue;
            }
            let inner: Vec<usize> = (1..count - 1).collect();
            let mut fair = original.clone();
            for _ in 0..200 {
                let Ok(next) = fair.smoothed(&inner, 0.25) else {
                    break;
                };
                if deviation(&original, &next) > tolerance {
                    break;
                }
                fair = next;
            }
            if fair != original {
                made.replace.push((id, Shape::Curve(fair)));
            }
        }
        made.message = Some(Message::with(
            "done.fair",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// A point in the plane's own coordinates.
fn flat(p: Point, origin: Point, x: Point, y: Point) -> [f64; 2] {
    let d = sub(p, origin);
    [dot(d, x), dot(d, y)]
}

fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut crossed = false;
    for (index, a) in ring.iter().enumerate() {
        let b = ring[(index + 1) % ring.len()];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                crossed = !crossed;
            }
        }
    }
    crossed
}

/// `CurveBoolean`: closed flat curves that overlap, then clicks inside the regions to keep; Enter
/// makes their outlines. `CombineRegions` (the default) outlines the regions picked together; off,
/// each separately. `DeleteInput` takes the curves away.
#[derive(Default)]
pub struct CurveBoolean;

impl Recipe for CurveBoolean {
    const NAME: &'static str = "curveboolean";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle("CombineRegions", true),
        OptionDef::toggle("DeleteInput", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.curveboolean.curves").curves()];
        if cx.values.is_empty() {
            return steps;
        }
        // Then a run of clicks inside regions, which Enter ends once there is one.
        let clicks = &cx.values[1..];
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point("prompt.curveboolean.inside");
            steps.push(if i > 0 { step.optional() } else { step });
        }
        if clicks.last() != Some(&Value::Empty) {
            let step = StepDef::point("prompt.curveboolean.inside");
            steps.push(if clicks.is_empty() {
                step
            } else {
                step.optional()
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<(u64, Curve)> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .filter(|(_, c)| c.is_closed().unwrap_or(false))
            .collect();
        let picks: Vec<Point> = cx.values[1..].iter().filter_map(Value::point).collect();
        if curves.is_empty() || picks.is_empty() {
            return Ok(Made::default());
        }
        let [x, y] = cx.plane();
        let origin = picks[0];
        let rings: Vec<Vec<[f64; 2]>> = curves
            .iter()
            .map(|(_, c)| {
                c.sample()
                    .map(|points| points.iter().map(|p| flat(*p, origin, x, y)).collect())
                    .unwrap_or_default()
            })
            .collect();
        let signature = |p: [f64; 2]| -> Vec<bool> { rings.iter().map(|r| inside(p, r)).collect() };
        let wanted: Vec<Vec<bool>> = picks
            .iter()
            .map(|p| signature(flat(*p, origin, x, y)))
            .filter(|s| s.iter().any(|inside| *inside))
            .collect();
        if wanted.is_empty() {
            return Err(Message::new("error.curveboolean_outside"));
        }
        // Every curve cut where the others cross it.
        let mut pieces = Vec::new();
        for (i, (_, curve)) in curves.iter().enumerate() {
            let mut cuts = Vec::new();
            for (j, (_, other)) in curves.iter().enumerate() {
                if i != j {
                    cuts.extend(curva_kernel::crossings(curve, other).unwrap_or_default());
                }
            }
            pieces.extend(curve.cut_at(&cuts).unwrap_or_else(|_| vec![curve.clone()]));
        }
        let size = rings
            .iter()
            .flatten()
            .fold(0.0_f64, |m, p| m.max(p[0].abs()).max(p[1].abs()))
            .max(1.0);
        let nudge = size * 1e-5;
        let regions: Vec<Vec<Vec<bool>>> = if cx.toggle("CombineRegions") {
            vec![wanted.clone()]
        } else {
            wanted.iter().map(|w| vec![w.clone()]).collect()
        };
        let mut made = Made::default();
        for region in regions {
            let held = |p: [f64; 2]| region.contains(&signature(p));
            let edges: Vec<Curve> = pieces
                .iter()
                .filter(|piece| {
                    let Ok(points) = piece.sample() else {
                        return false;
                    };
                    let middle = points.len() / 2;
                    let (a, b) = (
                        points[middle.saturating_sub(1)],
                        points[(middle + 1).min(points.len() - 1)],
                    );
                    let m = points[middle];
                    let along = flat(sub(b, a), [0.0; 3], x, y);
                    let n = (along[0].hypot(along[1])).max(1e-12);
                    let side = [-along[1] / n * nudge, along[0] / n * nudge];
                    let at = flat(m, origin, x, y);
                    held([at[0] + side[0], at[1] + side[1]])
                        != held([at[0] - side[0], at[1] - side[1]])
                })
                .cloned()
                .collect();
            for (_, outline) in curva_kernel::join(&edges).map_err(|e| failed(&e))? {
                made.add.push(Shape::Curve(outline));
            }
        }
        if cx.toggle("DeleteInput") {
            made.remove = curves.iter().map(|(id, _)| *id).collect();
        }
        made.message = Some(Message::with(
            "done.curveboolean",
            vec![made.add.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Match`: changes the end of a curve nearest another so it meets that curve, in position,
/// along its tangent (the default) or in curvature, moving only the control points near that end.
#[derive(Default)]
pub struct Match;

/// The spline with enough control points that moving the last three leaves its start alone.
fn with_room(mut spline: Spline, needed: usize) -> Spline {
    if spline.degree < 2 {
        spline = spline.elevated(2 - spline.degree);
    }
    while spline.points.len() < needed + 1 {
        let (a, b) = spline.domain();
        // Split the last span, so the start is the last to be affected.
        let last = spline.knots[spline.points.len() - 1].max(a);
        spline = spline.inserted((last + b) / 2.0, 1);
    }
    spline
}

/// The spline with its end moved to `end`, and, as asked, its end derivative `d1` and second
/// derivative `d2`.
fn matched_end(spline: Spline, end: Point, d1: Point, d2: Point, how: Continuity) -> Spline {
    let needed = match how {
        Continuity::Position => 1,
        Continuity::Tangency => 2,
        Continuity::Curvature => 3,
    };
    let mut s = with_room(spline, needed);
    let (p, n) = (s.degree as f64, s.points.len() - 1);
    let deg = s.degree;
    let u = s.knots.clone();
    let weight = |q: [f64; 4]| q[3];
    let put = |s: &mut Spline, i: usize, at: Point| {
        let w = weight(s.points[i]);
        s.points[i] = [at[0] * w, at[1] * w, at[2] * w, w];
    };
    put(&mut s, n, end);
    if how == Continuity::Position {
        return s;
    }
    let span1 = u[n + deg] - u[n];
    let p1 = plus(end, d1, -span1 / p);
    put(&mut s, n - 1, p1);
    if how == Continuity::Tangency {
        return s;
    }
    let span2 = u[n + deg - 1] - u[n];
    let q0: Point = std::array::from_fn(|k| d1[k] - d2[k] * span2 / (p - 1.0));
    let span3 = u[n + deg - 1] - u[n - 1];
    put(&mut s, n - 2, plus(p1, q0, -span3 / p));
    s
}

impl Recipe for Match {
    const NAME: &'static str = "match";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.match.change").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.match.target").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (first, second) = (
            curves_of(cx.doc, objects(cx, 0)),
            curves_of(cx.doc, objects(cx, 1)),
        );
        let (Some((id, a)), Some((_, b))) = (first.first(), second.first()) else {
            return Ok(Made::default());
        };
        let (a, b) = (
            Spline::of(a).map_err(|e| failed(&e))?,
            Spline::of(b).map_err(|e| failed(&e))?,
        );
        let (a_end, b_end) = nearest_ends(&a, &b);
        // The target: where b is, and how it carries on, seen from a arriving at it.
        let (end, bd1, bd2) = leaving(&b, b_end);
        let target_d1 = scaled(bd1, -1.0);
        let worked = if a_end { a.clone() } else { a.reversed() };
        let (_, old_d1, _) = worked.end_derivatives();
        let speed = length(old_d1).max(1e-12);
        let bspeed = length(target_d1).max(1e-12);
        let d1 = scaled(target_d1, speed / bspeed);
        let d2 = scaled(bd2, (speed / bspeed).powi(2));
        let changed = matched_end(worked, end, d1, d2, continuity(cx, Continuity::Tangency));
        let changed = if a_end { changed } else { changed.reversed() };
        let curve = changed.to_curve().map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(*id, Shape::Curve(curve))],
            message: Some(Message::new("done.match")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// The centre of the circle through three points, if they are not in a line.
fn circle_through(a: Point, b: Point, c: Point) -> Option<Point> {
    let (ab, ac) = (sub(b, a), sub(c, a));
    let n = cross(ab, ac);
    let nn = dot(n, n);
    if nn < 1e-18 {
        return None;
    }
    let term1 = scaled(cross(n, ab), dot(ac, ac));
    let term2 = scaled(cross(ac, n), dot(ab, ab));
    Some(plus(
        a,
        std::array::from_fn(|k| (term1[k] + term2[k]) / (2.0 * nn)),
        1.0,
    ))
}

/// A piece as a line or an arc, when it is one within `tolerance`.
fn simpler(piece: &Curve, tolerance: f64) -> Option<Curve> {
    if matches!(
        piece,
        Curve::Line { .. } | Curve::Arc { .. } | Curve::Circle { .. }
    ) {
        return None;
    }
    let points = piece.divided(64).ok()?;
    let (start, end) = (points[0], points[points.len() - 1]);
    let chord = unit(sub(end, start));
    if let Some(dir) = chord {
        let far = points
            .iter()
            .map(|p| {
                let d = sub(*p, start);
                length(sub(d, scaled(dir, dot(d, dir))))
            })
            .fold(0.0, f64::max);
        if far <= tolerance {
            return Curve::line(start, end).ok();
        }
    }
    let middle = points[points.len() / 2];
    let centre = circle_through(start, middle, end)?;
    let radius = gap(centre, start);
    if points
        .iter()
        .all(|p| (gap(*p, centre) - radius).abs() <= tolerance)
    {
        let x = unit(sub(start, centre))?;
        let n = unit(cross(x, sub(middle, centre)))?;
        let y = cross(n, x);
        return Curve::arc(centre, start, end, x, y).ok();
    }
    None
}

/// `SimplifyCrv`: replaces the parts of curves that are lines or arcs within the tolerance with
/// exact lines and arcs.
#[derive(Default)]
pub struct SimplifyCrv;

impl Recipe for SimplifyCrv {
    const NAME: &'static str = "simplifycrv";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.simplifycrv.objects").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let tolerance = 1e-3;
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let pieces = curve.exploded().unwrap_or_else(|| vec![curve.clone()]);
            let mut changed = false;
            let simplified: Vec<Curve> = pieces
                .iter()
                .map(|piece| {
                    simpler(piece, tolerance).map_or_else(
                        || piece.clone(),
                        |s| {
                            changed = true;
                            s
                        },
                    )
                })
                .collect();
            if !changed {
                continue;
            }
            let whole = if simplified.len() == 1 {
                simplified[0].clone()
            } else {
                Curve::compound(simplified).map_err(|e| failed(&e))?
            };
            made.replace.push((id, Shape::Curve(whole)));
        }
        made.message = Some(Message::with(
            "done.simplifycrv",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Reparameterize`: gives curves a new parameter range, start and end.
#[derive(Default)]
pub struct Reparameterize;

impl Recipe for Reparameterize {
    const NAME: &'static str = "reparameterize";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.reparameterize.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.reparameterize.start").remembered("Start", 0.0));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.reparameterize.end").remembered("End", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(start), Some(end)) = (number(cx, 1), number(cx, 2)) else {
            return Ok(Made::default());
        };
        if end <= start {
            return Err(Message::new("error.reparameterize_order"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            let curve = spline
                .reparameterized(start, end)
                .to_curve()
                .map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.reparameterize",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `MakeUniform`: puts curves' knots evenly apart, keeping their control points (the shape
/// changes a little).
#[derive(Default)]
pub struct MakeUniform;

impl Recipe for MakeUniform {
    const NAME: &'static str = "makeuniform";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.makeuniform.objects").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            if matches!(curve, Curve::Line { .. } | Curve::Point { .. }) {
                continue;
            }
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            let curve = spline.uniform().to_curve().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.makeuniform",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Weight`: sets the weight of the control points picked (with `PointsOn`): more than 1 pulls
/// the curve towards a point, less lets it go.
#[derive(Default)]
pub struct Weight;

impl Recipe for Weight {
    const NAME: &'static str = "weight";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::number("prompt.weight.value").remembered("Weight", 1.0)]
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.doc.grip_selection().is_empty() {
            return Err(Message::new("error.weight_points"));
        }
        match number(cx, 0) {
            Some(w) if w <= 0.0 => Err(Message::new("error.weight_positive")),
            _ => Ok(()),
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(weight) = number(cx, 0) else {
            return Ok(Made::default());
        };
        let grips = cx.doc.grip_selection();
        let mut owners: Vec<u64> = grips.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, &owners) {
            // Only control-point curves: their grips are their control points, one for one.
            if !matches!(curve, Curve::Nurbs { .. }) || curve.is_closed().unwrap_or(false) {
                continue;
            }
            let mut spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            for (_, index) in grips.iter().filter(|(owner, _)| *owner == id) {
                if let Some(q) = spline.points.get_mut(*index) {
                    let at = [q[0] / q[3], q[1] / q[3], q[2] / q[3]];
                    *q = [at[0] * weight, at[1] * weight, at[2] * weight, weight];
                }
            }
            let curve = spline.to_curve().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.weight",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::{Document, Shape};
    use curva_kernel::{Curve, Spline, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    fn ends(curve: &Curve) -> (curva_kernel::Point, curva_kernel::Point) {
        let points = curve.sample().unwrap();
        (points[0], points[points.len() - 1])
    }

    #[test]
    fn handle_curve_leaves_each_point_towards_its_handle() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["handlecurve", "0,0", "0,5", "10,0", "10,-5", ""],
        );
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        let (a, b) = spline.domain();
        assert!(gap(spline.point(a), [0.0; 3]) < 1e-9);
        assert!(gap(spline.point(b), [10.0, 0.0, 0.0]) < 1e-9);
        // Leaving upward, towards the first handle.
        let d = spline.derivative(a);
        assert!(d[0].abs() < 1e-3 * d[1] && d[1] > 0.0, "{d:?}");
    }

    #[test]
    fn sketch_passes_through_the_points_drawn() {
        let mut doc = Document::default();
        run(&mut doc, &["sketch", "0,0", "1,1", "2,0", "3,1", "4,0", ""]);
        let curve = only_curve(&doc);
        let (start, end) = ends(&curve);
        assert!(gap(start, [0.0; 3]) < 1e-9 && gap(end, [4.0, 0.0, 0.0]) < 1e-9);
    }

    #[test]
    fn a_conic_with_rho_a_half_is_a_parabola() {
        let mut doc = Document::default();
        run(&mut doc, &["conic", "-2,1", "2,1", "0,-1", "0.5"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        for i in 0..=10 {
            let p = spline.point(f64::from(i) / 10.0);
            // y = x² / 4 passes through the ends and has its apex tangents meeting at (0, -1).
            assert!((p[1] - p[0] * p[0] / 4.0).abs() < 1e-9, "{p:?}");
        }
        // A rho of 1 is refused.
        let mut doc = Document::default();
        let processor = run(&mut doc, &["conic", "-2,1", "2,1", "0,-1", "1"]);
        assert!(processor.log().iter().any(|line| line.contains("Rho")));
    }

    #[test]
    fn a_hyperbola_is_exact() {
        // a = 2, c = 3: b² = 5; every point satisfies x²/4 − y²/5 = 1.
        let mut doc = Document::default();
        run(&mut doc, &["hyperbola", "0,0", "2,0", "3", "5,4"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        for i in 0..=20 {
            let p = spline.point(f64::from(i) / 20.0);
            assert!(
                (p[0] * p[0] / 4.0 - p[1] * p[1] / 5.0 - 1.0).abs() < 1e-9,
                "{p:?}"
            );
        }
    }

    #[test]
    fn blends_meet_both_curves_smoothly() {
        for (mode, smooth) in [("Position", false), ("Tangency", true), ("Curvature", true)] {
            let mut doc = Document::default();
            let a = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
            let b = doc.add(Curve::line([10.0, 3.0, 0.0], [15.0, 3.0, 0.0]).unwrap());
            let mut processor = crate::Processor::default();
            processor.submit(&mut doc, "blendcrv");
            processor.submit(&mut doc, mode);
            doc.select(&[a]);
            processor.submit(&mut doc, "");
            doc.select(&[b]);
            processor.submit(&mut doc, "");
            let blend = doc.state().objects.last().unwrap().curve().unwrap().clone();
            let (start, end) = ends(&blend);
            assert!(gap(start, [5.0, 0.0, 0.0]) < 1e-6, "{mode}: {start:?}");
            assert!(gap(end, [10.0, 3.0, 0.0]) < 1e-6, "{mode}: {end:?}");
            if smooth {
                let spline = Spline::of(&blend).unwrap();
                let (a0, a1) = spline.domain();
                let (d0, d1) = (spline.derivative(a0), spline.derivative(a1));
                assert!(d0[1].abs() < 1e-3 * d0[0].abs(), "{mode}: {d0:?}");
                assert!(d1[1].abs() < 1e-3 * d1[0].abs(), "{mode}: {d1:?}");
            }
        }
    }

    #[test]
    fn offset_multiple_moves_each_curve_to_the_side() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 5.0, 0.0], [10.0, 5.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["offsetmultiple", "1", "5,20"]);
        assert_eq!(doc.state().objects.len(), 4);
        let ys: Vec<f64> = doc.state().objects[2..]
            .iter()
            .map(|o| o.curve().unwrap().sample().unwrap()[0][1])
            .collect();
        assert!(
            ys.iter().any(|y| (y - 1.0).abs() < 1e-9) && ys.iter().any(|y| (y - 6.0).abs() < 1e-9),
            "{ys:?}"
        );
    }

    #[test]
    fn dup_border_copies_a_surface_outline() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["dupborder"]);
        let border = only_curve(&doc);
        assert!(border.is_closed().unwrap());
        assert!((border.length().unwrap() - 14.0).abs() < 1e-6);
    }

    #[test]
    fn extend_by_a_length_and_to_a_boundary() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extend");
        processor.submit(&mut doc, "");
        doc.select(&[line]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "3");
        assert!((only_curve(&doc).length().unwrap() - 8.0).abs() < 1e-6);
        // To a boundary: a wall at x = 12.
        let mut doc = Document::default();
        let wall = doc.add(Curve::line([12.0, -5.0, 0.0], [12.0, 5.0, 0.0]).unwrap());
        let line = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extend");
        doc.select(&[wall]);
        processor.submit(&mut doc, "");
        doc.select(&[line]);
        processor.submit(&mut doc, "");
        let extended = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == line)
            .unwrap()
            .curve()
            .unwrap()
            .clone();
        assert!(
            (extended.length().unwrap() - 12.0).abs() < 1e-6,
            "{}",
            extended.length().unwrap()
        );
    }

    #[test]
    fn smooth_and_arc_extensions_carry_the_curve_on() {
        for kind in ["Smooth", "Arc"] {
            let mut doc = Document::default();
            let arc = doc.add(
                Curve::arc(
                    [0.0; 3],
                    [5.0, 0.0, 0.0],
                    [0.0, 5.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .unwrap(),
            );
            let mut processor = crate::Processor::default();
            processor.submit(&mut doc, "extend");
            processor.submit(&mut doc, kind);
            processor.submit(&mut doc, "");
            doc.select(&[arc]);
            processor.submit(&mut doc, "");
            processor.submit(&mut doc, "2");
            let longer = only_curve(&doc);
            assert!(
                longer.length().unwrap() > 5.0 * std::f64::consts::FRAC_PI_2 + 1.5,
                "{kind}"
            );
            // An arc carried on as an arc stays on its circle.
            if kind == "Arc" {
                let (_, end) = ends(&longer);
                assert!((gap(end, [0.0; 3]) - 5.0).abs() < 1e-6, "{end:?}");
            }
        }
    }

    #[test]
    fn fair_stays_within_its_tolerance() {
        let mut doc = Document::default();
        let wiggly = Curve::control_point(
            vec![
                [0.0; 3],
                [1.0, 0.3, 0.0],
                [2.0, -0.3, 0.0],
                [3.0, 0.3, 0.0],
                [4.0, -0.3, 0.0],
                [5.0, 0.0, 0.0],
            ],
            3,
            false,
        )
        .unwrap();
        let id = doc.add(wiggly.clone());
        doc.select(&[id]);
        run(&mut doc, &["fair", "0.05"]);
        let fair = only_curve(&doc);
        assert_ne!(fair, wiggly);
        assert!(super::deviation(&wiggly, &fair) <= 0.05 + 1e-9);
        let (start, end) = ends(&fair);
        assert!(gap(start, [0.0; 3]) < 1e-9 && gap(end, [5.0, 0.0, 0.0]) < 1e-9);
    }

    #[test]
    fn curve_boolean_outlines_the_regions_picked() {
        let mut doc = Document::default();
        let a = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        let b = doc.add(
            Curve::polyline(
                vec![
                    [2.0, 2.0, 0.0],
                    [6.0, 2.0, 0.0],
                    [6.0, 6.0, 0.0],
                    [2.0, 6.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        // The overlap only: a 2 × 2 square.
        run(&mut doc, &["curveboolean", "3,3", ""]);
        let outline = only_curve(&doc);
        assert!(outline.is_closed().unwrap());
        assert!(
            (outline.length().unwrap() - 8.0).abs() < 1e-6,
            "{}",
            outline.length().unwrap()
        );
        // Both squares together: the union's outline, 24 long.
        let mut doc = Document::default();
        let a = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        let b = doc.add(
            Curve::polyline(
                vec![
                    [2.0, 2.0, 0.0],
                    [6.0, 2.0, 0.0],
                    [6.0, 6.0, 0.0],
                    [2.0, 6.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        run(&mut doc, &["curveboolean", "1,1", "3,3", "5,5", ""]);
        let union = only_curve(&doc);
        assert!(
            (union.length().unwrap() - 24.0).abs() < 1e-6,
            "{}",
            union.length().unwrap()
        );
    }

    #[test]
    fn match_moves_the_nearest_end_onto_the_other_curve() {
        let mut doc = Document::default();
        let a = doc.add(
            Curve::control_point(
                vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0], [6.0, 1.0, 0.0]],
                3,
                false,
            )
            .unwrap(),
        );
        let b = doc.add(Curve::line([7.0, 0.0, 0.0], [12.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "match");
        processor.submit(&mut doc, "Curvature");
        doc.select(&[a]);
        processor.submit(&mut doc, "");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        let matched = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == a)
            .unwrap()
            .curve()
            .unwrap()
            .clone();
        let spline = Spline::of(&matched).unwrap();
        let (s, e) = spline.domain();
        assert!(gap(spline.point(e), [7.0, 0.0, 0.0]) < 1e-9);
        assert!(gap(spline.point(s), [0.0; 3]) < 1e-9);
        let (_, d1, d2) = spline.end_derivatives();
        assert!(d1[1].abs() < 1e-9 && d1[0] > 0.0, "{d1:?}");
        assert!(d2[1].abs() < 1e-6, "{d2:?}");
    }

    #[test]
    fn simplify_turns_straight_and_round_nurbs_into_lines_and_arcs() {
        let mut doc = Document::default();
        let straight = doc.add(
            Curve::control_point(
                vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                3,
                false,
            )
            .unwrap(),
        );
        let round = doc.add(
            Spline::of(
                &Curve::arc(
                    [0.0; 3],
                    [5.0, 0.0, 0.0],
                    [0.0, 5.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .unwrap(),
            )
            .unwrap()
            .to_curve()
            .unwrap(),
        );
        doc.select(&[straight, round]);
        run(&mut doc, &["simplifycrv"]);
        let kinds: Vec<bool> = doc
            .state()
            .objects
            .iter()
            .map(|o| matches!(o.curve(), Some(Curve::Line { .. } | Curve::Arc { .. })))
            .collect();
        assert_eq!(kinds, vec![true, true]);
    }

    #[test]
    fn reparameterize_and_make_uniform() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(
                vec![
                    [0.0; 3],
                    [1.0, 2.0, 0.0],
                    [3.0, 2.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [6.0, 1.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "10", "20"]);
        assert_eq!(
            Spline::of(&only_curve(&doc)).unwrap().domain(),
            (10.0, 20.0)
        );
        run(&mut doc, &["makeuniform"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        let inner = &spline.knots[4..spline.knots.len() - 4];
        assert!((inner[0] - 15.0).abs() < 1e-9, "{:?}", spline.knots);
    }

    #[test]
    fn weight_pulls_the_curve_to_the_point() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(vec![[0.0; 3], [5.0, 5.0, 0.0], [10.0, 0.0, 0.0]], 2, false)
                .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["pointson"]);
        doc.select_grips(&[(id, 1)]);
        run(&mut doc, &["weight", "5"]);
        let curve = doc.state().objects[0].curve().unwrap().clone();
        let peak = Spline::of(&curve).unwrap().point(0.5);
        // Unweighted the middle is at y = 2.5; weighted 5 it is pulled to 5 × 5 / 6.
        assert!((peak[1] - 25.0 / 6.0).abs() < 1e-9, "{peak:?}");
        let _ = Shape::Curve(curve);
    }
}
