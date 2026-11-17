//! The curve, and making, reshaping and fitting curves.

#[allow(clippy::wildcard_imports)]
use super::*;

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
        /// The plane's X direction, of length 1.
        x: Point,
        /// The plane's Y direction, of length 1, at a right angle to `x`.
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

impl Curve {
    /// About how many bytes it holds beyond its own size, for weighing undo steps.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let point = std::mem::size_of::<Point>();
        match self {
            Self::Point { .. } | Self::Line { .. } | Self::Circle { .. } | Self::Arc { .. } => 0,
            Self::Polyline { points, .. } | Self::Interpolated { points, .. } => {
                points.len() * point
            }
            Self::Nurbs {
                knots,
                points,
                weights,
                ..
            } => points.len() * point + (knots.len() + weights.len()) * 8,
            Self::Compound { segments } => segments
                .iter()
                .map(|s| std::mem::size_of::<Self>() + s.heap_bytes())
                .sum(),
        }
    }

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

    /// A smooth curve through the points as a fitted spline (AutoCAD's SPLINE by fit points)
    /// describes it: optional vectors at its two ends and a way of spacing the points along it.
    ///
    /// Open, it is the cubic spline whose knots are the points' parameters, running through
    /// the summed steps themselves (distances, their square roots, or ones), with each end
    /// vector as the derivative there and no bending at an end without one (a natural end),
    /// as AutoCAD's are read by Rhino and the Open Design Alliance. It is kept as exact NURBS, since an interpolated
    /// curve holds only its points. Closed, it is smooth at its seam and takes no end vectors.
    pub fn fitted(
        points: Vec<Point>,
        closed: bool,
        start: Option<Point>,
        end: Option<Point>,
        spacing: Spacing,
    ) -> Result<Self, KernelError> {
        if points.len() < 2 {
            return Err(KernelError("a curve needs two points".to_owned()));
        }
        if closed && spacing == Spacing::Chord {
            return Self::interpolated(points, closed);
        }
        let method = match spacing {
            Spacing::Chord => ParamMethod::ChordLength,
            Spacing::SquareRoot => ParamMethod::Centripetal,
            Spacing::Uniform => ParamMethod::Uniform,
        };
        let mut through: Vec<Point3> = points
            .iter()
            .map(|p| Point3::new(p[0], p[1], p[2]))
            .collect();
        if closed {
            through.push(through[0]);
            let engine = BSplineCurve::interpolate_points(
                &through,
                None,
                3.min(points.len()),
                None,
                None,
                true,
                method,
            )
            .map_err(engine_error)?;
            return curve_from_engine(&engine);
        }
        let steps = spacing.steps(&points);
        if steps.iter().any(|h| *h <= 1e-12) {
            return Err(KernelError(
                "a fitted curve's points must differ".to_owned(),
            ));
        }
        let given = |d: Option<Point>| d.filter(|v| gap(*v, [0.0; 3]) > 1e-12);
        let (first, last) = spline_ends(&points, &steps, given(start), given(end));
        // The engine's parameter runs from 0 to 1, so the derivatives grow by the whole span.
        let span: f64 = steps.iter().sum();
        let scaled = |v: Point| Vec3::new(v[0], v[1], v[2]) * span;
        let vectors: Vec<Vec3> = through.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect();
        let cubic = cubic_spline_interp(&vectors, scaled(first), scaled(last), method)
            .map_err(engine_error)?;
        curve_from_engine(&BSplineCurve::from_gw_nurb(3, &cubic))
    }
}

/// How a fitted curve spaces its points along itself (AutoCAD's knot parameterization).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spacing {
    /// In proportion to the distances between points.
    Chord,
    /// In proportion to the square roots of those distances.
    SquareRoot,
    /// Evenly, whatever the distances.
    Uniform,
}

impl Spacing {
    /// The parameter's steps between the points.
    pub(crate) fn steps(self, points: &[Point]) -> Vec<f64> {
        points
            .windows(2)
            .map(|pair| match self {
                Self::Chord => gap(pair[0], pair[1]),
                Self::SquareRoot => gap(pair[0], pair[1]).sqrt(),
                Self::Uniform => 1.0,
            })
            .collect()
    }
}

/// The end derivatives of the cubic spline through `points` (C2 throughout) whose parameter
/// steps are `steps`: an end given keeps its derivative, and one not given has no second
/// derivative there (a natural end). Solved as the spline's tridiagonal system for the
/// derivatives at every point.
pub(crate) fn spline_ends(
    points: &[Point],
    steps: &[f64],
    start: Option<Point>,
    end: Option<Point>,
) -> (Point, Point) {
    let n = points.len() - 1;
    let slope = |i: usize| -> Point {
        std::array::from_fn(|k| (points[i + 1][k] - points[i][k]) / steps[i])
    };
    // Row i: lower * m[i-1] + diagonal * m[i] + upper * m[i+1] = right.
    let mut lower = vec![0.0; n + 1];
    let mut diagonal = vec![0.0; n + 1];
    let mut upper = vec![0.0; n + 1];
    let mut right = vec![[0.0; 3]; n + 1];
    match start {
        Some(d) => (diagonal[0], right[0]) = (1.0, d),
        None => (diagonal[0], upper[0], right[0]) = (2.0, 1.0, slope(0).map(|c| 3.0 * c)),
    }
    for i in 1..n {
        let (before, after) = (steps[i - 1], steps[i]);
        let (s0, s1) = (slope(i - 1), slope(i));
        lower[i] = after;
        diagonal[i] = 2.0 * (before + after);
        upper[i] = before;
        right[i] = std::array::from_fn(|k| 3.0 * (after * s0[k] + before * s1[k]));
    }
    match end {
        Some(d) => (diagonal[n], right[n]) = (1.0, d),
        None => {
            (lower[n], diagonal[n], right[n]) = (1.0, 2.0, slope(n - 1).map(|c| 3.0 * c));
        }
    }
    // Forward sweep, then back substitution.
    for i in 1..=n {
        let factor = lower[i] / diagonal[i - 1];
        diagonal[i] -= factor * upper[i - 1];
        let above = right[i - 1];
        for k in 0..3 {
            right[i][k] -= factor * above[k];
        }
    }
    let mut m = vec![[0.0; 3]; n + 1];
    m[n] = right[n].map(|c| c / diagonal[n]);
    for i in (0..n).rev() {
        m[i] = std::array::from_fn(|k| (right[i][k] - upper[i] * m[i + 1][k]) / diagonal[i]);
    }
    (m[0], m[n])
}
