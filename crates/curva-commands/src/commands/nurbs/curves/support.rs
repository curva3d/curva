//! Helpers several commands in this tab share.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::Context;
use crate::recipe::Made;
use crate::recipe::OptionDef;
use crate::recipe::StepDef;
use crate::recipe::Value;
use crate::recipe::run_points;
use curva_document::Shape;
use curva_kernel::Curve;
use curva_kernel::Point;
use curva_kernel::Spline;
use curva_kernel::cross;
use curva_kernel::dot;
use curva_kernel::gap;
use curva_kernel::sub;
use curva_kernel::unit;
use std::f64::consts::TAU;

pub(crate) fn added(
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

pub(crate) const TURNS: &str = "Turns";

/// Points along a turning path from `start` to `end` about that axis, its radius going from
/// `from` to `to`, `turns` times round; with `flat`, all on the plane at `start`.
pub(crate) fn turning(
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

pub(crate) fn length(d: Point) -> f64 {
    dot(d, d).sqrt()
}

/// A Bézier piece as a clamped spline.
pub(crate) fn bezier(points: &[Point], weights: Option<&[f64]>) -> Spline {
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
pub(crate) fn run_steps(
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

/// Which end of `a` and which end of `b` are nearest each other: `(a at its end, b at its end)`.
pub(crate) fn nearest_ends(a: &Spline, b: &Spline) -> (bool, bool) {
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
pub(crate) fn leaving(curve: &Spline, at_end: bool) -> (Point, Point, Point) {
    if at_end {
        curve.end_derivatives()
    } else {
        curve.reversed().end_derivatives()
    }
}

/// How a blend or match joins: in position only, along the tangent too, or in curvature as well.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Continuity {
    Position,
    Tangency,
    Curvature,
}

pub(crate) fn continuity(cx: &Context<'_>, default: Continuity) -> Continuity {
    match cx.mode {
        Some("Position") => Continuity::Position,
        Some("Tangency") => Continuity::Tangency,
        Some("Curvature") => Continuity::Curvature,
        _ => default,
    }
}

pub(crate) const CONTINUITY: [OptionDef; 3] = [
    OptionDef::mode("Position"),
    OptionDef::mode("Tangency"),
    OptionDef::mode("Curvature"),
];

pub(crate) fn scaled(d: Point, by: f64) -> Point {
    d.map(|v| v * by)
}
