//! Helpers commands in several tabs share.

use crate::Answer;
use crate::commands::arguments::*;
use crate::recipe::Context;
use crate::recipe::Derived;
use crate::recipe::Value;
use crate::{Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::Shape;
use curva_kernel::Body;
use curva_kernel::Curve;
use curva_kernel::KernelError;
use curva_kernel::Point;
use curva_kernel::cross;
use curva_kernel::dot;
use curva_kernel::sub;

pub(crate) fn unexpected() -> Step {
    Step::Failed(Message::new("error.unexpected"))
}

/// An answer a step did not ask for, for the framework in `recipe.rs`.
pub(crate) fn unexpected_step() -> Step {
    unexpected()
}

/// The centre and radius of the circle through three points, on the plane with axes `x` and `y`
/// through the first point. `None` when the points lie in a line.
pub(crate) fn circle_through(
    a: Point,
    b: Point,
    c: Point,
    [x, y]: [Point; 2],
) -> Option<(Point, f64)> {
    let along = |p: Point| {
        let d = curva_kernel::sub(p, a);
        [curva_kernel::dot(d, x), curva_kernel::dot(d, y)]
    };
    let ([bx, by], [cx, cy]) = (along(b), along(c));
    let d = 2.0 * (bx * cy - by * cx);
    if d.abs() < 1e-12 {
        return None;
    }
    let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
    let (u, v) = ((cy * b2 - by * c2) / d, (bx * c2 - cx * b2) / d);
    let centre: Point = std::array::from_fn(|i| a[i] + u * x[i] + v * y[i]);
    Some((centre, u.hypot(v)))
}

pub(crate) fn geometry_failed(error: &KernelError) -> Step {
    Step::Failed(Message::with("error.geometry", vec![error.to_string()]))
}

pub(crate) fn ask_point(key: &'static str, preview: Vec<Point>) -> Step {
    Step::Ask(Request {
        preview,
        ..Request::new(key, Kind::Point)
    })
}

pub(crate) fn ask_point_or_number(key: &'static str, preview: Vec<Point>) -> Step {
    Step::Ask(Request {
        preview,
        ..Request::new(key, Kind::PointOrNumber)
    })
}

/// The direction straight out of the plane being drawn on.
pub(crate) fn plane_normal(doc: &curva_document::Document) -> Point {
    let [x, y] = doc.work_plane();
    curva_kernel::cross(x, y)
}

/// Most objects shown moving at once, so that a huge selection does not stall the window.
pub(crate) const MOST_PREVIEW_OBJECTS: usize = 2000;

/// A straight construction line for a preview, or nothing if its ends are in the same place.
pub(crate) fn guide(a: Point, b: Point) -> Vec<curva_kernel::Curve> {
    curva_kernel::Curve::line(a, b).into_iter().collect()
}

/// The shapes chosen objects draw, blocks opened into theirs, for a preview (#507).
fn chosen_shapes(doc: &curva_document::Document, ids: &[u64]) -> Vec<curva_document::Shape> {
    let chosen: std::collections::HashSet<u64> = ids.iter().copied().collect();
    let mut shapes = Vec::new();
    for object in doc
        .state()
        .objects
        .iter()
        .filter(|object| chosen.contains(&object.id))
        .take(MOST_PREVIEW_OBJECTS)
    {
        match &object.shape {
            curva_document::Shape::Instance(instance) => shapes.extend(
                doc.instance_shapes(instance)
                    .into_iter()
                    .map(|(shape, _)| shape),
            ),
            shape => shapes.push(shape.clone()),
        }
    }
    shapes
}

/// How chosen objects would look after a change, for a preview: their lines, blocks included.
pub(crate) fn ghosts(
    doc: &curva_document::Document,
    ids: &[u64],
    change: impl Fn(&curva_kernel::Curve) -> curva_kernel::Curve,
) -> Vec<curva_kernel::Curve> {
    chosen_shapes(doc, ids)
        .iter()
        .flat_map(curva_document::Shape::outlines)
        .map(|curve| change(&curve))
        .collect()
}

/// The surfaces and solids of chosen objects after `change`, for the preview's faces (#507: a
/// solid being arrayed or turned showed only its edges).
pub(crate) fn ghost_bodies(
    doc: &curva_document::Document,
    ids: &[u64],
    change: impl Fn(&curva_kernel::Body) -> curva_kernel::Body,
) -> Vec<curva_kernel::Body> {
    chosen_shapes(doc, ids)
        .iter()
        .filter_map(curva_document::Shape::body)
        .map(change)
        .collect()
}

/// Shapes for a preview with block instances opened into the shapes they draw (#507).
pub(crate) fn opened(
    doc: &curva_document::Document,
    shapes: &[curva_document::Shape],
) -> Vec<curva_document::Shape> {
    let mut out = Vec::new();
    for shape in shapes {
        match shape {
            curva_document::Shape::Instance(instance) => out.extend(
                doc.instance_shapes(instance)
                    .into_iter()
                    .map(|(shape, _)| shape),
            ),
            shape => out.push(shape.clone()),
        }
    }
    out
}

/// Block instances among chosen objects, with their ids.
pub(crate) fn instances_of(
    doc: &curva_document::Document,
    ids: &[u64],
) -> Vec<(u64, curva_document::blocks::Instance)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .filter_map(|object| match &object.shape {
            curva_document::Shape::Instance(instance) => Some((object.id, *instance)),
            _ => None,
        })
        .collect()
}

/// The surfaces and solids among shapes a command would make, for the preview's faces.
pub(crate) fn result_bodies(made: &[curva_document::Shape]) -> Vec<curva_kernel::Body> {
    made.iter()
        .filter_map(curva_document::Shape::body)
        .cloned()
        .collect()
}

/// The surfaces and solids among chosen objects, with their ids.
pub(crate) fn bodies_of(
    doc: &curva_document::Document,
    ids: &[u64],
) -> Vec<(u64, curva_kernel::Body)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .filter_map(|object| Some((object.id, object.shape.body()?.clone())))
        .collect()
}

/// The curves of chosen objects, with their ids; surfaces and solids are left out.
pub(crate) fn curves_of(
    doc: &curva_document::Document,
    ids: &[u64],
) -> Vec<(u64, curva_kernel::Curve)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .filter_map(|object| Some((object.id, object.curve()?.clone())))
        .collect()
}

/// The curve passing within `reach` of `at`, if any, for commands that pick a curve.
pub(crate) fn curve_at(
    doc: &curva_document::Document,
    at: Point,
    reach: f64,
) -> Result<Option<(u64, curva_kernel::Curve)>, KernelError> {
    for object in &doc.state().objects {
        if let Some(curve) = object.curve()
            && curva_kernel::gap(curve.closest_point(at)?, at) <= reach
        {
            return Ok(Some((object.id, curve.clone())));
        }
    }
    Ok(None)
}

/// A curve in pieces, cut wherever it crosses the cutters.
pub(crate) fn cut_by(
    curve: &curva_kernel::Curve,
    cutters: &[curva_kernel::Curve],
) -> Result<Vec<curva_kernel::Curve>, KernelError> {
    let mut points = Vec::new();
    for cutter in cutters {
        points.extend(curva_kernel::crossings(curve, cutter)?);
    }
    curve.cut_at(&points)
}

/// Curves worked out for a preview, without the ids they would replace.
pub(crate) fn ghost_results(made: Vec<(u64, curva_kernel::Curve)>) -> Vec<curva_kernel::Curve> {
    made.into_iter()
        .take(MOST_PREVIEW_OBJECTS)
        .map(|(_, curve)| curve)
        .collect()
}

pub(crate) fn ask_objects(key: &'static str) -> Step {
    Step::Ask(Request::new(key, Kind::Objects))
}

/// The objects a command works on: those given as the answer, or else the ones selected; `None`
/// when there are none, so the command can ask for them.
pub(crate) fn objects_or_selection(
    doc: &curva_document::Document,
    answer: Option<crate::Answer>,
) -> Option<Vec<u64>> {
    let chosen = chosen(doc, &answer);
    (!chosen.is_empty()).then_some(chosen)
}

/// Which curves a `Sel` command for one kind of curve picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CurveKind {
    Line,
    Polyline,
    Circle,
    Arc,
    Closed,
    Open,
}

impl CurveKind {
    pub(crate) fn takes(self, curve: &Curve) -> bool {
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

/// The points answered so far, up to an Enter.
pub(crate) fn picked(cx: &Context<'_>) -> Vec<Point> {
    cx.values.iter().filter_map(Value::point).collect()
}

/// The exact piece of the parabola with its vertex at `vertex`, opening along `axis` towards
/// its focus `focal` away, from `from` to `to` along `side`: a quadratic whose middle control
/// point is where the end tangents meet.
pub(crate) fn parabola_piece(
    vertex: Point,
    axis: Point,
    side: Point,
    focal: f64,
    from: f64,
    to: f64,
) -> Result<Curve, curva_kernel::KernelError> {
    let at = |s: f64, height: f64| plus(plus(vertex, side, s), axis, height);
    let points = vec![
        at(from, from * from / (4.0 * focal)),
        at((from + to) / 2.0, from * to / (4.0 * focal)),
        at(to, to * to / (4.0 * focal)),
    ];
    Curve::nurbs(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], points, Vec::new())
}

/// The centre of a solid's volume, from its mesh, by summing the tetrahedra its triangles make
/// with the origin.
pub(crate) fn volume_centroid(body: &Body) -> Option<Point> {
    let mesh = body.mesh().ok()?;
    let (mut volume, mut moment) = (0.0, [0.0; 3]);
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| mesh.positions[i as usize]);
        let v = dot(a, cross(b, c)) / 6.0;
        volume += v;
        for i in 0..3 {
            moment[i] += v * (a[i] + b[i] + c[i]) / 4.0;
        }
    }
    (volume.abs() > 1e-12).then(|| moment.map(|m| m / volume))
}

// CUSTOM KERNEL OPERATION: mesh-area-centroid
pub(crate) fn area_centroid(body: &Body) -> Option<Point> {
    let mesh = body.mesh().ok()?;
    let (mut area, mut moment) = (0.0, [0.0; 3]);
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| mesh.positions[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let piece = dot(n, n).sqrt() / 2.0;
        area += piece;
        for i in 0..3 {
            moment[i] += piece * (a[i] + b[i] + c[i]) / 3.0;
        }
    }
    (area > 1e-12).then(|| moment.map(|m| m / area))
}

pub(crate) fn chosen(doc: &Document, answer: &Option<Answer>) -> Vec<u64> {
    match answer {
        Some(Answer::Objects(list)) => list.clone(),
        _ => doc.selection(),
    }
}

pub(crate) fn added_curves(curves: Vec<Curve>, from: u64) -> Vec<Derived> {
    curves
        .into_iter()
        .map(|curve| Derived {
            shape: Shape::Curve(curve),
            from,
            history: None,
        })
        .collect()
}

pub(crate) fn body_of(cx: &Context<'_>, id: u64) -> Option<Body> {
    bodies_of(cx.doc, &[id]).into_iter().next().map(|(_, b)| b)
}

pub(crate) fn first_curve(cx: &Context<'_>, index: usize) -> Option<(u64, Curve)> {
    curves_of(cx.doc, objects(cx, index)).into_iter().next()
}
