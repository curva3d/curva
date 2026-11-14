//! Helpers several commands in this tab share.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::Context;
use crate::recipe::Made;
use crate::recipe::StepDef;
use curva_document::Shape;
use curva_kernel::Point;
use curva_kernel::dot;
use curva_kernel::sub;

/// Every chosen object deformed by `map`: surfaces and solids face by face, curves by their
/// control points.
pub(crate) fn deform_all(
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
            // A block is deformed by exploding it first, as in Rhino; text keeps its shape.
            Shape::Instance(_) | Shape::Annotation(_) => continue,
        };
        made.replace.push((object.id, shape));
    }
    made.message = Some(Message::with(done, vec![made.replace.len().to_string()]));
    Ok(made)
}

/// Steps shared by the axis deformations: objects, the axis's start and end.
pub(crate) fn axis_steps(
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
pub(crate) fn along_axis(p: Point, start: Point, end: Point) -> (f64, Point, Point) {
    let axis = sub(end, start);
    let length = dot(axis, axis).sqrt().max(1e-12);
    let e = axis.map(|v| v / length);
    let d = sub(p, start);
    let s = dot(d, e);
    let foot = plus(start, e, s);
    ((s / length).clamp(0.0, 1.0), foot, sub(p, foot))
}
