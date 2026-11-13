//! Curva's interface to the geometry engine. Nothing above this crate touches solidcore, so the
//! engine can change or be joined by another without the commands noticing.

use serde::{Deserialize, Serialize};
use solidcore::curve::curve::Curve as CoreCurve;
use solidcore::nurbs::b_spline_curve::BSplineCurve;
use solidcore::nurbs::crv_fit::p1::ParamMethod;
use solidcore::nurbs::math::{CPoint, NOW};
use solidcore::query::api_queries::curve_closest_point;
use solidcore::{Kernel, Point3};
use std::f64::consts::TAU;

mod area;
mod assemble;
pub use assemble::SEW_TOLERANCE;
mod body;
mod boolean;
pub use boolean::Combine;
mod conic;
mod corners;
mod cut;
mod fillet;
mod finishing;
mod grips;
mod knots;
mod making;
pub use making::nested_groups;
mod offset;
mod spline;
pub use spline::Spline;
mod tween;
pub use body::{Body, BodyMesh};
pub use conic::{Chamfered, chamfer_lines};
pub use cut::{closest_between, crossings};
pub use fillet::fillet_curves;
pub use offset::{Filleted, fillet_lines};
use std::fmt;
pub use tween::tween_curves;

/// A position in model space, in millimetres.
pub type Point = [f64; 3];

/// Accuracy asked of curve lengths.
const LENGTH_ACCURACY: f64 = 1e-6;
/// How far plane directions may be from length 1 and a right angle.
const PLANE_TOLERANCE: f64 = 1e-9;
/// Smallest angle treated as a real turn, in radians.
const ANGLE_TOLERANCE: f64 = 1e-9;
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
    /// A single point in space.
    Point {
        /// Where it is.
        at: Point,
    },
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
    /// Part of a circle, starting along `x` and turning towards `y`.
    Arc {
        /// The centre.
        centre: Point,
        /// The radius.
        radius: f64,
        /// The direction of its start, of length 1.
        x: Point,
        /// A quarter turn on from `x`, of length 1.
        y: Point,
        /// How far it turns, in radians, between 0 and a full turn.
        sweep: f64,
    },
    /// A NURBS curve, which can hold any piece of a smooth curve exactly.
    Nurbs {
        /// The polynomial degree.
        degree: usize,
        /// The knot vector, with `points.len() + degree + 1` values.
        knots: Vec<f64>,
        /// The control points.
        points: Vec<Point>,
        /// One weight per control point, or empty when every weight is 1.
        weights: Vec<f64>,
    },
    /// Parts that meet end to end, each keeping its own kind.
    Compound {
        /// The parts in order.
        segments: Vec<Curve>,
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

/// The distance between two points.
#[must_use]
pub fn gap(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn shifted(p: Point, by: Point) -> Point {
    [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
}

fn engine_error(error: impl fmt::Debug) -> KernelError {
    KernelError(format!("{error:?}"))
}
fn spline(points: &[Point], closed: bool) -> Result<CoreCurve, KernelError> {
    spline_data(points, closed).map(CoreCurve::from)
}

fn spline_data(points: &[Point], closed: bool) -> Result<BSplineCurve, KernelError> {
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
    .map_err(engine_error)
}

/// Builds the engine's curve from NURBS data. Rational control points go to it weighted.
fn nurbs_data(
    degree: usize,
    knots: &[f64],
    points: &[Point],
    weights: &[f64],
) -> Result<BSplineCurve, KernelError> {
    let control: Vec<CPoint> = points
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let w = weights.get(index).copied();
            let scale = w.unwrap_or(1.0);
            CPoint {
                x: p[0] * scale,
                y: p[1] * scale,
                z: p[2] * scale,
                w: w.unwrap_or(NOW),
            }
        })
        .collect();
    BSplineCurve::from_data(
        3,
        points.len().saturating_sub(1),
        knots.len().saturating_sub(1),
        degree,
        &control,
        knots,
    )
    .map_err(engine_error)
}

/// The engine curve as Curva NURBS data.
fn curve_from_engine(engine: &BSplineCurve) -> Result<Curve, KernelError> {
    let degree = engine.degree().map_err(engine_error)?;
    let (values, counts) = engine.knots(None).map_err(engine_error)?;
    let knots = values
        .iter()
        .zip(&counts)
        .flat_map(|(value, count)| std::iter::repeat_n(*value, *count))
        .collect();
    let (polygon, weights) = engine.control_polygon().map_err(engine_error)?;
    let points = polygon.iter().map(|p| [p.x, p.y, p.z]).collect();
    Curve::nurbs(degree, knots, points, weights)
}
fn nurbs_engine_curve(
    degree: usize,
    knots: &[f64],
    points: &[Point],
    weights: &[f64],
) -> Result<CoreCurve, KernelError> {
    nurbs_data(degree, knots, points, weights).map(CoreCurve::from)
}

/// Evenly spaced points along an engine curve, by its parameter.
fn sampled(curve: &CoreCurve, steps: usize) -> Result<Vec<Point>, KernelError> {
    let kernel = Kernel::default();
    let interval = curve.natural_interval();
    let mut out = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let t = interval.min() + interval.length() * step as f64 / steps as f64;
        let at = curve.evaluate_point(&kernel, t).map_err(engine_error)?;
        out.push([at.x, at.y, at.z]);
    }
    Ok(out)
}

/// How close two curve ends must be to join, in millimetres.
const JOIN_TOLERANCE: f64 = 1e-3;

/// One open curve ready to be chained to others.
struct JoinPiece {
    index: usize,
    curve: Curve,
    start: Point,
    end: Point,
}

impl JoinPiece {
    fn of(index: usize, curve: &Curve) -> Option<Self> {
        if matches!(curve, Curve::Polyline { closed: true, .. }) {
            return None;
        }
        let points = curve.sample().ok()?;
        let (start, end) = (*points.first()?, *points.last()?);
        (gap(start, end) > JOIN_TOLERANCE).then(|| Self {
            index,
            curve: curve.clone(),
            start,
            end,
        })
    }

    fn tail(&self, reversed: bool) -> Point {
        if reversed { self.start } else { self.end }
    }

    fn head(&self, reversed: bool) -> Point {
        self.tail(!reversed)
    }
}

/// Grows a chain of pieces from both its ends, returning each piece used and whether it is
/// reversed.
fn chain_from(pieces: &[JoinPiece], used: &mut [bool], seed: usize) -> Vec<(usize, bool)> {
    let mut chain = vec![(seed, false)];
    used[seed] = true;
    for grow_tail in [true, false] {
        loop {
            let &(at, flipped) = if grow_tail {
                chain.last()
            } else {
                chain.first()
            }
            .unwrap_or(&(seed, false));
            let open = if grow_tail {
                pieces[at].tail(flipped)
            } else {
                pieces[at].head(flipped)
            };
            let next = (0..pieces.len()).find_map(|candidate| {
                if used[candidate] {
                    return None;
                }
                [false, true].into_iter().find_map(|reversed| {
                    let meeting = if grow_tail {
                        pieces[candidate].head(reversed)
                    } else {
                        pieces[candidate].tail(reversed)
                    };
                    (gap(open, meeting) <= JOIN_TOLERANCE).then_some((candidate, reversed))
                })
            });
            let Some(link) = next else {
                break;
            };
            used[link.0] = true;
            if grow_tail {
                chain.push(link);
            } else {
                chain.insert(0, link);
            }
        }
    }
    chain
}

fn joined_straight(pieces: &[JoinPiece], chain: &[(usize, bool)]) -> Result<Curve, KernelError> {
    let mut points: Vec<Point> = Vec::new();
    for &(at, reversed) in chain {
        let mut run = pieces[at].curve.sample()?;
        if reversed {
            run.reverse();
        }
        let skip = usize::from(!points.is_empty());
        points.extend(run.into_iter().skip(skip));
    }
    let closed = points.len() > 3 && gap(points[0], points[points.len() - 1]) <= JOIN_TOLERANCE;
    if closed {
        points.pop();
    }
    Curve::polyline(points, closed)
}

fn joined_smooth(pieces: &[JoinPiece], chain: &[(usize, bool)]) -> Result<Curve, KernelError> {
    let mut segments = chain
        .iter()
        .map(|&(at, _)| match pieces[at].curve.to_nurbs()? {
            Curve::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => nurbs_data(degree, &knots, &points, &weights),
            _ => Err(KernelError("that curve cannot be joined".to_owned())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let senses: Vec<bool> = chain.iter().map(|&(_, reversed)| !reversed).collect();
    let engine =
        BSplineCurve::create_by_joining(&mut segments, Some(&senses)).map_err(engine_error)?;
    curve_from_engine(&engine)
}

fn joined_mixed(pieces: &[JoinPiece], chain: &[(usize, bool)]) -> Result<Curve, KernelError> {
    let mut segments = Vec::new();
    for &(at, reversed) in chain {
        let piece = if reversed {
            pieces[at].curve.reversed()
        } else {
            pieces[at].curve.clone()
        };
        match piece {
            Curve::Compound { segments: inner } => segments.extend(inner),
            single => segments.push(single),
        }
    }
    Curve::compound(segments)
}

/// Joins curves that meet end to end. Returns each new curve with the indexes of the curves it
/// replaces; curves that meet nothing are left out. Straight curves join into polylines and
/// smooth curves into NURBS curves. Any other mix keeps its parts in a compound curve.
pub fn join(curves: &[Curve]) -> Result<Vec<(Vec<usize>, Curve)>, KernelError> {
    let pieces: Vec<JoinPiece> = curves
        .iter()
        .enumerate()
        .filter_map(|(index, curve)| JoinPiece::of(index, curve))
        .collect();
    let mut used = vec![false; pieces.len()];
    let mut made = Vec::new();
    for seed in 0..pieces.len() {
        if used[seed] {
            continue;
        }
        let chain = chain_from(&pieces, &mut used, seed);
        if chain.len() < 2 {
            continue;
        }
        let all = |test: fn(&Curve) -> bool| chain.iter().all(|&(at, _)| test(&pieces[at].curve));
        let curve = if all(|c| matches!(c, Curve::Line { .. } | Curve::Polyline { .. })) {
            joined_straight(&pieces, &chain)?
        } else if all(|c| matches!(c, Curve::Interpolated { .. } | Curve::Nurbs { .. })) {
            joined_smooth(&pieces, &chain)?
        } else {
            joined_mixed(&pieces, &chain)?
        };
        let replaced = chain.iter().map(|&(at, _)| pieces[at].index).collect();
        made.push((replaced, curve));
    }
    Ok(made)
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

/// A change that keeps shapes the same: a turn, a mirror image or an even change of size, about a
/// point. Circles stay circles under all of these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Similarity {
    origin: Point,
    rows: [Point; 3],
    scale: f64,
}

impl Similarity {
    /// A turn by `angle` radians about a line through `origin` along the unit vector `axis`,
    /// by the Rodrigues rotation formula.
    #[must_use]
    pub fn rotation(origin: Point, axis: Point, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        let [x, y, z] = axis;
        let rest = 1.0 - cos;
        Self {
            origin,
            rows: [
                [
                    cos + x * x * rest,
                    x * y * rest - z * sin,
                    x * z * rest + y * sin,
                ],
                [
                    y * x * rest + z * sin,
                    cos + y * y * rest,
                    y * z * rest - x * sin,
                ],
                [
                    z * x * rest - y * sin,
                    z * y * rest + x * sin,
                    cos + z * z * rest,
                ],
            ],
            scale: 1.0,
        }
    }

    /// An even change of size about `origin`.
    #[must_use]
    pub fn scaling(origin: Point, factor: f64) -> Self {
        Self {
            origin,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            scale: factor,
        }
    }

    /// The turn about `origin` that takes the three points' frame `from` onto `to`: the first
    /// point to the second sets X, and the third sets which way Y faces. `None` when either set
    /// of points lies on one line.
    #[must_use]
    pub fn between_frames(origin: Point, from: [Point; 3], to: [Point; 3]) -> Option<Self> {
        let axes = |[a, b, c]: [Point; 3]| -> Option<[Point; 3]> {
            let x = unit(sub(b, a))?;
            let z = unit(cross(x, sub(c, a)))?;
            Some([x, cross(z, x), z])
        };
        let (from, to) = (axes(from)?, axes(to)?);
        let rows = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| to[k][i] * from[k][j]).sum())
        });
        Some(Self {
            origin,
            rows,
            scale: 1.0,
        })
    }

    /// A mirror image in the plane through `origin` facing along the unit vector `normal`.
    #[must_use]
    pub fn mirror(origin: Point, normal: Point) -> Self {
        let row = |i: usize| {
            let across = |j: usize| f64::from(u8::from(i == j)) - 2.0 * normal[i] * normal[j];
            [across(0), across(1), across(2)]
        };
        Self {
            origin,
            rows: [row(0), row(1), row(2)],
            scale: 1.0,
        }
    }

    /// Where a direction goes. Its length does not change.
    fn direction(&self, v: Point) -> Point {
        self.rows.map(|row| dot(row, v))
    }

    /// How much it resizes: 1 for a turn or a mirror.
    #[must_use]
    pub fn factor(&self) -> f64 {
        self.scale
    }

    /// How far it turns, in radians from 0 to π, read from its rotation.
    #[must_use]
    pub fn angle(&self) -> f64 {
        let trace = self.rows[0][0] + self.rows[1][1] + self.rows[2][2];
        ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos()
    }

    /// Where a point goes.
    #[must_use]
    pub fn point(&self, p: Point) -> Point {
        let turned = self.direction(sub(p, self.origin));
        [
            self.origin[0] + turned[0] * self.scale,
            self.origin[1] + turned[1] * self.scale,
            self.origin[2] + turned[2] * self.scale,
        ]
    }
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

    /// An arc about `centre` on the plane of two directions, from `start` counterclockwise to
    /// the direction of `end`. Its radius comes from `start`.
    pub fn arc(
        centre: Point,
        start: Point,
        end: Point,
        plane_x: Point,
        plane_y: Point,
    ) -> Result<Self, KernelError> {
        let bad = || KernelError("those points do not make an arc".to_owned());
        let normal = unit(cross(plane_x, plane_y)).ok_or_else(bad)?;
        let from_centre = flattened(sub(start, centre), normal);
        let x = unit(from_centre).ok_or_else(bad)?;
        let y = cross(normal, x);
        let to_end = unit(flattened(sub(end, centre), normal)).ok_or_else(bad)?;
        let mut sweep = dot(to_end, y).atan2(dot(to_end, x));
        if sweep < 0.0 {
            sweep += TAU;
        }
        if !(ANGLE_TOLERANCE..=TAU - ANGLE_TOLERANCE).contains(&sweep) {
            return Err(bad());
        }
        Ok(Self::Arc {
            centre,
            radius: dot(from_centre, from_centre).sqrt(),
            x,
            y,
            sweep,
        })
    }

    /// The same curve turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let many = |list: &[Point]| list.iter().map(|p| change.point(*p)).collect();
        match self {
            Self::Point { at } => Self::Point {
                at: change.point(*at),
            },
            Self::Line { start, end } => Self::Line {
                start: change.point(*start),
                end: change.point(*end),
            },
            Self::Polyline { points, closed } => Self::Polyline {
                points: many(points),
                closed: *closed,
            },
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => Self::Circle {
                centre: change.point(*centre),
                radius: radius * change.scale,
                x: change.direction(*x),
                y: change.direction(*y),
            },
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => Self::Arc {
                centre: change.point(*centre),
                radius: radius * change.scale,
                x: change.direction(*x),
                y: change.direction(*y),
                sweep: *sweep,
            },
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => Self::Nurbs {
                degree: *degree,
                knots: knots.clone(),
                points: many(points),
                weights: weights.clone(),
            },
            Self::Compound { segments } => Self::Compound {
                segments: segments.iter().map(|s| s.mapped(change)).collect(),
            },
            Self::Interpolated { points, closed } => Self::Interpolated {
                points: many(points),
                closed: *closed,
            },
        }
    }

    /// A NURBS curve, checked by building it now. Leave `weights` empty when every weight is 1.
    pub fn nurbs(
        degree: usize,
        knots: Vec<f64>,
        points: Vec<Point>,
        weights: Vec<f64>,
    ) -> Result<Self, KernelError> {
        let bad = |why: &str| Err(KernelError(format!("not a valid NURBS curve: {why}")));
        if degree == 0 || points.len() <= degree {
            return bad("too few control points for its degree");
        }
        if knots.len() != points.len() + degree + 1 {
            return bad("wrong number of knots");
        }
        if knots.iter().any(|k| !k.is_finite()) || knots.windows(2).any(|pair| pair[0] > pair[1]) {
            return bad("the knots must not decrease");
        }
        if !weights.is_empty()
            && (weights.len() != points.len()
                || weights.iter().any(|w| !w.is_finite() || *w <= 0.0))
        {
            return bad("each control point needs one positive weight");
        }
        nurbs_engine_curve(degree, &knots, &points, &weights)?;
        Ok(Self::Nurbs {
            degree,
            knots,
            points,
            weights,
        })
    }

    /// The same shape as an exact NURBS curve, for the smooth kinds that can be cut.
    pub fn to_nurbs(&self) -> Result<Self, KernelError> {
        match self {
            Self::Nurbs { .. } => Ok(self.clone()),
            Self::Interpolated { points, closed } => {
                let engine = spline_data(points, *closed)?;
                curve_from_engine(&engine)
            }
            _ => Err(KernelError(
                "only smooth curves can be turned into NURBS for now".to_owned(),
            )),
        }
    }

    /// A curve made of parts that meet end to end, each keeping its own kind, like a Rhino
    /// polycurve. Parts that are themselves compound are flattened.
    pub fn compound(parts: Vec<Self>) -> Result<Self, KernelError> {
        let mut segments = Vec::new();
        for part in parts {
            match part {
                Self::Compound { segments: inner } => segments.extend(inner),
                single => segments.push(single),
            }
        }
        if segments.len() < 2 {
            return Err(KernelError("a compound curve needs two parts".to_owned()));
        }
        let mut previous: Option<Point> = None;
        for segment in &segments {
            let points = segment.sample()?;
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                return Err(KernelError("a part of the curve is empty".to_owned()));
            };
            if previous.is_some_and(|end| gap(end, *first) > JOIN_TOLERANCE) {
                return Err(KernelError("the parts do not meet end to end".to_owned()));
            }
            previous = Some(*last);
        }
        Ok(Self::Compound { segments })
    }

    /// The same curve running the other way.
    #[must_use]
    pub fn reversed(&self) -> Self {
        match self {
            Self::Point { .. } => self.clone(),
            Self::Line { start, end } => Self::Line {
                start: *end,
                end: *start,
            },
            Self::Polyline { points, closed } => Self::Polyline {
                points: points.iter().rev().copied().collect(),
                closed: *closed,
            },
            Self::Circle { .. } => self.clone(),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => {
                let (sin, cos) = sweep.sin_cos();
                let mix = |a: f64, b: f64| {
                    let at = |axis: usize| x[axis] * a + y[axis] * b;
                    [at(0), at(1), at(2)]
                };
                Self::Arc {
                    centre: *centre,
                    radius: *radius,
                    x: mix(cos, sin),
                    y: mix(sin, -cos),
                    sweep: *sweep,
                }
            }
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => {
                let span =
                    knots.first().copied().unwrap_or(0.0) + knots.last().copied().unwrap_or(0.0);
                Self::Nurbs {
                    degree: *degree,
                    knots: knots.iter().rev().map(|k| span - k).collect(),
                    points: points.iter().rev().copied().collect(),
                    weights: weights.iter().rev().copied().collect(),
                }
            }
            Self::Interpolated { points, closed } => Self::Interpolated {
                points: points.iter().rev().copied().collect(),
                closed: *closed,
            },
            Self::Compound { segments } => Self::Compound {
                segments: segments.iter().rev().map(Self::reversed).collect(),
            },
        }
    }

    /// Points at equal distances along the curve, exactly on it. A closed curve does not repeat
    /// its start.
    pub fn divided(&self, segments: usize) -> Result<Vec<Point>, KernelError> {
        let path = self.sample()?;
        let lengths: Vec<f64> = path.windows(2).map(|p| gap(p[0], p[1])).collect();
        let total: f64 = lengths.iter().sum();
        if total <= 0.0 || segments == 0 {
            return Err(KernelError("that curve cannot be divided".to_owned()));
        }
        let closed = gap(path[0], path[path.len() - 1]) <= JOIN_TOLERANCE;
        let count = if closed { segments } else { segments + 1 };
        let mut found = Vec::with_capacity(count);
        let (mut index, mut before) = (0, 0.0);
        for step in 0..count {
            let wanted = total * step as f64 / segments as f64;
            while index + 1 < lengths.len() && before + lengths[index] < wanted {
                before += lengths[index];
                index += 1;
            }
            let t = ((wanted - before) / lengths[index].max(1e-24)).clamp(0.0, 1.0);
            let a = path[index];
            let b = path[index + 1];
            let on = [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ];
            found.push(self.closest_point(on)?);
        }
        Ok(found)
    }
    /// The separate curves this one is made of, or `None` if it is a single piece.
    #[must_use]
    pub fn exploded(&self) -> Option<Vec<Self>> {
        match self {
            Self::Compound { segments } => Some(segments.clone()),
            Self::Polyline { .. } => {
                let points = self.sample().ok()?;
                let lines: Vec<Self> = points
                    .windows(2)
                    .filter_map(|pair| Self::line(pair[0], pair[1]).ok())
                    .collect();
                (lines.len() >= 2).then_some(lines)
            }
            _ => None,
        }
    }

    /// A smooth curve through the points, checked by building it now.
    pub fn interpolated(points: Vec<Point>, closed: bool) -> Result<Self, KernelError> {
        if points.len() < 2 {
            return Err(KernelError("a curve needs two points".to_owned()));
        }
        spline(&points, closed)?;
        Ok(Self::Interpolated { points, closed })
    }

    /// A control-point curve, as Rhino's `Curve` draws it. Open, it is clamped: it starts on
    /// the first point and ends on the last. Closed, it is smooth all the way round, with no
    /// kink where it joins. The degree is lowered when there are too few points for it.
    pub fn control_point(
        points: Vec<Point>,
        degree: usize,
        closed: bool,
    ) -> Result<Self, KernelError> {
        if points.len() < 2 || (closed && points.len() < 3) {
            return Err(KernelError("too few points for a curve".to_owned()));
        }
        let degree = degree.clamp(1, points.len() - 1);
        if closed {
            // Wrapping the first `degree` points round, with evenly spaced knots, gives a
            // periodic curve; it is then cut to its complete span as an ordinary clamped one.
            let count = points.len();
            let mut wrapped = points;
            wrapped.extend_from_within(..degree);
            let knots: Vec<f64> = (0..wrapped.len() + degree + 1).map(|i| i as f64).collect();
            let (knots, points) = clamp_span(
                degree,
                knots,
                wrapped,
                degree as f64,
                (degree + count) as f64,
            );
            return Self::nurbs(degree, knots, points, Vec::new());
        }
        let inner = points.len() - degree;
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..inner).map(|i| i as f64 / inner as f64));
        knots.extend(std::iter::repeat_n(1.0, degree + 1));
        Self::nurbs(degree, knots, points, Vec::new())
    }

    /// The curve's length in millimetres.
    pub fn length(&self) -> Result<f64, KernelError> {
        Ok(match self {
            Self::Point { .. } => 0.0,
            Self::Line { start, end } => gap(*start, *end),
            Self::Polyline { .. } => self
                .sample()?
                .windows(2)
                .map(|pair| gap(pair[0], pair[1]))
                .sum(),
            Self::Circle { radius, .. } => TAU * radius,
            Self::Arc { radius, sweep, .. } => radius * sweep,
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => {
                let kernel = Kernel::default();
                let curve = nurbs_engine_curve(*degree, knots, points, weights)?;
                curve
                    .length(&kernel, &curve.natural_interval(), LENGTH_ACCURACY)
                    .map_err(engine_error)?
            }
            Self::Compound { segments } => {
                let mut total = 0.0;
                for segment in segments {
                    total += segment.length()?;
                }
                total
            }
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
            Self::Point { at } => vec![*at],
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
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => {
                let steps = ((CIRCLE_SAMPLES as f64 * sweep / TAU).ceil() as usize).max(2);
                (0..=steps)
                    .map(|step| {
                        let angle = sweep * step as f64 / steps as f64;
                        let (cos, sin) = (angle.cos() * radius, angle.sin() * radius);
                        let around = |axis: usize| centre[axis] + x[axis] * cos + y[axis] * sin;
                        [around(0), around(1), around(2)]
                    })
                    .collect()
            }
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => sampled(
                &nurbs_engine_curve(*degree, knots, points, weights)?,
                SAMPLES_PER_POINT * points.len(),
            )?,
            Self::Compound { segments } => {
                let mut out: Vec<Point> = Vec::new();
                for segment in segments {
                    let skip = usize::from(!out.is_empty());
                    out.extend(segment.sample()?.into_iter().skip(skip));
                }
                out
            }
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
            Self::Point { at } => Self::Point {
                at: shifted(*at, by),
            },
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
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => Self::Arc {
                centre: shifted(*centre, by),
                radius: *radius,
                x: *x,
                y: *y,
                sweep: *sweep,
            },
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => Self::Nurbs {
                degree: *degree,
                knots: knots.clone(),
                points: moved(points),
                weights: weights.clone(),
            },
            Self::Compound { segments } => Self::Compound {
                segments: segments.iter().map(|s| s.translated(by)).collect(),
            },
            Self::Interpolated { points, closed } => Self::Interpolated {
                points: moved(points),
                closed: *closed,
            },
        }
    }
}

/// Cuts a B-spline to the part between knot values `start` and `end`, as a clamped B-spline
/// with knots from 0 to 1: knots are inserted at both ends until each is repeated `degree`
/// times (Boehm's algorithm), which leaves the shape unchanged, then the span is kept.
fn clamp_span(
    degree: usize,
    mut knots: Vec<f64>,
    mut points: Vec<Point>,
    start: f64,
    end: f64,
) -> (Vec<f64>, Vec<Point>) {
    let same = |a: f64, b: f64| (a - b).abs() < 1e-12;
    for at in [start, end] {
        let present = knots.iter().filter(|k| same(**k, at)).count();
        for _ in present..degree {
            // The span holding `at`: the last knot not after it.
            let span = knots.iter().rposition(|k| *k <= at).unwrap_or(degree);
            let mut next = Vec::with_capacity(points.len() + 1);
            for i in 0..=points.len() {
                if i + degree <= span {
                    next.push(points[i]);
                } else if i > span {
                    next.push(points[i - 1]);
                } else {
                    let a = (at - knots[i]) / (knots[i + degree] - knots[i]);
                    // At the very end of the curve `a` is 0 and the point after does not exist.
                    let p = points[i - 1];
                    let q = points.get(i).copied().unwrap_or(p);
                    next.push(std::array::from_fn(|axis| {
                        (1.0 - a) * p[axis] + a * q[axis]
                    }));
                }
            }
            knots.insert(span + 1, at);
            points = next;
        }
    }
    let first = knots.iter().position(|k| same(*k, start)).unwrap_or(1);
    let last = knots
        .iter()
        .rposition(|k| same(*k, end))
        .unwrap_or(knots.len() - 1);
    let mut kept = vec![start];
    kept.extend_from_slice(&knots[first..=last]);
    kept.push(end);
    let kept_points = points[first - 1..last + 1 - degree].to_vec();
    let scaled = kept.iter().map(|k| (k - start) / (end - start)).collect();
    (scaled, kept_points)
}

#[cfg(test)]
mod control_point_tests {
    use super::*;

    fn ends(curve: &Curve) -> (Point, Point) {
        let path = curve.sample().unwrap();
        (path[0], path[path.len() - 1])
    }

    #[test]
    fn an_open_curve_starts_and_ends_on_its_end_points() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [5.0, 8.0, 0.0],
            [10.0, -3.0, 0.0],
            [15.0, 2.0, 0.0],
        ];
        let curve = Curve::control_point(points, 3, false).unwrap();
        let Curve::Nurbs {
            points: kept,
            degree,
            ..
        } = &curve
        else {
            panic!("not NURBS")
        };
        assert_eq!((kept.len(), *degree), (4, 3));
        let (first, last) = ends(&curve);
        assert!(gap(first, [0.0, 0.0, 0.0]) < 1e-6 && gap(last, [15.0, 2.0, 0.0]) < 1e-6);
    }

    #[test]
    fn closed_curves_close_for_every_degree() {
        let hexagon: Vec<Point> = (0..6)
            .map(|i| {
                let turn = f64::from(i) * std::f64::consts::TAU / 6.0;
                [10.0 * turn.cos(), 10.0 * turn.sin(), f64::from(i % 2)]
            })
            .collect();
        for degree in 1..=7 {
            let curve = Curve::control_point(hexagon.clone(), degree, true).unwrap();
            let (first, last) = ends(&curve);
            assert!(
                gap(first, last) < 1e-6,
                "degree {degree}: {first:?} {last:?}"
            );
        }
    }

    #[test]
    fn too_few_points_lower_the_degree() {
        let curve = Curve::control_point(vec![[0.0; 3], [4.0, 1.0, 0.0]], 3, false).unwrap();
        assert!(matches!(curve, Curve::Nurbs { degree: 1, .. }));
    }

    #[test]
    fn a_closed_cubic_starts_where_a_uniform_b_spline_does_and_closes() {
        let square = vec![
            [0.0, 0.0, 0.0],
            [6.0, 0.0, 0.0],
            [6.0, 6.0, 0.0],
            [0.0, 6.0, 0.0],
        ];
        let curve = Curve::control_point(square.clone(), 3, true).unwrap();
        let (first, last) = ends(&curve);
        assert!(gap(first, last) < 1e-6, "{first:?} {last:?}");
        // A uniform cubic B-spline over points a, b, c, d starts at (a + 4b + c) / 6.
        let expected: Point =
            std::array::from_fn(|i| (square[0][i] + 4.0 * square[1][i] + square[2][i]) / 6.0);
        assert!(gap(first, expected) < 1e-6, "{first:?} vs {expected:?}");
        // Symmetric points give a curve that stays inside them and keeps away from the corners.
        for point in curve.sample().unwrap() {
            assert!((0.0..=6.0).contains(&point[0]) && (0.0..=6.0).contains(&point[1]));
            assert!(gap(point, [0.0, 0.0, 0.0]) > 1.0);
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

    fn points_near(a: Point, b: Point) -> bool {
        gap(a, b) < CLOSE
    }

    const X: Point = [1.0, 0.0, 0.0];
    const Y: Point = [0.0, 1.0, 0.0];
    const Z: Point = [0.0, 0.0, 1.0];

    #[test]
    fn a_rotation_turns_counterclockwise() {
        let turn = Similarity::rotation([0.0; 3], Z, std::f64::consts::FRAC_PI_2);
        assert!(points_near(turn.point([1.0, 0.0, 5.0]), [0.0, 1.0, 5.0]));
    }

    #[test]
    fn a_rotation_is_about_its_origin() {
        let turn = Similarity::rotation([2.0, 0.0, 0.0], Z, std::f64::consts::PI);
        assert!(points_near(turn.point([3.0, 0.0, 0.0]), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn scaling_is_about_its_origin() {
        let bigger = Similarity::scaling([1.0, 1.0, 1.0], 3.0);
        assert!(points_near(bigger.point([2.0, 1.0, 1.0]), [4.0, 1.0, 1.0]));
    }

    #[test]
    fn a_mirror_reflects_across_its_plane() {
        let mirror = Similarity::mirror([5.0, 0.0, 0.0], X);
        assert!(points_near(mirror.point([3.0, 7.0, 1.0]), [7.0, 7.0, 1.0]));
    }

    #[test]
    fn scaling_a_circle_scales_its_radius_only() {
        let circle = Curve::circle([2.0, 0.0, 0.0], 3.0).unwrap();
        let bigger = circle.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert_eq!(bigger, Curve::circle([4.0, 0.0, 0.0], 6.0).unwrap());
    }

    #[test]
    fn turning_a_flat_circle_on_edge_keeps_it_a_circle() {
        let circle = Curve::circle([0.0; 3], 3.0).unwrap();
        let turned = circle.mapped(&Similarity::rotation(
            [0.0; 3],
            X,
            std::f64::consts::FRAC_PI_2,
        ));
        for point in turned.sample().unwrap() {
            assert!(point[1].abs() < CLOSE);
            assert!(near(point[0].hypot(point[2]), 3.0));
        }
    }

    #[test]
    fn an_arc_goes_counterclockwise_from_start_to_end() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 9.0, 0.0], X, Y).unwrap();
        assert!(near(
            arc.length().unwrap(),
            5.0 * std::f64::consts::FRAC_PI_2
        ));
        let points = arc.sample().unwrap();
        assert!(points_near(points[0], [5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
    }

    #[test]
    fn an_arc_can_turn_past_half_way() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, -1.0, 0.0], X, Y).unwrap();
        assert!(near(
            arc.length().unwrap(),
            5.0 * 3.0 * std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn an_arc_lies_on_its_plane() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 0.0, 5.0], X, Z).unwrap();
        assert!(arc.sample().unwrap().iter().all(|p| p[1].abs() < CLOSE));
    }

    #[test]
    fn an_arc_needs_a_real_radius_and_turn() {
        assert!(Curve::arc([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0], X, Y).is_err());
        assert!(Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [9.0, 0.0, 0.0], X, Y).is_err());
    }

    #[test]
    fn mirroring_an_arc_keeps_its_ends_mirrored() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let mirrored = arc.mapped(&Similarity::mirror([0.0; 3], X));
        let points = mirrored.sample().unwrap();
        assert!(points_near(points[0], [-5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
    }

    #[test]
    fn moving_an_arc_keeps_its_shape() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let moved = arc.translated([1.0, 0.0, 0.0]);
        assert!(points_near(moved.sample().unwrap()[0], [6.0, 0.0, 0.0]));
    }

    fn quarter_circle() -> Curve {
        let corner_weight = std::f64::consts::FRAC_1_SQRT_2;
        Curve::nurbs(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            vec![1.0, corner_weight, 1.0],
        )
        .unwrap()
    }

    fn smooth_curve() -> Curve {
        Curve::interpolated(
            vec![
                [0.0; 3],
                [10.0, 5.0, 0.0],
                [20.0, -3.0, 4.0],
                [30.0, 0.0, 0.0],
            ],
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_rational_nurbs_curve_can_be_an_exact_quarter_circle() {
        let curve = quarter_circle();
        for point in curve.sample().unwrap() {
            assert!(near(point[0].hypot(point[1]), 1.0), "{point:?}");
        }
        assert!(near(curve.length().unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn a_nurbs_curve_starts_and_ends_on_its_end_control_points() {
        let points = quarter_circle().sample().unwrap();
        assert!(points_near(points[0], X));
        assert!(points_near(*points.last().unwrap(), Y));
    }

    #[test]
    fn turning_a_smooth_curve_into_nurbs_keeps_its_shape() {
        let original = smooth_curve();
        let exact = original.to_nurbs().unwrap();
        assert!(matches!(exact, Curve::Nurbs { .. }));
        assert!(near(exact.length().unwrap(), original.length().unwrap()));
        let points = exact.sample().unwrap();
        assert!(points_near(points[0], [0.0; 3]));
        assert!(points_near(*points.last().unwrap(), [30.0, 0.0, 0.0]));
        let through = [10.0, 5.0, 0.0];
        let closest = points;
        assert!(closest.iter().any(|p| gap(*p, through) < 0.5));
    }

    #[test]
    fn a_closed_smooth_curve_turns_into_nurbs_too() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let original = Curve::interpolated(points, true).unwrap();
        let exact = original.to_nurbs().unwrap();
        assert!(near(exact.length().unwrap(), original.length().unwrap()));
    }

    #[test]
    fn lines_do_not_turn_into_nurbs_yet() {
        assert!(Curve::line([0.0; 3], X).unwrap().to_nurbs().is_err());
    }

    #[test]
    fn moving_a_nurbs_curve_moves_it_exactly() {
        let moved = quarter_circle().translated([5.0, 0.0, 0.0]);
        assert!(points_near(moved.sample().unwrap()[0], [6.0, 0.0, 0.0]));
        assert!(near(moved.length().unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn resizing_and_turning_a_nurbs_curve_keeps_its_weights() {
        let bigger = quarter_circle().mapped(&Similarity::scaling([0.0; 3], 3.0));
        assert!(near(
            bigger.length().unwrap(),
            3.0 * std::f64::consts::FRAC_PI_2
        ));
        let turned = quarter_circle().mapped(&Similarity::rotation(
            [0.0; 3],
            Z,
            std::f64::consts::FRAC_PI_2,
        ));
        assert!(points_near(turned.sample().unwrap()[0], Y));
        for point in turned.sample().unwrap() {
            assert!(near(point[0].hypot(point[1]), 1.0));
        }
    }

    #[test]
    fn bad_nurbs_data_is_refused() {
        let points = vec![[0.0; 3], X, Y];
        let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        assert!(Curve::nurbs(0, knots.clone(), points.clone(), vec![]).is_err());
        assert!(Curve::nurbs(2, vec![0.0, 1.0], points.clone(), vec![]).is_err());
        let falling = vec![0.0, 0.0, 1.0, 0.5, 1.0, 1.0];
        assert!(Curve::nurbs(2, falling, points.clone(), vec![]).is_err());
        assert!(Curve::nurbs(2, knots.clone(), points.clone(), vec![1.0]).is_err());
        assert!(Curve::nurbs(2, knots, points, vec![1.0, -1.0, 1.0]).is_err());
    }

    #[test]
    fn a_nurbs_curve_survives_being_saved_as_text() {
        let curve = quarter_circle();
        let text = serde_json::to_string(&curve).unwrap();
        let back: Curve = serde_json::from_str(&text).unwrap();
        assert_eq!(back, curve);
    }

    #[test]
    fn lines_meeting_end_to_end_join_into_one_polyline() {
        let a = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([1.0, 0.0, 0.0], [1.0, 1.0, 0.0]).unwrap();
        let made = join(&[a, b]).unwrap();
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].0, [0, 1]);
        assert_eq!(
            made[0].1,
            Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], false).unwrap()
        );
    }

    #[test]
    fn join_flips_pieces_and_finds_them_in_any_order() {
        let a = Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([3.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
        let c = Curve::line([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]).unwrap();
        let made = join(&[a, b, c]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(made[0].1.length().unwrap(), 3.0));
    }

    #[test]
    fn lines_forming_a_loop_join_closed() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line(X, Y).unwrap();
        let c = Curve::line(Y, [0.0; 3]).unwrap();
        let made = join(&[a, b, c]).unwrap();
        assert!(matches!(made[0].1, Curve::Polyline { closed: true, .. }));
        assert!(near(made[0].1.length().unwrap(), 2.0 + 2.0_f64.sqrt()));
    }

    #[test]
    fn curves_that_do_not_meet_are_left_alone() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap();
        assert!(join(&[a, b]).unwrap().is_empty());
    }

    #[test]
    fn smooth_curves_join_with_their_lengths_adding_up() {
        let a =
            Curve::interpolated(vec![[0.0; 3], [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let b = Curve::interpolated(
            vec![[10.0, 0.0, 0.0], [15.0, -3.0, 0.0], [20.0, 0.0, 0.0]],
            false,
        )
        .unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!(matches!(made[0].1, Curve::Nurbs { .. }));
        assert!((made[0].1.length().unwrap() - total).abs() < 1e-4);
        let points = made[0].1.sample().unwrap();
        assert!(points_near(points[0], [0.0; 3]));
        assert!(points_near(*points.last().unwrap(), [20.0, 0.0, 0.0]));
    }

    #[test]
    fn a_reversed_smooth_curve_joins_the_right_way_round() {
        let a =
            Curve::interpolated(vec![[0.0; 3], [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let b = Curve::interpolated(
            vec![[20.0, 0.0, 0.0], [15.0, -3.0, 0.0], [10.0, 0.0, 0.0]],
            false,
        )
        .unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!((made[0].1.length().unwrap() - total).abs() < 1e-4);
    }

    fn line_then_arc() -> (Curve, Curve) {
        let line = Curve::line([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]).unwrap();
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        (line, arc)
    }

    #[test]
    fn a_line_and_an_arc_join_into_a_compound_curve_that_keeps_them() {
        let (line, arc) = line_then_arc();
        let total = line.length().unwrap() + arc.length().unwrap();
        let made = join(&[line.clone(), arc.clone()]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(made[0].1.length().unwrap(), total));
        assert_eq!(made[0].1.exploded().unwrap(), [line, arc]);
    }

    #[test]
    fn a_line_and_a_smooth_curve_join_into_a_compound_curve() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::interpolated(vec![X, [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!(matches!(made[0].1, Curve::Compound { .. }));
        assert!(near(made[0].1.length().unwrap(), total));
    }

    #[test]
    fn a_compound_curve_runs_continuously() {
        let (line, arc) = line_then_arc();
        let curve = Curve::compound(vec![line, arc]).unwrap();
        let points = curve.sample().unwrap();
        assert!(points_near(points[0], [-5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
        assert!(points.windows(2).all(|pair| gap(pair[0], pair[1]) > 0.0));
    }

    #[test]
    fn parts_that_do_not_meet_cannot_make_a_compound_curve() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap();
        assert!(Curve::compound(vec![a.clone(), b]).is_err());
        assert!(Curve::compound(vec![a]).is_err());
    }

    #[test]
    fn a_compound_curve_moves_turns_and_resizes_as_a_whole() {
        let (line, arc) = line_then_arc();
        let curve = Curve::compound(vec![line, arc]).unwrap();
        let moved = curve.translated([0.0, 0.0, 7.0]);
        assert!(moved.sample().unwrap().iter().all(|p| near(p[2], 7.0)));
        let bigger = curve.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert!(near(
            bigger.length().unwrap(),
            2.0 * curve.length().unwrap()
        ));
    }

    #[test]
    fn a_reversed_curve_keeps_its_shape_and_swaps_its_ends() {
        let (line, arc) = line_then_arc();
        let smooth = smooth_curve();
        let exact = quarter_circle();
        let joined = Curve::compound(vec![line.clone(), arc.clone()]).unwrap();
        for curve in [line, arc, smooth, exact, joined] {
            let forward = curve.sample().unwrap();
            let back = curve.reversed().sample().unwrap();
            assert!(near(
                curve.reversed().length().unwrap(),
                curve.length().unwrap()
            ));
            assert!(points_near(back[0], *forward.last().unwrap()), "{curve:?}");
            assert!(points_near(*back.last().unwrap(), forward[0]), "{curve:?}");
        }
    }

    #[test]
    fn an_arc_joins_when_it_has_to_be_flipped() {
        let (line, arc) = line_then_arc();
        let made = join(&[line.reversed(), arc.reversed()]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(
            made[0].1.length().unwrap(),
            line.length().unwrap() + arc.length().unwrap()
        ));
    }

    #[test]
    fn a_compound_curve_joins_on_as_one_flat_list() {
        let (line, arc) = line_then_arc();
        let first = Curve::compound(vec![line, arc]).unwrap();
        let next = Curve::line([0.0, 5.0, 0.0], [0.0, 9.0, 0.0]).unwrap();
        let made = join(&[first, next]).unwrap();
        assert_eq!(made[0].1.exploded().unwrap().len(), 3);
    }

    #[test]
    fn a_polyline_explodes_into_lines() {
        let polyline = Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], false).unwrap();
        assert_eq!(polyline.exploded().unwrap().len(), 2);
    }

    #[test]
    fn a_closed_polyline_explodes_into_every_side() {
        let polyline = Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], true).unwrap();
        assert_eq!(polyline.exploded().unwrap().len(), 3);
    }

    #[test]
    fn a_single_curve_does_not_explode() {
        assert!(Curve::line([0.0; 3], X).unwrap().exploded().is_none());
        assert!(smooth_curve().exploded().is_none());
    }

    #[test]
    fn a_point_moves_turns_and_has_no_length() {
        let point = Curve::Point { at: X };
        assert_eq!(point.length().unwrap(), 0.0);
        assert_eq!(point.sample().unwrap(), [X]);
        assert_eq!(
            point.translated(Y),
            Curve::Point {
                at: [1.0, 1.0, 0.0]
            }
        );
        let bigger = point.mapped(&Similarity::scaling([0.0; 3], 3.0));
        assert_eq!(
            bigger,
            Curve::Point {
                at: [3.0, 0.0, 0.0]
            }
        );
    }

    #[test]
    fn a_circle_divides_exactly_on_itself() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let found = circle.divided(4).unwrap();
        assert_eq!(found.len(), 4);
        assert!(points_near(found[1], [0.0, 5.0, 0.0]), "{:?}", found[1]);
        assert!(found.iter().all(|p| near(p[0].hypot(p[1]), 5.0)));
    }

    #[test]
    fn a_polyline_divides_by_distance_along_it() {
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let found = Curve::polyline(path, false).unwrap().divided(2).unwrap();
        assert!(points_near(found[1], [10.0, 0.0, 0.0]));
        assert!(points_near(found[2], [10.0, 10.0, 0.0]));
    }

    #[test]
    fn a_point_cannot_be_divided() {
        assert!(Curve::Point { at: X }.divided(2).is_err());
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
