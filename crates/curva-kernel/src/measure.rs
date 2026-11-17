//! Control-point curves, and measuring, sampling and moving curves.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Curve {
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
pub(crate) fn clamp_span(
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
