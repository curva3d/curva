//! Helpers several commands in this tab share.

use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::Context;
use crate::recipe::Value;
use curva_kernel::Body;

pub(crate) fn edge(cx: &Context<'_>, index: usize) -> Option<(u64, usize)> {
    match cx.values.get(index) {
        Some(Value::Edges(edges)) => edges.first().copied(),
        _ => None,
    }
}

pub(crate) fn first_body(cx: &Context<'_>, index: usize) -> Option<(u64, Body)> {
    bodies_of(cx.doc, objects(cx, index)).into_iter().next()
}

pub(crate) fn continuity(cx: &Context<'_>, default: usize) -> usize {
    match cx.mode {
        Some("Position") => 0,
        Some("Tangency") => 1,
        Some("Curvature") => 2,
        _ => default,
    }
}
