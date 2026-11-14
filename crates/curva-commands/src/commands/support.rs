//! Helpers commands in several tabs share.

use crate::Answer;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::commands::curves_of;
use crate::recipe::Context;
use crate::recipe::Derived;
use crate::recipe::Value;
use curva_document::Document;
use curva_document::Shape;
use curva_kernel::Body;
use curva_kernel::Curve;
use curva_kernel::Point;
use curva_kernel::cross;
use curva_kernel::dot;
use curva_kernel::sub;

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
