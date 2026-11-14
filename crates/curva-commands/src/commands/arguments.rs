//! Small helpers recipes share for reading their answers
//! (`docs/reviews/code-health-2026-11-14.md`, fix 5).

use crate::Message;
use crate::recipe::{Context, Value};
use curva_kernel::{KernelError, Point, cross, unit};

pub(crate) fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

pub(crate) fn number(cx: &Context<'_>, index: usize) -> Option<f64> {
    cx.values.get(index).and_then(Value::number)
}

pub(crate) fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

pub(crate) fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

pub(crate) fn objects<'a>(cx: &'a Context<'_>, index: usize) -> &'a [u64] {
    match cx.values.get(index) {
        Some(Value::Objects(ids)) => ids,
        _ => &[],
    }
}

/// The normal of the plane being drawn on.
pub(crate) fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}
