//! Offsetting curves on a plane, and rounding the corner between two lines.

use crate::{Curve, KernelError, Point, cross, dot, gap, sub, unit};

/// How far apart two lines may be, in millimetres, to count as meeting.
const LINES_MEET: f64 = 1e-6;
/// How many fitted points a smooth offset gets for each sampled point skipped.
const SMOOTH_FIT_STEP: usize = 4;

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [
        p[0] + direction[0] * amount,
        p[1] + direction[1] * amount,
        p[2] + direction[2] * amount,
    ]
}

fn too_far() -> KernelError {
    KernelError("that distance is too large for this curve".to_owned())
}

/// Which side of a path `toward` lies: `1.0` for the left looking along it, seen from `normal`.
fn side_of(path: &[Point], normal: Point, toward: Point) -> f64 {
    let mut best = (f64::INFINITY, 1.0);
    for pair in path.windows(2) {
        let along = sub(pair[1], pair[0]);
        let t = (dot(sub(toward, pair[0]), along) / dot(along, along).max(1e-24)).clamp(0.0, 1.0);
        let on = plus(pair[0], along, t);
        let distance = gap(toward, on);
        if distance < best.0 {
            let left = cross(normal, along);
            let sign = if dot(sub(toward, on), left) < 0.0 {
                -1.0
            } else {
                1.0
            };
            best = (distance, sign);
        }
    }
    best.1
}

/// Offsets corners by mitring them, so sides stay straight and meet.
fn offset_corners(
    points: &[Point],
    closed: bool,
    normal: Point,
    amount: f64,
) -> Result<Vec<Point>, KernelError> {
    let count = points.len();
    let sides = if closed { count } else { count - 1 };
    let lefts = (0..sides)
        .map(|index| {
            let direction = unit(sub(points[(index + 1) % count], points[index]));
            direction.map(|d| cross(normal, d)).ok_or_else(too_far)
        })
        .collect::<Result<Vec<Point>, _>>()?;
    let moved = (0..count)
        .map(|index| {
            let before = if index > 0 {
                Some(lefts[index - 1])
            } else if closed {
                Some(lefts[sides - 1])
            } else {
                None
            };
            let after = if index < sides {
                Some(lefts[index])
            } else {
                None
            };
            let (a, b) = match (before, after) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) => (a, a),
                (None, Some(b)) => (b, b),
                (None, None) => return Err(too_far()),
            };
            let turn = 1.0 + dot(a, b);
            if turn < 1e-9 {
                return Err(too_far());
            }
            let mitre = [
                (a[0] + b[0]) / turn,
                (a[1] + b[1]) / turn,
                (a[2] + b[2]) / turn,
            ];
            Ok(plus(points[index], mitre, amount))
        })
        .collect::<Result<Vec<Point>, _>>()?;
    // A side that has turned round means the corners have passed each other.
    for index in 0..sides {
        let next = (index + 1) % count;
        if dot(
            sub(moved[next], moved[index]),
            sub(points[next], points[index]),
        ) <= 0.0
        {
            return Err(too_far());
        }
    }
    Ok(moved)
}

/// Offsets a smooth curve by moving sampled points sideways and fitting a curve through them.
fn offset_smooth(
    path: &[Point],
    closed: bool,
    normal: Point,
    amount: f64,
) -> Result<Curve, KernelError> {
    let last = path.len() - 1;
    let tangent = |index: usize| {
        let (before, after) = if closed {
            (
                if index == 0 { last - 1 } else { index - 1 },
                if index == last { 1 } else { index + 1 },
            )
        } else {
            (index.saturating_sub(1), (index + 1).min(last))
        };
        unit(sub(path[after], path[before])).ok_or_else(too_far)
    };
    let mut fitted = Vec::new();
    let mut index = 0;
    while index <= last {
        fitted.push(plus(path[index], cross(normal, tangent(index)?), amount));
        index += SMOOTH_FIT_STEP;
    }
    if !last.is_multiple_of(SMOOTH_FIT_STEP) {
        fitted.push(plus(path[last], cross(normal, tangent(last)?), amount));
    }
    if closed {
        fitted.pop();
    }
    Curve::interpolated(fitted, closed)
}

impl Curve {
    /// A copy `distance` away on the plane facing along the unit vector `normal`, on the side
    /// of the curve that `toward` lies.
    pub fn offset(&self, normal: Point, distance: f64, toward: Point) -> Result<Self, KernelError> {
        if !distance.is_finite() || distance <= 0.0 {
            return Err(KernelError(
                "the distance must be more than zero".to_owned(),
            ));
        }
        let path = self.sample()?;
        let amount = side_of(&path, normal, toward) * distance;
        match self {
            Self::Point { .. } => Err(KernelError("a point cannot be offset".to_owned())),
            Self::Line { start, end } => {
                let moved = offset_corners(&[*start, *end], false, normal, amount)?;
                Self::line(moved[0], moved[1])
            }
            Self::Polyline { points, closed } => {
                Self::polyline(offset_corners(points, *closed, normal, amount)?, *closed)
            }
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => {
                let radius = changed_radius(*radius, *centre, distance, toward)?;
                Ok(Self::Circle {
                    centre: *centre,
                    radius,
                    x: *x,
                    y: *y,
                })
            }
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => {
                let radius = changed_radius(*radius, *centre, distance, toward)?;
                Ok(Self::Arc {
                    centre: *centre,
                    radius,
                    x: *x,
                    y: *y,
                    sweep: *sweep,
                })
            }
            Self::Interpolated { closed, .. } => offset_smooth(&path, *closed, normal, amount),
            Self::Nurbs { .. } => {
                let closed = gap(path[0], path[path.len() - 1]) < 1e-6;
                offset_smooth(&path, closed, normal, amount)
            }
            Self::Compound { .. } => Err(KernelError(
                "joined curves cannot be offset yet; explode them first".to_owned(),
            )),
        }
    }
}

/// How far beside a curve a point is put to stand for a side, in millimetres.
const SIDE_MARKER: f64 = 1e-4;

impl Curve {
    /// Which side of the curve `toward` is on, seen from `normal`: `1.0` for the left.
    pub fn side_of_point(&self, normal: Point, toward: Point) -> Result<f64, KernelError> {
        let path = self.sample()?;
        Ok(side_of(&path, normal, toward))
    }

    /// A copy at a distance on a side given by [`Self::side_of_point`], which stays the same side
    /// when the curve moves or changes.
    pub fn offset_on_side(
        &self,
        normal: Point,
        distance: f64,
        side: f64,
    ) -> Result<Self, KernelError> {
        let path = self.sample()?;
        let along = unit(sub(path[1], path[0])).ok_or_else(too_far)?;
        let left = cross(normal, along);
        let start = [
            (path[0][0] + path[1][0]) / 2.0,
            (path[0][1] + path[1][1]) / 2.0,
            (path[0][2] + path[1][2]) / 2.0,
        ];
        self.offset(normal, distance, plus(start, left, side * SIDE_MARKER))
    }
}

fn changed_radius(
    radius: f64,
    centre: Point,
    distance: f64,
    toward: Point,
) -> Result<f64, KernelError> {
    let outside = gap(toward, centre) > radius;
    let changed = if outside {
        radius + distance
    } else {
        radius - distance
    };
    if changed <= 0.0 {
        return Err(too_far());
    }
    Ok(changed)
}

/// Two lines with the corner between them rounded.
#[derive(Clone, Debug, PartialEq)]
pub struct Filleted {
    /// The first line, shortened or lengthened to the fillet.
    pub first: Curve,
    /// The second line, shortened or lengthened to the fillet.
    pub second: Curve,
    /// The arc between them; `None` when the radius is zero and the lines just meet.
    pub arc: Option<Curve>,
}

/// The ends of a curve if it is a straight line.
fn line_ends(curve: &Curve) -> Result<(Point, Point), KernelError> {
    match curve {
        Curve::Line { start, end } => Ok((*start, *end)),
        _ => Err(KernelError("only lines can be filleted so far".to_owned())),
    }
}

/// Where two lines, taken as endless, meet.
fn meeting_point(a: (Point, Point), b: (Point, Point)) -> Result<Point, KernelError> {
    let parallel = || KernelError("those lines do not meet".to_owned());
    let (d1, d2) = (sub(a.1, a.0), sub(b.1, b.0));
    let across = cross(d1, d2);
    let square = dot(across, across);
    if square < 1e-18 {
        return Err(parallel());
    }
    let offset = sub(b.0, a.0);
    let on_first = plus(a.0, d1, dot(cross(offset, d2), across) / square);
    let on_second = plus(b.0, d2, dot(cross(offset, d1), across) / square);
    if gap(on_first, on_second) > LINES_MEET {
        return Err(parallel());
    }
    Ok(on_first)
}

/// From the corner, the unit direction of the kept side of a line and how far it reaches.
fn kept_side(
    ends: (Point, Point),
    corner: Point,
    pick: Point,
) -> Result<(Point, f64), KernelError> {
    let along = unit(sub(ends.1, ends.0)).ok_or_else(too_far)?;
    let direction = if dot(sub(pick, corner), along) >= 0.0 {
        along
    } else {
        [-along[0], -along[1], -along[2]]
    };
    let reach = dot(sub(ends.0, corner), direction).max(dot(sub(ends.1, corner), direction));
    if reach <= 0.0 {
        return Err(too_far());
    }
    Ok((direction, reach))
}

/// Rounds the corner between two lines, keeping the side of each that was picked.
pub fn fillet_lines(
    first: &Curve,
    first_pick: Point,
    second: &Curve,
    second_pick: Point,
    radius: f64,
) -> Result<Filleted, KernelError> {
    if !radius.is_finite() || radius < 0.0 {
        return Err(KernelError("the radius cannot be negative".to_owned()));
    }
    let (a, b) = (line_ends(first)?, line_ends(second)?);
    let corner = meeting_point(a, b)?;
    let (toward_a, reach_a) = kept_side(a, corner, first_pick)?;
    let (toward_b, reach_b) = kept_side(b, corner, second_pick)?;
    let far_a = plus(corner, toward_a, reach_a);
    let far_b = plus(corner, toward_b, reach_b);
    if radius == 0.0 {
        return Ok(Filleted {
            first: Curve::line(far_a, corner)?,
            second: Curve::line(corner, far_b)?,
            arc: None,
        });
    }
    let half = dot(toward_a, toward_b).clamp(-1.0, 1.0).acos() / 2.0;
    let setback = radius / half.tan();
    if setback >= reach_a || setback >= reach_b {
        return Err(KernelError(
            "that radius is too large for those lines".to_owned(),
        ));
    }
    let bisector = unit([
        toward_a[0] + toward_b[0],
        toward_a[1] + toward_b[1],
        toward_a[2] + toward_b[2],
    ])
    .ok_or_else(too_far)?;
    let centre = plus(corner, bisector, radius / half.sin());
    let (touch_a, touch_b) = (
        plus(corner, toward_a, setback),
        plus(corner, toward_b, setback),
    );
    // The plane is turned so that the short way round the fillet runs counterclockwise.
    let normal = unit(cross(sub(touch_a, centre), sub(touch_b, centre))).ok_or_else(too_far)?;
    let x = unit(sub(touch_a, centre)).ok_or_else(too_far)?;
    let y = cross(normal, x);
    Ok(Filleted {
        first: Curve::line(far_a, touch_a)?,
        second: Curve::line(touch_b, far_b)?,
        arc: Some(Curve::arc(centre, touch_a, touch_b, x, y)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: Point = [1.0, 0.0, 0.0];
    const Y: Point = [0.0, 1.0, 0.0];
    const Z: Point = [0.0, 0.0, 1.0];

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn close(a: Point, b: Point) -> bool {
        gap(a, b) < 1e-6
    }

    fn line(a: Point, b: Point) -> Curve {
        Curve::line(a, b).unwrap()
    }

    #[test]
    fn a_line_offsets_to_the_side_of_the_cursor() {
        let original = line([0.0; 3], [10.0, 0.0, 0.0]);
        let left = original.offset(Z, 2.0, [5.0, 9.0, 0.0]).unwrap();
        let right = original.offset(Z, 2.0, [5.0, -9.0, 0.0]).unwrap();
        assert_eq!(left, line([0.0, 2.0, 0.0], [10.0, 2.0, 0.0]));
        assert_eq!(right, line([0.0, -2.0, 0.0], [10.0, -2.0, 0.0]));
    }

    #[test]
    fn a_polyline_offsets_with_sharp_corners() {
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let moved = Curve::polyline(path, false)
            .unwrap()
            .offset(Z, 1.0, [5.0, 5.0, 0.0])
            .unwrap();
        let points = moved.sample().unwrap();
        assert!(close(points[0], [0.0, 1.0, 0.0]));
        assert!(close(points[1], [9.0, 1.0, 0.0]));
        assert!(close(points[2], [9.0, 10.0, 0.0]));
    }

    fn square() -> Curve {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        Curve::polyline(path, true).unwrap()
    }

    #[test]
    fn a_closed_polyline_offsets_inward_and_outward() {
        let inward = square().offset(Z, 1.0, [5.0, 5.0, 0.0]).unwrap();
        assert!(near(inward.length().unwrap(), 32.0));
        let outward = square().offset(Z, 1.0, [-5.0, 5.0, 0.0]).unwrap();
        assert!(near(outward.length().unwrap(), 48.0));
    }

    #[test]
    fn an_offset_too_far_inside_is_refused() {
        assert!(square().offset(Z, 6.0, [5.0, 5.0, 0.0]).is_err());
    }

    #[test]
    fn a_circle_grows_or_shrinks_by_the_distance() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let grown = circle.offset(Z, 2.0, [9.0, 0.0, 0.0]).unwrap();
        assert_eq!(grown, Curve::circle([0.0; 3], 7.0).unwrap());
        let shrunk = circle.offset(Z, 2.0, [1.0, 0.0, 0.0]).unwrap();
        assert_eq!(shrunk, Curve::circle([0.0; 3], 3.0).unwrap());
    }

    #[test]
    fn a_circle_cannot_shrink_away() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        assert!(circle.offset(Z, 6.0, [1.0, 0.0, 0.0]).is_err());
    }

    #[test]
    fn an_arc_keeps_its_centre_and_turn() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let moved = arc.offset(Z, 1.0, [9.0, 9.0, 0.0]).unwrap();
        assert!(near(
            moved.length().unwrap(),
            6.0 * std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn a_smooth_offset_stays_the_distance_from_the_original() {
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
        let moved = original.offset(Z, 2.0, [10.0, 30.0, 0.0]).unwrap();
        let points = moved.sample().unwrap();
        for point in &points[8..points.len() - 8] {
            let on = original.closest_point(*point).unwrap();
            assert!((gap(on, *point) - 2.0).abs() < 1e-2, "{point:?}");
        }
    }

    #[test]
    fn an_offset_follows_the_plane_it_is_given() {
        let upright = line([0.0; 3], [10.0, 0.0, 0.0]);
        let moved = upright
            .offset([0.0, -1.0, 0.0], 3.0, [5.0, 0.0, 9.0])
            .unwrap();
        assert_eq!(moved, line([0.0, 0.0, 3.0], [10.0, 0.0, 3.0]));
    }

    #[test]
    fn a_distance_of_zero_is_refused() {
        assert!(line([0.0; 3], X).offset(Z, 0.0, Y).is_err());
    }

    #[test]
    fn a_right_angle_rounds_with_the_exact_arc() {
        let a = line([0.0; 3], [10.0, 0.0, 0.0]);
        let b = line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]);
        let done = fillet_lines(&a, [1.0, 0.0, 0.0], &b, [10.0, 9.0, 0.0], 2.0).unwrap();
        assert!(near(done.first.length().unwrap(), 8.0));
        assert!(near(done.second.length().unwrap(), 8.0));
        let arc = done.arc.unwrap();
        assert!(near(
            arc.length().unwrap(),
            2.0 * std::f64::consts::FRAC_PI_2
        ));
        let points = arc.sample().unwrap();
        assert!(close(points[0], [8.0, 0.0, 0.0]));
        assert!(close(*points.last().unwrap(), [10.0, 2.0, 0.0]));
    }

    #[test]
    fn lines_that_cross_keep_the_sides_picked() {
        let a = line([-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        let b = line([0.0, -10.0, 0.0], [0.0, 10.0, 0.0]);
        let done = fillet_lines(&a, [5.0, 0.0, 0.0], &b, [0.0, -5.0, 0.0], 1.0).unwrap();
        assert!(near(done.first.length().unwrap(), 9.0));
        assert!(done.first.sample().unwrap().iter().all(|p| p[0] > 0.0));
        assert!(done.second.sample().unwrap().iter().all(|p| p[1] < 0.0));
    }

    #[test]
    fn lines_short_of_each_other_are_extended_to_the_fillet() {
        let a = line([0.0; 3], [5.0, 0.0, 0.0]);
        let b = line([10.0, 5.0, 0.0], [10.0, 10.0, 0.0]);
        let done = fillet_lines(&a, [1.0, 0.0, 0.0], &b, [10.0, 9.0, 0.0], 2.0).unwrap();
        assert!(near(done.first.length().unwrap(), 8.0));
        assert!(near(done.second.length().unwrap(), 8.0));
    }

    #[test]
    fn a_radius_of_zero_makes_a_sharp_corner() {
        let a = line([0.0; 3], [5.0, 0.0, 0.0]);
        let b = line([10.0, 5.0, 0.0], [10.0, 10.0, 0.0]);
        let done = fillet_lines(&a, [1.0, 0.0, 0.0], &b, [10.0, 9.0, 0.0], 0.0).unwrap();
        assert!(done.arc.is_none());
        assert!(close(
            *done.first.sample().unwrap().last().unwrap(),
            [10.0, 0.0, 0.0]
        ));
        assert!(close(done.second.sample().unwrap()[0], [10.0, 0.0, 0.0]));
    }

    #[test]
    fn the_fillet_touches_both_lines_smoothly() {
        let a = line([0.0; 3], [20.0, 0.0, 0.0]);
        let b = line([20.0, 0.0, 0.0], [30.0, 15.0, 0.0]);
        let arc = fillet_lines(&a, [1.0, 0.0, 0.0], &b, [29.0, 13.5, 0.0], 3.0)
            .unwrap()
            .arc
            .unwrap();
        let points = arc.sample().unwrap();
        let start = unit(sub(points[1], points[0])).unwrap();
        assert!(dot(start, X) > 0.999);
    }

    #[test]
    fn a_radius_too_large_is_refused() {
        let a = line([0.0; 3], [10.0, 0.0, 0.0]);
        let b = line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]);
        assert!(fillet_lines(&a, [1.0, 0.0, 0.0], &b, [10.0, 9.0, 0.0], 20.0).is_err());
    }

    #[test]
    fn parallel_lines_are_refused() {
        let a = line([0.0; 3], X);
        let b = line(Y, [1.0, 1.0, 0.0]);
        assert!(fillet_lines(&a, X, &b, Y, 1.0).is_err());
    }

    #[test]
    fn only_lines_can_be_filleted_so_far() {
        let a = line([0.0; 3], X);
        let circle = Curve::circle([0.0; 3], 1.0).unwrap();
        assert!(fillet_lines(&a, X, &circle, Y, 0.1).is_err());
    }
}
