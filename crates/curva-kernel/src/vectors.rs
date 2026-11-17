//! Points and vectors: distances, sums, products and directions.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn world_x() -> Point {
    [1.0, 0.0, 0.0]
}

pub(crate) fn world_y() -> Point {
    [0.0, 1.0, 0.0]
}

/// The distance between two points.
#[must_use]
pub fn gap(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

pub(crate) fn shifted(p: Point, by: Point) -> Point {
    [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
}

/// `a` minus `b`.
#[must_use]
pub fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The dot product of two vectors.
#[must_use]
pub fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product of two vectors.
#[must_use]
pub fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A vector made length 1, or `None` if it has no real length.
#[must_use]
pub fn unit(a: Point) -> Option<Point> {
    let length = dot(a, a).sqrt();
    (length >= SMALLEST).then(|| [a[0] / length, a[1] / length, a[2] / length])
}

/// A vector with its part along a unit `normal` removed.
#[must_use]
pub fn flattened(a: Point, normal: Point) -> Point {
    let along = dot(a, normal);
    [
        a[0] - normal[0] * along,
        a[1] - normal[1] * along,
        a[2] - normal[2] * along,
    ]
}
