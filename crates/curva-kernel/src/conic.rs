//! Exact ellipses and elliptical arcs as rational curves, uneven stretching, and chamfers.

use crate::{Curve, KernelError, Point, TAU, cross, dot, fillet_lines, gap, sub, unit};

/// Most of a turn one rational piece covers, so that its weights stay well behaved.
const QUARTER: f64 = TAU / 4.0;

fn at(centre: Point, a: Point, b: Point, across: f64, up: f64) -> Point {
    let axis = |i: usize| centre[i] + a[i] * across + b[i] * up;
    [axis(0), axis(1), axis(2)]
}

/// An exact rational curve for part of an ellipse with axis vectors `a` and `b`, turning `sweep`
/// from `a` towards `b` (*The NURBS Book*, section 7.5).
pub(crate) fn conic(centre: Point, a: Point, b: Point, sweep: f64) -> Result<Curve, KernelError> {
    let pieces = (sweep / QUARTER - 1e-9).ceil().max(1.0) as usize;
    let step = sweep / pieces as f64;
    let shoulder = (step / 2.0).cos();
    let mut points = vec![at(centre, a, b, 1.0, 0.0)];
    let mut weights = vec![1.0];
    let mut knots = vec![0.0; 3];
    for piece in 0..pieces {
        let middle = step * (piece as f64 + 0.5);
        let end = step * (piece as f64 + 1.0);
        let reach = 1.0 / shoulder;
        points.push(at(centre, a, b, middle.cos() * reach, middle.sin() * reach));
        weights.push(shoulder);
        points.push(at(centre, a, b, end.cos(), end.sin()));
        weights.push(1.0);
        let knot = (piece + 1) as f64;
        let copies = if piece + 1 == pieces { 3 } else { 2 };
        knots.extend(std::iter::repeat_n(knot, copies));
    }
    Curve::nurbs(2, knots, points, weights)
}

impl Curve {
    /// An exact ellipse from its centre and two axis vectors at a right angle.
    pub fn ellipse(centre: Point, a: Point, b: Point) -> Result<Self, KernelError> {
        if unit(a).is_none()
            || unit(b).is_none()
            || dot(a, b).abs() > 1e-9 * gap(a, [0.0; 3]) * gap(b, [0.0; 3])
        {
            return Err(KernelError(
                "an ellipse needs two axes at a right angle".to_owned(),
            ));
        }
        conic(centre, a, b, TAU)
    }

    /// An exact part of an ellipse from its centre and two axis vectors at a right angle, starting
    /// at parameter `start` (the angle in the ellipse's own frame, as DXF counts it) and turning
    /// `sweep` from `major` towards `minor`.
    pub fn elliptical_arc(
        centre: Point,
        major: Point,
        minor: Point,
        start: f64,
        sweep: f64,
    ) -> Result<Self, KernelError> {
        if !(sweep > 1e-12 && sweep <= TAU + 1e-12) {
            return Err(KernelError(
                "an elliptical arc needs a turn of more than nothing and at most a whole turn"
                    .to_owned(),
            ));
        }
        Self::ellipse(centre, major, minor)?;
        let (s, c) = start.sin_cos();
        let a: Point = std::array::from_fn(|i| major[i] * c + minor[i] * s);
        let b: Point = std::array::from_fn(|i| minor[i] * c - major[i] * s);
        conic(centre, a, b, sweep.min(TAU))
    }

    /// The ellipse or elliptical arc this curve exactly is, if it is one: a circle, an arc, or a
    /// rational quadratic NURBS curve made of conic pieces (as Curva and Rhino write ellipses).
    pub fn as_ellipse(&self) -> Option<Elliptical> {
        match self {
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => Some(Elliptical {
                centre: *centre,
                major: scaled_by(*x, *radius),
                minor: scaled_by(*y, *radius),
                start: 0.0,
                sweep: TAU,
            }),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => Some(Elliptical {
                centre: *centre,
                major: scaled_by(*x, *radius),
                minor: scaled_by(*y, *radius),
                start: 0.0,
                sweep: *sweep,
            }),
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => conic_parts(*degree, knots, points, weights),
            _ => None,
        }
    }

    /// The same curve stretched by `factor` along the unit vector `axis`, about `origin`.
    pub fn stretched(&self, origin: Point, axis: Point, factor: f64) -> Result<Self, KernelError> {
        let map = |p: Point| {
            let along = dot(sub(p, origin), axis) * (factor - 1.0);
            [
                p[0] + axis[0] * along,
                p[1] + axis[1] * along,
                p[2] + axis[2] * along,
            ]
        };
        let many = |list: &[Point]| list.iter().map(|p| map(*p)).collect::<Vec<Point>>();
        let scaled = |vector: Point, radius: f64| {
            [vector[0] * radius, vector[1] * radius, vector[2] * radius]
        };
        match self {
            Self::Point { at } => Ok(Self::Point { at: map(*at) }),
            Self::Line { start, end } => Self::line(map(*start), map(*end)),
            Self::Polyline { points, closed } => Self::polyline(many(points), *closed),
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => conic(*centre, scaled(*x, *radius), scaled(*y, *radius), TAU)?
                .stretched(origin, axis, factor),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => conic(*centre, scaled(*x, *radius), scaled(*y, *radius), *sweep)?
                .stretched(origin, axis, factor),
            Self::Interpolated { .. } => self.to_nurbs()?.stretched(origin, axis, factor),
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => Self::nurbs(*degree, knots.clone(), many(points), weights.clone()),
            Self::Compound { segments } => Self::compound(
                segments
                    .iter()
                    .map(|segment| segment.stretched(origin, axis, factor))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        }
    }
}

/// An ellipse or elliptical arc by its axes, the way DXF describes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elliptical {
    /// The centre.
    pub centre: Point,
    /// The major axis, from the centre to the curve: never shorter than `minor` for a curve
    /// that was recognised (a circle or arc gives its own axes).
    pub major: Point,
    /// The minor axis, at a right angle to `major`.
    pub minor: Point,
    /// Where the curve starts, as an angle in the ellipse's own frame from `major` towards
    /// `minor`.
    pub start: f64,
    /// How far it turns from `start`, a whole turn for a closed ellipse.
    pub sweep: f64,
}

/// The ellipse a rational quadratic curve of conic pieces lies on, worked out from its first piece
/// and checked against the whole curve.
fn conic_parts(
    degree: usize,
    knots: &[f64],
    points: &[Point],
    weights: &[f64],
) -> Option<Elliptical> {
    let count = points.len();
    if degree != 2 || weights.len() != count || count < 3 || count.is_multiple_of(2) {
        return None;
    }
    // Each piece is a rational quadratic Bézier: end knots three times, inner ones twice.
    let pieces = (count - 1) / 2;
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-12 * (1.0 + a.abs().max(b.abs()));
    let mut expected = Vec::with_capacity(count + 3);
    for piece in 0..=pieces {
        let copies = if piece == 0 || piece == pieces { 3 } else { 2 };
        let knot = knots.get(expected.len()).copied()?;
        for _ in 0..copies {
            expected.push(knot);
        }
    }
    if expected.len() != knots.len() || !expected.iter().zip(knots).all(|(a, b)| same(*a, *b)) {
        return None;
    }
    // A conic piece's shoulder weight, with its ends' weights brought to one, is the cosine of
    // half the angle it turns.
    let mut sweep = 0.0;
    for piece in 0..pieces {
        let (w0, w1, w2) = (
            weights[2 * piece],
            weights[2 * piece + 1],
            weights[2 * piece + 2],
        );
        if !(w0 > 0.0 && w2 > 0.0) {
            return None;
        }
        let shoulder = w1 / (w0 * w2).sqrt();
        if !(shoulder > 1e-9 && shoulder < 1.0 - 1e-12) {
            return None;
        }
        sweep += 2.0 * shoulder.acos();
    }
    if sweep > TAU + 1e-6 {
        return None;
    }
    // From the first piece: its ends E0, E1 and shoulder Q with weight w give the centre
    // c = ((E0 + E1) / 2 - w² Q) / (1 - w²), then conjugate axes along E0 and the way it turns.
    let (e0, q, e1) = (points[0], points[1], points[2]);
    let w = weights[1] / (weights[0] * weights[2]).sqrt();
    let step = 2.0 * w.acos();
    let w2 = w * w;
    let centre: Point = std::array::from_fn(|i| ((e0[i] + e1[i]) / 2.0 - w2 * q[i]) / (1.0 - w2));
    let a = sub(e0, centre);
    let to_end = sub(e1, centre);
    let b: Point = std::array::from_fn(|i| (to_end[i] - a[i] * step.cos()) / step.sin());
    // The principal axes of the conjugate pair: the curve c + a cos t + b sin t is longest
    // across at t0 with tan 2t0 = 2 a·b / (a·a − b·b).
    let t0 = 0.5 * (2.0 * dot(a, b)).atan2(dot(a, a) - dot(b, b));
    let along = |t: f64| -> Point { std::array::from_fn(|i| a[i] * t.cos() + b[i] * t.sin()) };
    let (mut major, mut minor, mut start) = (along(t0), along(t0 + QUARTER), -t0);
    if dot(minor, minor) > dot(major, major) {
        (major, minor) = (minor, scaled_by(major, -1.0));
        start -= QUARTER;
    }
    let size = gap(major, [0.0; 3]);
    let across = gap(minor, [0.0; 3]);
    if across.is_nan() || across <= 1e-9 * size.max(1.0) {
        return None;
    }
    // Every point of the curve must lie on that ellipse, in its plane.
    let normal = unit(cross(major, minor))?;
    let (mu, nu) = (unit(major)?, unit(minor)?);
    let curve = Curve::nurbs(2, knots.to_vec(), points.to_vec(), weights.to_vec()).ok()?;
    let tolerance = 1e-7 * size.max(1.0);
    for point in curve.sample().ok()? {
        let d = sub(point, centre);
        let (x, y) = (dot(d, mu) / size, dot(d, nu) / across);
        let off = ((x * x + y * y).sqrt() - 1.0).abs() * size;
        if dot(d, normal).abs() > tolerance || off > tolerance {
            return None;
        }
    }
    let sweep = if gap(points[0], points[count - 1]) <= tolerance {
        TAU
    } else {
        sweep
    };
    Some(Elliptical {
        centre,
        major,
        minor,
        start: start.rem_euclid(TAU),
        sweep,
    })
}

fn scaled_by(v: Point, factor: f64) -> Point {
    [v[0] * factor, v[1] * factor, v[2] * factor]
}

/// Two lines with the corner between them cut off by a straight line.
#[derive(Clone, Debug, PartialEq)]
pub struct Chamfered {
    /// The first line, cut back.
    pub first: Curve,
    /// The second line, cut back.
    pub second: Curve,
    /// The line across the corner.
    pub across: Curve,
}

/// Cuts the corner between two lines, `first_distance` and `second_distance` back from where
/// they meet, keeping the side of each that was picked.
pub fn chamfer_lines(
    first: &Curve,
    first_pick: Point,
    second: &Curve,
    second_pick: Point,
    first_distance: f64,
    second_distance: f64,
) -> Result<Chamfered, KernelError> {
    if !(first_distance > 0.0 && second_distance > 0.0) {
        return Err(KernelError(
            "the distances must be more than zero".to_owned(),
        ));
    }
    // A sharp corner gives the meeting point and the part of each line that is kept.
    let corner = fillet_lines(first, first_pick, second, second_pick, 0.0)?;
    let ends = |curve: &Curve| -> Result<(Point, Point), KernelError> {
        let path = curve.sample()?;
        Ok((path[0], path[path.len() - 1]))
    };
    let (far_a, meet) = ends(&corner.first)?;
    let (_, far_b) = ends(&corner.second)?;
    let back = |far: Point, distance: f64| -> Result<Point, KernelError> {
        if distance >= gap(far, meet) {
            return Err(KernelError(
                "that distance is too large for those lines".to_owned(),
            ));
        }
        let direction = unit(sub(far, meet))
            .ok_or_else(|| KernelError("that line has no length".to_owned()))?;
        Ok([
            meet[0] + direction[0] * distance,
            meet[1] + direction[1] * distance,
            meet[2] + direction[2] * distance,
        ])
    };
    let (cut_a, cut_b) = (back(far_a, first_distance)?, back(far_b, second_distance)?);
    if unit(cross(sub(far_a, meet), sub(far_b, meet))).is_none() {
        return Err(KernelError("those lines do not make a corner".to_owned()));
    }
    Ok(Chamfered {
        first: Curve::line(far_a, cut_a)?,
        second: Curve::line(cut_b, far_b)?,
        across: Curve::line(cut_a, cut_b)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: Point = [1.0, 0.0, 0.0];
    const Y: Point = [0.0, 1.0, 0.0];

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn close(a: Point, b: Point) -> bool {
        gap(a, b) < 1e-6
    }

    fn on_ellipse(p: Point, a: f64, b: f64) -> bool {
        near((p[0] / a).powi(2) + (p[1] / b).powi(2), 1.0)
    }

    #[test]
    fn an_ellipse_is_exactly_an_ellipse() {
        let curve = Curve::ellipse([0.0; 3], [5.0, 0.0, 0.0], [0.0, 3.0, 0.0]).unwrap();
        for point in curve.sample().unwrap() {
            assert!(on_ellipse(point, 5.0, 3.0), "{point:?}");
        }
    }

    #[test]
    fn an_ellipse_with_equal_axes_has_the_length_of_a_circle() {
        let curve = Curve::ellipse([0.0; 3], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]).unwrap();
        assert!((curve.length().unwrap() - TAU * 4.0).abs() < 1e-5);
    }

    #[test]
    fn an_ellipse_is_recognised_by_its_axes() {
        let curve = Curve::ellipse([1.0, 2.0, 3.0], [0.0, 3.0, 0.0], [-5.0, 0.0, 0.0]).unwrap();
        let found = curve.as_ellipse().unwrap();
        assert!(close(found.centre, [1.0, 2.0, 3.0]), "{found:?}");
        assert!(near(gap(found.major, [0.0; 3]), 5.0), "{found:?}");
        assert!(near(gap(found.minor, [0.0; 3]), 3.0), "{found:?}");
        assert!(near(found.sweep, TAU));
        assert!(near(dot(found.major, found.minor), 0.0));
    }

    #[test]
    fn an_elliptical_arc_comes_back_with_its_start_and_turn() {
        let (major, minor) = ([4.0, 0.0, 0.0], [0.0, 0.0, 2.0]);
        let arc = Curve::elliptical_arc([0.0; 3], major, minor, 0.5, 2.5).unwrap();
        let path = arc.sample().unwrap();
        let at =
            |t: f64| -> Point { std::array::from_fn(|i| major[i] * t.cos() + minor[i] * t.sin()) };
        assert!(close(path[0], at(0.5)));
        assert!(close(path[path.len() - 1], at(3.0)));
        let found = arc.as_ellipse().unwrap();
        let again = Curve::elliptical_arc(
            found.centre,
            found.major,
            found.minor,
            found.start,
            found.sweep,
        )
        .unwrap()
        .sample()
        .unwrap();
        assert!(close(again[0], path[0]), "{found:?}");
        assert!(
            close(again[again.len() - 1], path[path.len() - 1]),
            "{found:?}"
        );
    }

    #[test]
    fn a_curve_that_is_not_an_ellipse_is_not_recognised() {
        let wavy = Curve::nurbs(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![[0.0; 3], [1.0, 2.0, 0.0], [3.0, 0.0, 0.0]],
            vec![],
        )
        .unwrap();
        assert!(wavy.as_ellipse().is_none());
        let line = Curve::line([0.0; 3], X).unwrap();
        assert!(line.as_ellipse().is_none());
    }

    #[test]
    fn an_ellipse_needs_axes_at_a_right_angle() {
        assert!(Curve::ellipse([0.0; 3], X, [1.0, 1.0, 0.0]).is_err());
        assert!(Curve::ellipse([0.0; 3], X, [0.0; 3]).is_err());
    }

    #[test]
    fn a_stretched_circle_becomes_an_exact_ellipse() {
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let stretched = circle.stretched([0.0; 3], X, 3.0).unwrap();
        assert!(matches!(stretched, Curve::Nurbs { .. }));
        for point in stretched.sample().unwrap() {
            assert!(on_ellipse(point, 6.0, 2.0), "{point:?}");
        }
    }

    #[test]
    fn a_stretched_arc_keeps_its_ends_stretched() {
        let arc = Curve::arc([0.0; 3], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], X, Y).unwrap();
        let points = arc.stretched([0.0; 3], Y, 5.0).unwrap().sample().unwrap();
        assert!(close(points[0], [2.0, 0.0, 0.0]));
        assert!(close(*points.last().unwrap(), [0.0, 10.0, 0.0]));
    }

    #[test]
    fn stretching_is_about_the_origin_and_only_along_the_axis() {
        let line = Curve::line([1.0, 1.0, 0.0], [3.0, 1.0, 0.0]).unwrap();
        let points = line
            .stretched([1.0, 0.0, 0.0], X, 2.0)
            .unwrap()
            .sample()
            .unwrap();
        assert!(close(points[0], [1.0, 1.0, 0.0]));
        assert!(close(points[1], [5.0, 1.0, 0.0]));
    }

    #[test]
    fn a_stretched_smooth_curve_stays_on_the_stretched_shape() {
        let original = Curve::interpolated(
            vec![
                [0.0; 3],
                [10.0, 5.0, 0.0],
                [20.0, -3.0, 0.0],
                [30.0, 0.0, 0.0],
            ],
            false,
        )
        .unwrap();
        let stretched = original.stretched([0.0; 3], Y, 2.0).unwrap();
        for point in stretched.sample().unwrap() {
            let back = [point[0], point[1] / 2.0, point[2]];
            assert!(
                gap(original.closest_point(back).unwrap(), back) < 1e-5,
                "{point:?}"
            );
        }
    }

    #[test]
    fn a_joined_curve_stretches_part_by_part() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line(X, [1.0, 1.0, 0.0]).unwrap();
        let joined = Curve::compound(vec![a, b]).unwrap();
        let stretched = joined.stretched([0.0; 3], Y, 4.0).unwrap();
        assert!(near(stretched.length().unwrap(), 5.0));
    }

    #[test]
    fn a_right_angle_chamfers_by_the_distances() {
        let a = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap();
        let done = chamfer_lines(&a, X, &b, [10.0, 9.0, 0.0], 3.0, 4.0).unwrap();
        assert!(near(done.first.length().unwrap(), 7.0));
        assert!(near(done.second.length().unwrap(), 6.0));
        assert!(near(done.across.length().unwrap(), 5.0));
    }

    #[test]
    fn a_distance_too_large_is_refused() {
        let a = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap();
        assert!(chamfer_lines(&a, X, &b, [10.0, 9.0, 0.0], 30.0, 1.0).is_err());
        assert!(chamfer_lines(&a, X, &b, [10.0, 9.0, 0.0], 0.0, 1.0).is_err());
    }
}
