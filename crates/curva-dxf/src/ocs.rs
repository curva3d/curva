//! Points and vectors in AutoCAD's terms, and the object coordinate system circles, arcs and
//! planar polylines lie in, fixed by their normal through the arbitrary axis algorithm.

use crate::*;

pub(crate) fn v3(p: P) -> Vector3 {
    Vector3::new(p[0], p[1], p[2])
}

pub(crate) fn p3(v: &Vector3) -> P {
    [v.x, v.y, v.z]
}

pub(crate) fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn unit(v: P) -> Option<P> {
    let length = dot(v, v).sqrt();
    (length > 1e-12).then(|| v.map(|c| c / length))
}

/// The X and Y axes of the object coordinate system for a normal, by the arbitrary axis
/// algorithm: X is the world Y (or Z) axis crossed with the normal.
pub(crate) fn object_axes(normal: P) -> (P, P) {
    let limit = 1.0 / 64.0;
    let x = if normal[0].abs() < limit && normal[1].abs() < limit {
        cross([0.0, 1.0, 0.0], normal)
    } else {
        cross([0.0, 0.0, 1.0], normal)
    };
    let x = unit(x).unwrap_or([1.0, 0.0, 0.0]);
    (x, cross(normal, x))
}

/// A world point in a normal's object coordinates.
pub(crate) fn to_object(p: P, normal: P) -> P {
    let (x, y) = object_axes(normal);
    [dot(p, x), dot(p, y), dot(p, normal)]
}

/// A point in a normal's object coordinates back in the world.
pub(crate) fn from_object(p: P, normal: P) -> P {
    let (x, y) = object_axes(normal);
    std::array::from_fn(|i| x[i] * p[0] + y[i] * p[1] + normal[i] * p[2])
}
