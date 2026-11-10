//! Finding where curves cross, and cutting them there.

use crate::{
    Curve, JOIN_TOLERANCE, KernelError, Point, TAU, dot, engine_error, gap, join,
    nurbs_engine_curve, spline, sub,
};
use solidcore::curve::curve::Curve as CoreCurve;
use solidcore::query::api_queries::curve_closest_point;
use solidcore::{Kernel, Point3};

/// How far apart two curves may be, in millimetres, for a sampled crossing to be considered.
const SAMPLED_CROSSING_TOLERANCE: f64 = 1e-2;
/// How far apart two curves may be after refining, in millimetres, to really cross.
const CROSSING_TOLERANCE: f64 = 1e-6;
/// How close two crossings may be, in millimetres, before they count as one.
const SAME_POINT: f64 = 1e-5;
/// Most rounds of refining a crossing.
const REFINING_ROUNDS: usize = 60;
/// How far along a piece, as a fraction, a cut must be from its ends to count.
const END_MARGIN: f64 = 1e-9;

fn along(a: Point, b: Point, t: f64) -> Point {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The closest points of two 3D segments, as a fraction along each
/// (Ericson, *Real-Time Collision Detection*, section 5.1.9).
#[must_use]
pub fn closest_between(a: (Point, Point), b: (Point, Point)) -> (f64, f64) {
    let (d1, d2, r) = (sub(a.1, a.0), sub(b.1, b.0), sub(a.0, b.0));
    let (e1, e2, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    let c = dot(d1, r);
    let shared = dot(d1, d2);
    let denominator = e1 * e2 - shared * shared;
    let mut s = if denominator > 1e-12 {
        ((shared * f - c * e2) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = if e2 > 1e-12 {
        (shared * s + f) / e2
    } else {
        0.0
    };
    if t < 0.0 {
        t = 0.0;
        s = if e1 > 1e-12 {
            (-c / e1).clamp(0.0, 1.0)
        } else {
            0.0
        };
    } else if t > 1.0 {
        t = 1.0;
        s = if e1 > 1e-12 {
            ((shared - c) / e1).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
    (s, t)
}

/// How far along a segment, from 0 to 1, a point is closest.
fn fraction_on(p: Point, a: Point, b: Point) -> f64 {
    let direction = sub(b, a);
    (dot(sub(p, a), direction) / dot(direction, direction).max(1e-24)).clamp(0.0, 1.0)
}

/// The point on an engine curve closest to `p`, and its parameter.
fn engine_closest(curve: &CoreCurve, p: Point) -> Result<(Point, f64), KernelError> {
    let kernel = Kernel::default();
    let found =
        curve_closest_point(&kernel, curve, Point3::new(p[0], p[1], p[2])).map_err(engine_error)?;
    let at = curve
        .evaluate_point(&kernel, found.parameter)
        .map_err(engine_error)?;
    Ok(([at.x, at.y, at.z], found.parameter))
}

/// The angle of `p` around a circle, from 0 to a full turn.
fn angle_of(p: Point, centre: Point, x: Point, y: Point) -> f64 {
    let offset = sub(p, centre);
    let angle = dot(offset, y).atan2(dot(offset, x));
    if angle < 0.0 { angle + TAU } else { angle }
}

fn on_circle(centre: Point, radius: f64, x: Point, y: Point, angle: f64) -> Point {
    let (cos, sin) = (angle.cos() * radius, angle.sin() * radius);
    let at = |axis: usize| centre[axis] + x[axis] * cos + y[axis] * sin;
    [at(0), at(1), at(2)]
}

fn sorted_stops(mut stops: Vec<f64>, low: f64, high: f64) -> Vec<f64> {
    let margin = (high - low) * END_MARGIN;
    stops.retain(|s| *s > low + margin && *s < high - margin);
    stops.sort_by(f64::total_cmp);
    stops.dedup_by(|a, b| (*a - *b).abs() <= margin);
    stops
}

fn arc_piece(centre: Point, radius: f64, x: Point, y: Point, from: f64, to: f64) -> Curve {
    let (sin, cos) = from.sin_cos();
    let mix = |a: f64, b: f64| {
        let at = |axis: usize| x[axis] * a + y[axis] * b;
        [at(0), at(1), at(2)]
    };
    Curve::Arc {
        centre,
        radius,
        x: mix(cos, sin),
        y: mix(-sin, cos),
        sweep: to - from,
    }
}

/// Cuts straight segments through `path` at the given points.
fn cut_path(path: &[Point], points: &[Point], closed: bool) -> Result<Vec<Curve>, KernelError> {
    let segments = path.len().saturating_sub(1);
    let position = |p: Point| {
        (0..segments)
            .map(|index| {
                let t = fraction_on(p, path[index], path[index + 1]);
                (
                    gap(p, along(path[index], path[index + 1], t)),
                    index as f64 + t,
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, at)| at)
    };
    let stops = sorted_stops(
        points.iter().filter_map(|p| position(*p)).collect(),
        0.0,
        segments as f64,
    );
    let point_at = |s: f64| {
        let index = (s.floor() as usize).min(segments - 1);
        along(path[index], path[index + 1], s - index as f64)
    };
    let bounds: Vec<f64> = std::iter::once(0.0)
        .chain(stops.iter().copied())
        .chain(std::iter::once(segments as f64))
        .collect();
    let mut runs: Vec<Vec<Point>> = bounds
        .windows(2)
        .map(|pair| {
            let mut run = vec![point_at(pair[0])];
            let inside =
                (pair[0].floor() as usize + 1)..=(pair[1].ceil() as usize).saturating_sub(1);
            run.extend(
                inside
                    .filter(|i| (*i as f64) > pair[0] && (*i as f64) < pair[1])
                    .map(|i| path[i]),
            );
            run.push(point_at(pair[1]));
            run
        })
        .collect();
    if closed && runs.len() >= 2 {
        // The seam at the first corner is not a cut, so the last run carries on into the first.
        let first = runs.remove(0);
        if let Some(last) = runs.last_mut() {
            last.extend(first.into_iter().skip(1));
        }
    }
    runs.into_iter()
        .map(|run| {
            if run.len() == 2 {
                Curve::line(run[0], run[1])
            } else {
                Curve::polyline(run, false)
            }
        })
        .collect()
}

type Homogeneous = [f64; 4];

/// One piece of NURBS data, in the weighted form knot insertion works in.
struct Weighted {
    degree: usize,
    knots: Vec<f64>,
    control: Vec<Homogeneous>,
}

impl Weighted {
    fn new(degree: usize, knots: &[f64], points: &[Point], weights: &[f64]) -> Self {
        let control = points
            .iter()
            .enumerate()
            .map(|(index, p)| {
                let w = weights.get(index).copied().unwrap_or(1.0);
                [p[0] * w, p[1] * w, p[2] * w, w]
            })
            .collect();
        Self {
            degree,
            knots: knots.to_vec(),
            control,
        }
    }

    /// Inserts one knot (*The NURBS Book*, section 5.2, algorithm A5.1).
    fn insert(&mut self, u: f64) {
        let p = self.degree;
        let k = self.knots.partition_point(|knot| *knot <= u) - 1;
        let mut control = Vec::with_capacity(self.control.len() + 1);
        for i in 0..=self.control.len() {
            control.push(if i + p <= k {
                self.control[i]
            } else if i > k {
                self.control[i - 1]
            } else {
                let share = (u - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
                let (here, before) = (self.control[i], self.control[i - 1]);
                std::array::from_fn(|axis| share * here[axis] + (1.0 - share) * before[axis])
            });
        }
        self.control = control;
        self.knots.insert(k + 1, u);
    }

    /// Cuts at `u`, which must be inside the curve, by bringing it up to full multiplicity.
    fn split(mut self, u: f64) -> (Self, Self) {
        let p = self.degree;
        let already = self.knots.iter().filter(|knot| **knot == u).count();
        for _ in already..p {
            self.insert(u);
        }
        let last = self.knots.partition_point(|knot| *knot <= u) - 1;
        let first = last + 1 - p;
        let shared = last - p;
        let left = Self {
            degree: p,
            knots: self.knots[..=last]
                .iter()
                .copied()
                .chain(std::iter::once(u))
                .collect(),
            control: self.control[..=shared].to_vec(),
        };
        let right = Self {
            degree: p,
            knots: std::iter::once(u)
                .chain(self.knots[first..].iter().copied())
                .collect(),
            control: self.control[shared..].to_vec(),
        };
        (left, right)
    }

    fn into_curve(self, rational: bool) -> Result<Curve, KernelError> {
        let points = self
            .control
            .iter()
            .map(|c| [c[0] / c[3], c[1] / c[3], c[2] / c[3]])
            .collect();
        let weights = if rational {
            self.control.iter().map(|c| c[3]).collect()
        } else {
            Vec::new()
        };
        Curve::nurbs(self.degree, self.knots, points, weights)
    }
}

impl Curve {
    /// The point on this curve closest to `p`.
    pub fn closest_point(&self, p: Point) -> Result<Point, KernelError> {
        Ok(match self {
            Self::Point { at } => *at,
            Self::Line { start, end } => along(*start, *end, fraction_on(p, *start, *end)),
            Self::Polyline { .. } => {
                let path = self.sample()?;
                let mut best = path[0];
                for pair in path.windows(2) {
                    let here = along(pair[0], pair[1], fraction_on(p, pair[0], pair[1]));
                    if gap(p, here) < gap(p, best) {
                        best = here;
                    }
                }
                best
            }
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => on_circle(*centre, *radius, *x, *y, angle_of(p, *centre, *x, *y)),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => {
                let angle = angle_of(p, *centre, *x, *y);
                let end = on_circle(*centre, *radius, *x, *y, *sweep);
                let start = on_circle(*centre, *radius, *x, *y, 0.0);
                if angle <= *sweep {
                    on_circle(*centre, *radius, *x, *y, angle)
                } else if gap(p, start) < gap(p, end) {
                    start
                } else {
                    end
                }
            }
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => engine_closest(&nurbs_engine_curve(*degree, knots, points, weights)?, p)?.0,
            Self::Interpolated { points, closed } => {
                engine_closest(&spline(points, *closed)?, p)?.0
            }
            Self::Compound { segments } => {
                let mut best: Option<Point> = None;
                for segment in segments {
                    let here = segment.closest_point(p)?;
                    if best.is_none_or(|held| gap(p, here) < gap(p, held)) {
                        best = Some(here);
                    }
                }
                best.ok_or_else(|| KernelError("an empty curve".to_owned()))?
            }
        })
    }

    /// The curve in pieces, cut at points on it, in order along the curve. A curve that is not
    /// really cut comes back whole, as a single piece.
    pub fn cut_at(&self, points: &[Point]) -> Result<Vec<Self>, KernelError> {
        match self {
            Self::Point { .. } => Ok(vec![self.clone()]),
            Self::Line { .. } | Self::Polyline { closed: false, .. } => {
                let path = self.sample()?;
                let pieces = cut_path(&path, points, false)?;
                Ok(if pieces.len() < 2 {
                    vec![self.clone()]
                } else {
                    pieces
                })
            }
            Self::Polyline { closed: true, .. } => {
                let path = self.sample()?;
                let stops = cut_path(&path, points, false)?;
                if stops.len() < 2 {
                    return Ok(vec![self.clone()]);
                }
                cut_path(&path, points, true)
            }
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => {
                let stops = sorted_stops(
                    points
                        .iter()
                        .map(|p| angle_of(*p, *centre, *x, *y))
                        .collect(),
                    -1.0,
                    TAU + 1.0,
                );
                if stops.len() < 2 {
                    return Ok(vec![self.clone()]);
                }
                let mut pieces: Vec<Self> = stops
                    .windows(2)
                    .map(|pair| arc_piece(*centre, *radius, *x, *y, pair[0], pair[1]))
                    .collect();
                let (first, last) = (stops[0], stops[stops.len() - 1]);
                pieces.push(arc_piece(*centre, *radius, *x, *y, last, first + TAU));
                Ok(pieces)
            }
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => {
                let inside = |p: &Point| angle_of(*p, *centre, *x, *y);
                let stops = sorted_stops(points.iter().map(inside).collect(), 0.0, *sweep);
                if stops.is_empty() {
                    return Ok(vec![self.clone()]);
                }
                let bounds: Vec<f64> = std::iter::once(0.0)
                    .chain(stops)
                    .chain(std::iter::once(*sweep))
                    .collect();
                Ok(bounds
                    .windows(2)
                    .map(|pair| arc_piece(*centre, *radius, *x, *y, pair[0], pair[1]))
                    .collect())
            }
            Self::Interpolated { .. } => {
                let pieces = self.to_nurbs()?.cut_at(points)?;
                Ok(if pieces.len() < 2 && !self.is_closed()? {
                    vec![self.clone()]
                } else {
                    pieces
                })
            }
            Self::Nurbs {
                degree,
                knots,
                points: control,
                weights,
            } => {
                let engine = nurbs_engine_curve(*degree, knots, control, weights)?;
                let mut stops = Vec::with_capacity(points.len());
                for p in points {
                    stops.push(engine_closest(&engine, *p)?.1);
                }
                let (low, high) = (knots[0], knots[knots.len() - 1]);
                let stops = sorted_stops(stops, low, high);
                if stops.is_empty() {
                    return Ok(vec![self.clone()]);
                }
                let mut rest = Weighted::new(*degree, knots, control, weights);
                let mut pieces = Vec::with_capacity(stops.len() + 1);
                for stop in stops {
                    let (left, right) = rest.split(stop);
                    pieces.push(left.into_curve(!weights.is_empty())?);
                    rest = right;
                }
                pieces.push(rest.into_curve(!weights.is_empty())?);
                if self.is_closed()? {
                    return carry_seam(pieces);
                }
                Ok(pieces)
            }
            Self::Compound { segments } => self.cut_compound(segments, points),
        }
    }

    pub(crate) fn is_closed(&self) -> Result<bool, KernelError> {
        let path = self.sample()?;
        Ok(match (path.first(), path.last()) {
            (Some(first), Some(last)) => path.len() > 2 && gap(*first, *last) <= JOIN_TOLERANCE,
            _ => false,
        })
    }

    fn cut_compound(&self, segments: &[Self], points: &[Point]) -> Result<Vec<Self>, KernelError> {
        let mut runs: Vec<Vec<Self>> = vec![Vec::new()];
        let total = segments.len();
        for (index, segment) in segments.iter().enumerate() {
            let mine: Vec<Point> = points
                .iter()
                .copied()
                .filter(|p| {
                    segment
                        .closest_point(*p)
                        .is_ok_and(|on| gap(on, *p) <= CROSSING_TOLERANCE * 10.0)
                })
                .collect();
            let pieces = segment.cut_at(&mine)?;
            for (at, piece) in pieces.into_iter().enumerate() {
                if at > 0 {
                    runs.push(Vec::new());
                }
                if let Some(run) = runs.last_mut() {
                    run.push(piece);
                }
            }
            let path = segment.sample()?;
            let at_joint = path
                .last()
                .is_some_and(|end| mine.iter().any(|p| gap(*p, *end) <= SAME_POINT));
            if at_joint && index + 1 < total {
                runs.push(Vec::new());
            }
        }
        if runs.len() < 2 {
            return Ok(vec![self.clone()]);
        }
        if self.is_closed()? {
            let first = runs.remove(0);
            if let Some(last) = runs.last_mut() {
                last.extend(first);
            }
        }
        runs.into_iter()
            .map(|mut run| {
                if run.len() == 1 {
                    run.pop()
                        .ok_or_else(|| KernelError("an empty piece".to_owned()))
                } else {
                    Self::compound(run)
                }
            })
            .collect()
    }
}

/// Makes the last and first pieces of a cut closed curve into one, as the seam is not a cut.
fn carry_seam(mut pieces: Vec<Curve>) -> Result<Vec<Curve>, KernelError> {
    if pieces.len() < 2 {
        return Ok(pieces);
    }
    let first = pieces.remove(0);
    let last = pieces.pop().unwrap_or_else(|| first.clone());
    let merged = join(&[last, first])?;
    pieces.extend(merged.into_iter().map(|(_, curve)| curve));
    Ok(pieces)
}

/// The points where two curves cross or touch.
pub fn crossings(a: &Curve, b: &Curve) -> Result<Vec<Point>, KernelError> {
    let (path_a, path_b) = (a.sample()?, b.sample()?);
    let mut found: Vec<Point> = Vec::new();
    for pair_a in path_a.windows(2) {
        for pair_b in path_b.windows(2) {
            let (s, t) = closest_between((pair_a[0], pair_a[1]), (pair_b[0], pair_b[1]));
            let start = along(pair_a[0], pair_a[1], s);
            if gap(start, along(pair_b[0], pair_b[1], t)) > SAMPLED_CROSSING_TOLERANCE {
                continue;
            }
            // Sampling cuts corners off curves, so walk between the two real curves.
            let mut on_a = a.closest_point(start)?;
            let mut on_b = b.closest_point(on_a)?;
            for _ in 0..REFINING_ROUNDS {
                if gap(on_a, on_b) <= CROSSING_TOLERANCE / 10.0 {
                    break;
                }
                on_a = a.closest_point(on_b)?;
                on_b = b.closest_point(on_a)?;
            }
            if gap(on_a, on_b) <= CROSSING_TOLERANCE
                && found.iter().all(|p| gap(*p, on_a) > SAME_POINT)
            {
                found.push(on_a);
            }
        }
    }
    Ok(found)
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

    fn line(a: Point, b: Point) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn total(pieces: &[Curve]) -> f64 {
        pieces.iter().map(|piece| piece.length().unwrap()).sum()
    }

    fn across() -> Curve {
        line([3.0, -20.0, 0.0], [3.0, 20.0, 0.0])
    }

    fn smooth() -> Curve {
        Curve::interpolated(
            vec![
                [0.0; 3],
                [10.0, 5.0, 0.0],
                [20.0, -3.0, 0.0],
                [30.0, 0.0, 0.0],
            ],
            false,
        )
        .unwrap()
    }

    #[test]
    fn two_lines_cross_exactly() {
        let flat = line([-10.0, 7.0, 0.0], [10.0, 7.0, 0.0]);
        assert_eq!(crossings(&flat, &across()).unwrap().len(), 1);
        assert!(close(
            crossings(&flat, &across()).unwrap()[0],
            [3.0, 7.0, 0.0]
        ));
    }

    #[test]
    fn lines_that_miss_do_not_cross() {
        let high = line([-10.0, 0.0, 5.0], [10.0, 0.0, 5.0]);
        assert!(crossings(&high, &across()).unwrap().is_empty());
    }

    #[test]
    fn a_line_crosses_a_circle_on_the_true_circle() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let found = crossings(&across(), &circle).unwrap();
        assert_eq!(found.len(), 2);
        for point in found {
            assert!(near(point[0].hypot(point[1]), 5.0), "{point:?}");
            assert!(near(point[0], 3.0), "{point:?}");
            assert!(near(point[1].abs(), 4.0), "{point:?}");
        }
    }

    #[test]
    fn a_line_crosses_a_smooth_curve_on_both() {
        let curve = smooth();
        let found = crossings(&across(), &curve).unwrap();
        assert_eq!(found.len(), 1);
        assert!(near(found[0][0], 3.0));
        assert!(gap(curve.closest_point(found[0]).unwrap(), found[0]) < 1e-6);
    }

    #[test]
    fn cutting_a_line_makes_two_lines_of_the_right_lengths() {
        let pieces = line([0.0; 3], [10.0, 0.0, 0.0])
            .cut_at(&[[3.0, 0.0, 0.0]])
            .unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(pieces[0].length().unwrap(), 3.0));
        assert!(near(pieces[1].length().unwrap(), 7.0));
    }

    #[test]
    fn a_cut_at_an_end_does_not_cut() {
        let original = line([0.0; 3], X);
        assert_eq!(original.cut_at(&[[0.0; 3]]).unwrap(), [original]);
    }

    #[test]
    fn a_polyline_cut_mid_segment_keeps_its_corners() {
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let pieces = Curve::polyline(path, false)
            .unwrap()
            .cut_at(&[[5.0, 0.0, 0.0]])
            .unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(matches!(pieces[1], Curve::Polyline { .. }));
        assert!(near(total(&pieces), 20.0));
    }

    #[test]
    fn a_polyline_can_be_cut_at_a_corner() {
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let pieces = Curve::polyline(path, false)
            .unwrap()
            .cut_at(&[[10.0, 0.0, 0.0]])
            .unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(pieces[0].length().unwrap(), 10.0));
    }

    #[test]
    fn a_closed_polyline_cut_twice_makes_two_open_pieces_around_its_seam() {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let pieces = Curve::polyline(path, true)
            .unwrap()
            .cut_at(&[[5.0, 0.0, 0.0], [5.0, 10.0, 0.0]])
            .unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(total(&pieces), 40.0));
    }

    #[test]
    fn a_circle_cut_twice_makes_two_arcs_that_add_up() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let cuts = [[3.0, 4.0, 0.0], [3.0, -4.0, 0.0]];
        let pieces = circle.cut_at(&cuts).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(total(&pieces), circle.length().unwrap()));
        assert!(pieces.iter().all(|p| matches!(p, Curve::Arc { .. })));
    }

    #[test]
    fn a_circle_cut_once_stays_whole() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        assert_eq!(circle.cut_at(&[[5.0, 0.0, 0.0]]).unwrap().len(), 1);
    }

    #[test]
    fn an_arc_is_cut_into_arcs_that_add_up() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let on = 5.0 * std::f64::consts::FRAC_PI_4.cos();
        let pieces = arc.cut_at(&[[on, on, 0.0]]).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(
            pieces[0].length().unwrap(),
            pieces[1].length().unwrap()
        ));
        assert!(near(total(&pieces), arc.length().unwrap()));
    }

    #[test]
    fn pieces_of_a_smooth_curve_are_exact_and_add_up() {
        let curve = smooth();
        let cut = crossings(&across(), &curve).unwrap();
        let pieces = curve.cut_at(&cut).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(pieces.iter().all(|p| matches!(p, Curve::Nurbs { .. })));
        assert!((total(&pieces) - curve.length().unwrap()).abs() < 1e-4);
        let first = pieces[0].sample().unwrap();
        let second = pieces[1].sample().unwrap();
        assert!(gap(*first.last().unwrap(), cut[0]) < 1e-6);
        assert!(gap(second[0], cut[0]) < 1e-6);
        for point in first.iter().chain(&second) {
            assert!(
                gap(curve.closest_point(*point).unwrap(), *point) < 1e-5,
                "{point:?}"
            );
        }
    }

    #[test]
    fn a_rational_curve_is_cut_exactly() {
        let quarter = Curve::nurbs(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![X, [1.0, 1.0, 0.0], Y],
            vec![1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0],
        )
        .unwrap();
        let on = std::f64::consts::FRAC_PI_6;
        let pieces = quarter.cut_at(&[[on.cos(), on.sin(), 0.0]]).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(pieces[0].length().unwrap(), on));
        for point in pieces[1].sample().unwrap() {
            assert!(near(point[0].hypot(point[1]), 1.0), "{point:?}");
        }
    }

    #[test]
    fn a_smooth_curve_can_be_cut_many_times() {
        let curve = smooth();
        let cuts: Vec<Point> = [5.0, 12.0, 25.0]
            .iter()
            .map(|x| curve.closest_point([*x, 0.0, 0.0]).unwrap())
            .collect();
        let pieces = curve.cut_at(&cuts).unwrap();
        assert_eq!(pieces.len(), 4);
        assert!((total(&pieces) - curve.length().unwrap()).abs() < 1e-4);
    }

    fn joined() -> Curve {
        let first = line([0.0; 3], [10.0, 0.0, 0.0]);
        let arc = Curve::arc([10.0, 5.0, 0.0], [10.0, 0.0, 0.0], [15.0, 5.0, 0.0], X, Y).unwrap();
        Curve::compound(vec![first, arc]).unwrap()
    }

    #[test]
    fn a_joined_curve_is_cut_inside_a_part() {
        let curve = joined();
        let pieces = curve.cut_at(&[[4.0, 0.0, 0.0]]).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(matches!(pieces[0], Curve::Line { .. }));
        assert!(matches!(pieces[1], Curve::Compound { .. }));
        assert!(near(total(&pieces), curve.length().unwrap()));
    }

    #[test]
    fn a_joined_curve_is_cut_at_its_joint() {
        let curve = joined();
        let pieces = curve.cut_at(&[[10.0, 0.0, 0.0]]).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(near(pieces[0].length().unwrap(), 10.0));
    }

    #[test]
    fn the_closest_point_on_an_arc_stops_at_its_ends() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        assert!(close(
            arc.closest_point([1.0, -9.0, 0.0]).unwrap(),
            [5.0, 0.0, 0.0]
        ));
    }
}
