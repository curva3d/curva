//! Curva's interface to the geometry engine. Nothing above this crate touches solidcore, so the
//! engine can change or be joined by another without the commands noticing.

use serde::{Deserialize, Serialize};
use solidcore::curve::curve::Curve as CoreCurve;
use solidcore::nurbs::b_spline_curve::BSplineCurve;
use solidcore::nurbs::crv_fit::p1::ParamMethod;
use solidcore::query::api_queries::curve_closest_point;
use solidcore::{Kernel, Point3};
use std::f64::consts::TAU;
use std::fmt;

/// A position in model space, in millimetres.
pub type Point = [f64; 3];

/// Accuracy asked of curve lengths.
const LENGTH_ACCURACY: f64 = 1e-6;
/// How far plane directions may be from length 1 and a right angle.
const PLANE_TOLERANCE: f64 = 1e-9;
/// Shortest length treated as a real size, in millimetres.
const SMALLEST: f64 = 1e-9;
/// Display points per input point of an interpolated curve.
const SAMPLES_PER_POINT: usize = 32;
/// Display points around a circle.
const CIRCLE_SAMPLES: usize = 128;

/// Why a curve could not be made or measured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelError(pub String);

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KernelError {}

/// A curve, stored by what defines it so files stay small and readable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// A straight segment.
    Line {
        /// Where it starts.
        start: Point,
        /// Where it ends.
        end: Point,
    },
    /// Straight segments through points.
    Polyline {
        /// The points in order.
        points: Vec<Point>,
        /// Whether a last segment returns to the first point.
        closed: bool,
    },
    /// A circle on a plane given by two directions.
    Circle {
        /// The centre.
        centre: Point,
        /// The radius.
        radius: f64,
        /// The plane's X direction, of length 1. Files from before circles had planes lack it.
        #[serde(default = "world_x")]
        x: Point,
        /// The plane's Y direction, of length 1, at a right angle to `x`.
        #[serde(default = "world_y")]
        y: Point,
    },
    /// A smooth curve through points.
    Interpolated {
        /// The points it passes through.
        points: Vec<Point>,
        /// Whether it joins back to its start.
        closed: bool,
    },
}

fn world_x() -> Point {
    [1.0, 0.0, 0.0]
}

fn world_y() -> Point {
    [0.0, 1.0, 0.0]
}

fn gap(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn shifted(p: Point, by: Point) -> Point {
    [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
}

fn engine_error(error: impl fmt::Debug) -> KernelError {
    KernelError(format!("{error:?}"))
}

fn spline(points: &[Point], closed: bool) -> Result<CoreCurve, KernelError> {
    let mut through: Vec<Point3> = points
        .iter()
        .map(|p| Point3::new(p[0], p[1], p[2]))
        .collect();
    // The engine closes a curve onto a repeated first point; Curva stores each point once.
    if closed && let Some(first) = through.first().copied() {
        through.push(first);
    }
    let degree = 3.min(through.len().saturating_sub(1));
    BSplineCurve::interpolate_points(
        &through,
        None,
        degree,
        None,
        None,
        closed,
        ParamMethod::ChordLength,
    )
    .map(CoreCurve::from)
    .map_err(engine_error)
}

impl Curve {
    /// A line between two different points.
    pub fn line(start: Point, end: Point) -> Result<Self, KernelError> {
        if gap(start, end) < SMALLEST {
            return Err(KernelError("the two points are the same".to_owned()));
        }
        Ok(Self::Line { start, end })
    }

    /// A polyline through at least two points.
    pub fn polyline(points: Vec<Point>, closed: bool) -> Result<Self, KernelError> {
        if points.len() < 2 {
            return Err(KernelError("a polyline needs two points".to_owned()));
        }
        Ok(Self::Polyline { points, closed })
    }

    /// A circle flat on the ground, with a positive radius.
    pub fn circle(centre: Point, radius: f64) -> Result<Self, KernelError> {
        Self::circle_on(centre, radius, world_x(), world_y())
    }

    /// A circle with a positive radius on the plane of two directions at a right angle.
    pub fn circle_on(centre: Point, radius: f64, x: Point, y: Point) -> Result<Self, KernelError> {
        if !radius.is_finite() || radius < SMALLEST {
            return Err(KernelError("the radius must be positive".to_owned()));
        }
        let dot = |a: Point, b: Point| (0..3).map(|i| a[i] * b[i]).sum::<f64>();
        let square = 1.0 - PLANE_TOLERANCE..=1.0 + PLANE_TOLERANCE;
        if !square.contains(&dot(x, x))
            || !square.contains(&dot(y, y))
            || dot(x, y).abs() > PLANE_TOLERANCE
        {
            return Err(KernelError(
                "the plane directions must be at a right angle".to_owned(),
            ));
        }
        Ok(Self::Circle {
            centre,
            radius,
            x,
            y,
        })
    }

    /// A smooth curve through the points, checked by building it now.
    pub fn interpolated(points: Vec<Point>, closed: bool) -> Result<Self, KernelError> {
        if points.len() < 2 {
            return Err(KernelError("a curve needs two points".to_owned()));
        }
        spline(&points, closed)?;
        Ok(Self::Interpolated { points, closed })
    }

    /// The curve's length in millimetres.
    pub fn length(&self) -> Result<f64, KernelError> {
        Ok(match self {
            Self::Line { start, end } => gap(*start, *end),
            Self::Polyline { .. } => self
                .sample()?
                .windows(2)
                .map(|pair| gap(pair[0], pair[1]))
                .sum(),
            Self::Circle { radius, .. } => TAU * radius,
            Self::Interpolated { points, closed } => {
                let kernel = Kernel::default();
                let curve = spline(points, *closed)?;
                curve
                    .length(&kernel, &curve.natural_interval(), LENGTH_ACCURACY)
                    .map_err(engine_error)?
            }
        })
    }

    /// Points along the curve for drawing. Corners of lines and polylines are kept exactly.
    pub fn sample(&self) -> Result<Vec<Point>, KernelError> {
        Ok(match self {
            Self::Line { start, end } => vec![*start, *end],
            Self::Polyline { points, closed } => {
                let mut out = points.clone();
                if *closed {
                    out.push(points[0]);
                }
                out
            }
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => (0..=CIRCLE_SAMPLES)
                .map(|step| {
                    let angle = TAU * step as f64 / CIRCLE_SAMPLES as f64;
                    let (cos, sin) = (angle.cos() * radius, angle.sin() * radius);
                    let around = |axis: usize| centre[axis] + x[axis] * cos + y[axis] * sin;
                    [around(0), around(1), around(2)]
                })
                .collect(),
            Self::Interpolated { points, closed } => {
                let kernel = Kernel::default();
                let curve = spline(points, *closed)?;
                let interval = curve.natural_interval();
                let steps = SAMPLES_PER_POINT * points.len();
                let mut out = Vec::with_capacity(steps + 1);
                for step in 0..=steps {
                    let t = interval.min() + interval.length() * step as f64 / steps as f64;
                    let at = curve.evaluate_point(&kernel, t).map_err(engine_error)?;
                    out.push([at.x, at.y, at.z]);
                }
                out
            }
        })
    }

    /// The shortest distance from a point to an interpolated curve, measured exactly by the
    /// engine. Other curve kinds return `None`.
    pub fn distance_to(&self, point: Point) -> Result<Option<f64>, KernelError> {
        let Self::Interpolated { points, closed } = self else {
            return Ok(None);
        };
        let kernel = Kernel::default();
        let curve = spline(points, *closed)?;
        let at = Point3::new(point[0], point[1], point[2]);
        curve_closest_point(&kernel, &curve, at)
            .map(|found| Some(found.closest.distance))
            .map_err(engine_error)
    }

    /// The same curve moved by a vector.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        let moved = |list: &[Point]| list.iter().map(|p| shifted(*p, by)).collect();
        match self {
            Self::Line { start, end } => Self::Line {
                start: shifted(*start, by),
                end: shifted(*end, by),
            },
            Self::Polyline { points, closed } => Self::Polyline {
                points: moved(points),
                closed: *closed,
            },
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => Self::Circle {
                centre: shifted(*centre, by),
                radius: *radius,
                x: *x,
                y: *y,
            },
            Self::Interpolated { points, closed } => Self::Interpolated {
                points: moved(points),
                closed: *closed,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOSE: f64 = 1e-6;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < CLOSE
    }

    #[test]
    fn a_circle_lies_on_its_plane() {
        let x = [1.0, 0.0, 0.0];
        let z = [0.0, 0.0, 1.0];
        let circle = Curve::circle_on([2.0, 3.0, 4.0], 5.0, x, z).unwrap();
        for point in circle.sample().unwrap() {
            assert!(near(point[1], 3.0));
            let reach = (point[0] - 2.0).hypot(point[2] - 4.0);
            assert!(near(reach, 5.0));
        }
    }

    #[test]
    fn a_circle_on_a_plane_has_the_same_length() {
        let plane = Curve::circle_on([0.0; 3], 3.0, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let flat = Curve::circle([0.0; 3], 3.0).unwrap();
        assert!(near(plane.length().unwrap(), flat.length().unwrap()));
    }

    #[test]
    fn a_circle_needs_directions_at_a_right_angle() {
        let tilted = [1.0, 1.0, 0.0];
        assert!(Curve::circle_on([0.0; 3], 3.0, [1.0, 0.0, 0.0], tilted).is_err());
        assert!(Curve::circle_on([0.0; 3], 3.0, [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]).is_err());
    }

    #[test]
    fn moving_a_circle_keeps_its_plane() {
        let circle = Curve::circle_on([0.0; 3], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        assert_eq!(
            circle.translated([1.0, 2.0, 3.0]),
            Curve::circle_on([1.0, 2.0, 3.0], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
        );
    }

    #[test]
    fn a_circle_stored_without_a_plane_lies_flat() {
        let text = r#"{"Circle":{"centre":[1.0,2.0,0.0],"radius":4.0}}"#;
        let circle: Curve = serde_json::from_str(text).unwrap();
        assert_eq!(circle, Curve::circle([1.0, 2.0, 0.0], 4.0).unwrap());
    }
    #[test]
    fn line_length_is_exact() {
        let line = Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap();
        assert!(near(line.length().unwrap(), 5.0));
    }

    #[test]
    fn zero_length_line_is_refused() {
        assert!(Curve::line([1.0; 3], [1.0; 3]).is_err());
    }

    #[test]
    fn closed_polyline_counts_the_last_segment() {
        let square = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let open = Curve::polyline(square.clone(), false).unwrap();
        let closed = Curve::polyline(square, true).unwrap();
        assert!(near(open.length().unwrap(), 30.0));
        assert!(near(closed.length().unwrap(), 40.0));
    }

    #[test]
    fn circle_length_and_points_are_exact() {
        let circle = Curve::circle([1.0, 2.0, 3.0], 5.0).unwrap();
        assert!(near(circle.length().unwrap(), TAU * 5.0));
        for point in circle.sample().unwrap() {
            assert!(near(gap(point, [1.0, 2.0, 3.0]), 5.0));
        }
    }

    #[test]
    fn non_positive_radius_is_refused() {
        assert!(Curve::circle([0.0; 3], 0.0).is_err());
        assert!(Curve::circle([0.0; 3], -1.0).is_err());
    }

    #[test]
    fn interpolated_curve_passes_through_every_point() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 2.0],
            [20.0, -3.0, 6.0],
            [30.0, 4.0, 1.0],
        ];
        let curve = Curve::interpolated(points.clone(), false).unwrap();
        let drawn = curve.sample().unwrap();
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
        assert!(near(gap(drawn[0], [0.0; 3]), 0.0));
        assert!(near(gap(*drawn.last().unwrap(), [30.0, 4.0, 1.0]), 0.0));
    }

    #[test]
    fn closed_interpolated_curve_returns_to_its_start() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let curve = Curve::interpolated(points.clone(), true).unwrap();
        let drawn = curve.sample().unwrap();
        assert!(gap(drawn[0], *drawn.last().unwrap()) < CLOSE);
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
    }

    #[test]
    fn closed_interpolated_curve_is_smooth_across_its_join() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let drawn = Curve::interpolated(points, true).unwrap().sample().unwrap();
        let count = drawn.len();
        let direction = |a: Point, b: Point| {
            let length = gap(a, b);
            [
                (b[0] - a[0]) / length,
                (b[1] - a[1]) / length,
                (b[2] - a[2]) / length,
            ]
        };
        let leaving = direction(drawn[0], drawn[1]);
        let arriving = direction(drawn[count - 2], drawn[count - 1]);
        let turn = (0..3).map(|i| leaving[i] * arriving[i]).sum::<f64>();
        // Neighbouring sample segments of a smooth curve point almost the same way.
        assert!(turn > 0.99, "the join turns sharply: cosine {turn}");
    }

    #[test]
    fn closed_curve_through_three_points_closes_on_them() {
        let points = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let curve = Curve::interpolated(points.clone(), true).unwrap();
        let drawn = curve.sample().unwrap();
        let last = *drawn.last().unwrap();
        assert!(
            gap(drawn[0], last) < CLOSE,
            "open by {}: starts {:?}, ends {last:?}",
            gap(drawn[0], last),
            drawn[0]
        );
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
    }

    #[test]
    fn translation_keeps_shape_and_moves_position() {
        let by = [1.0, 2.0, 3.0];
        let curve =
            Curve::interpolated(vec![[0.0; 3], [5.0, 5.0, 0.0], [9.0, 0.0, 0.0]], false).unwrap();
        let moved = curve.translated(by);
        assert!((moved.length().unwrap() - curve.length().unwrap()).abs() < 1e-3);
        let (a, b) = (curve.sample().unwrap(), moved.sample().unwrap());
        assert!(near(gap(shifted(a[0], by), b[0]), 0.0));
    }
}
