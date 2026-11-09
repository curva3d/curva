//! Rounding the corner between lines, arcs and the segments of polylines.

use crate::{Curve, KernelError, Point, cross, dot, fillet_lines, gap, sub, unit};

/// The points of a polyline apart from a filleted end segment, and the vertex it joined at.
type PolylineRest = (Vec<Point>, Point);

/// How close a point must be, in millimetres, to count as on a curve end or vertex.
const SAME_POINT: f64 = 1e-6;

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [
        p[0] + direction[0] * amount,
        p[1] + direction[1] * amount,
        p[2] + direction[2] * amount,
    ]
}

/// The endless shape a fillet is worked out against.
#[derive(Clone, Copy)]
enum Carrier {
    Line { through: Point, along: Point },
    Circle { centre: Point, radius: f64 },
}

fn carrier_of(curve: &Curve) -> Result<Carrier, KernelError> {
    match curve {
        Curve::Line { start, end } => Ok(Carrier::Line {
            through: *start,
            along: unit(sub(*end, *start)).ok_or_else(|| bad("that line has no length"))?,
        }),
        Curve::Arc { centre, radius, .. } => Ok(Carrier::Circle {
            centre: *centre,
            radius: *radius,
        }),
        _ => Err(bad(
            "only lines, arcs and polyline segments can be filleted so far",
        )),
    }
}

/// The curves a fillet's centre can lie on: the shape moved out by the radius on each side.
fn offsets_of(carrier: Carrier, normal: Point, distance: f64) -> Vec<Carrier> {
    match carrier {
        Carrier::Line { through, along } => {
            let side = cross(normal, along);
            [1.0, -1.0]
                .into_iter()
                .map(|sign| Carrier::Line {
                    through: plus(through, side, sign * distance),
                    along,
                })
                .collect()
        }
        Carrier::Circle { centre, radius } => [radius + distance, (radius - distance).abs()]
            .into_iter()
            .filter(|r| *r > SAME_POINT)
            .map(|radius| Carrier::Circle { centre, radius })
            .collect(),
    }
}

/// Where two coplanar shapes meet.
fn meetings(a: Carrier, b: Carrier, normal: Point) -> Vec<Point> {
    match (a, b) {
        (
            Carrier::Line {
                through: p,
                along: u,
            },
            Carrier::Line {
                through: q,
                along: v,
            },
        ) => {
            let turn = dot(cross(u, v), normal);
            if turn.abs() < 1e-12 {
                return Vec::new();
            }
            vec![plus(p, u, dot(cross(sub(q, p), v), normal) / turn)]
        }
        (Carrier::Line { through, along }, Carrier::Circle { centre, radius })
        | (Carrier::Circle { centre, radius }, Carrier::Line { through, along }) => {
            let foot = plus(through, along, dot(sub(centre, through), along));
            let away = gap(foot, centre);
            if away > radius {
                return Vec::new();
            }
            let half = (radius * radius - away * away).sqrt();
            vec![plus(foot, along, half), plus(foot, along, -half)]
        }
        (
            Carrier::Circle {
                centre: c1,
                radius: r1,
            },
            Carrier::Circle {
                centre: c2,
                radius: r2,
            },
        ) => {
            let between = gap(c1, c2);
            if between < SAME_POINT || between > r1 + r2 || between < (r1 - r2).abs() {
                return Vec::new();
            }
            let Some(toward) = unit(sub(c2, c1)) else {
                return Vec::new();
            };
            let along = (between * between + r1 * r1 - r2 * r2) / (2.0 * between);
            let high = (r1 * r1 - along * along).max(0.0).sqrt();
            let middle = plus(c1, toward, along);
            let side = cross(normal, toward);
            vec![plus(middle, side, high), plus(middle, side, -high)]
        }
    }
}

/// Where a circle centred at `at` touches a shape.
fn touch_point(carrier: Carrier, at: Point) -> Option<Point> {
    match carrier {
        Carrier::Line { through, along } => {
            Some(plus(through, along, dot(sub(at, through), along)))
        }
        Carrier::Circle { centre, radius } => {
            unit(sub(at, centre)).map(|d| plus(centre, d, radius))
        }
    }
}

/// A curve cut back or carried out to `touch`, keeping the side that was picked.
fn kept_part(curve: &Curve, touch: Point, pick: Point) -> Result<Curve, KernelError> {
    match curve {
        Curve::Line { start, end } => {
            let along = unit(sub(*end, *start)).ok_or_else(|| bad("that line has no length"))?;
            let wanted = if dot(sub(pick, touch), along) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let reach = |p: Point| dot(sub(p, touch), along) * wanted;
            let far = if reach(*start) > reach(*end) {
                *start
            } else {
                *end
            };
            if reach(far) <= SAME_POINT {
                return Err(bad("that radius is too large for those curves"));
            }
            Curve::line(far, touch)
        }
        Curve::Arc { .. } => {
            let on = curve.closest_point(touch)?;
            if gap(on, touch) > SAME_POINT {
                return Err(bad("that radius is too large for that arc"));
            }
            let mut pieces = curve.cut_at(&[touch])?;
            if pieces.len() < 2 {
                return Err(bad("that radius leaves nothing of that arc"));
            }
            let mut best = 0;
            let mut nearest = f64::INFINITY;
            for (index, piece) in pieces.iter().enumerate() {
                let distance = gap(piece.closest_point(pick)?, pick);
                if distance < nearest {
                    nearest = distance;
                    best = index;
                }
            }
            Ok(pieces.swap_remove(best))
        }
        _ => Err(bad(
            "only lines, arcs and polyline segments can be filleted so far",
        )),
    }
}

/// The plane two curves lie on, for working out a fillet between them.
fn shared_normal(a: &Curve, b: &Curve) -> Option<Point> {
    let of = |curve: &Curve| match curve {
        Curve::Arc { x, y, .. } => unit(cross(*x, *y)),
        _ => None,
    };
    of(a).or_else(|| of(b))
}

/// The two kept parts and the arc, for a pair with at least one arc.
fn fillet_with_arc(
    first: &Curve,
    first_pick: Point,
    second: &Curve,
    second_pick: Point,
    radius: f64,
) -> Result<(Curve, Curve, Curve), KernelError> {
    let normal = shared_normal(first, second).ok_or_else(|| bad("those curves share no plane"))?;
    let (carrier_a, carrier_b) = (carrier_of(first)?, carrier_of(second)?);
    let mut best: Option<(f64, Point, Point, Point)> = None;
    for a in offsets_of(carrier_a, normal, radius) {
        for b in offsets_of(carrier_b, normal, radius) {
            for centre in meetings(a, b, normal) {
                let (Some(touch_a), Some(touch_b)) = (
                    touch_point(carrier_a, centre),
                    touch_point(carrier_b, centre),
                ) else {
                    continue;
                };
                let score = gap(touch_a, first_pick) + gap(touch_b, second_pick);
                if best.is_none_or(|(held, ..)| score < held) {
                    best = Some((score, centre, touch_a, touch_b));
                }
            }
        }
    }
    let (_, centre, touch_a, touch_b) =
        best.ok_or_else(|| bad("those curves cannot be rounded at that radius"))?;
    let kept_a = kept_part(first, touch_a, first_pick)?;
    let kept_b = kept_part(second, touch_b, second_pick)?;
    let x = unit(sub(touch_a, centre)).ok_or_else(|| bad("that radius is too small"))?;
    let turn = unit(cross(sub(touch_a, centre), sub(touch_b, centre))).unwrap_or(normal);
    let arc = Curve::arc(centre, touch_a, touch_b, x, cross(turn, x))?;
    Ok((kept_a, kept_b, arc))
}

/// Which segment of a path a point is on, with the segment as a line.
fn segment_at(path: &[Point], pick: Point) -> Result<(usize, Curve), KernelError> {
    let mut best: Option<(f64, usize)> = None;
    for (index, pair) in path.windows(2).enumerate() {
        let segment = Curve::line(pair[0], pair[1])?;
        let distance = gap(segment.closest_point(pick)?, pick);
        if best.is_none_or(|(held, _)| distance < held) {
            best = Some((distance, index));
        }
    }
    let (_, index) = best.ok_or_else(|| bad("that polyline has no segments"))?;
    Ok((index, Curve::line(path[index], path[index + 1])?))
}

fn piece_through(points: Vec<Point>) -> Result<Curve, KernelError> {
    if points.len() == 2 {
        Curve::line(points[0], points[1])
    } else {
        Curve::polyline(points, false)
    }
}

fn ends_of(curve: &Curve) -> Result<(Point, Point), KernelError> {
    let path = curve.sample()?;
    match (path.first(), path.last()) {
        (Some(first), Some(last)) => Ok((*first, *last)),
        _ => Err(bad("an empty curve")),
    }
}

/// Rounds two neighbouring segments of one polyline, giving one joined curve.
fn fillet_within(
    path: &[Point],
    closed: bool,
    picks: (Point, Point),
    radius: f64,
) -> Result<Vec<Curve>, KernelError> {
    let (index_a, line_a) = segment_at(path, picks.0)?;
    let (index_b, line_b) = segment_at(path, picks.1)?;
    let last = path.len() - 2;
    let (low, high, low_pick, high_pick, low_line, high_line) = if index_a < index_b {
        (index_a, index_b, picks.0, picks.1, line_a, line_b)
    } else {
        (index_b, index_a, picks.1, picks.0, line_b, line_a)
    };
    let neighbours = high == low + 1;
    let wraps = closed && low == 0 && high == last;
    if !neighbours && !wraps {
        return Err(bad("those segments do not share a corner"));
    }
    if wraps && !neighbours {
        // Walking from the seam, the last segment comes first, so turn the path to start there.
        let done = fillet_lines(&high_line, high_pick, &low_line, low_pick, radius)?;
        let (_, touch_high) = ends_of(&done.first)?;
        let (touch_low, _) = ends_of(&done.second)?;
        let mut around = vec![touch_low];
        around.extend_from_slice(&path[1..=last]);
        around.push(touch_high);
        let run = piece_through(around)?;
        return Ok(match done.arc {
            Some(arc) => vec![Curve::compound(vec![run, arc])?],
            None => vec![run],
        });
    }
    let done = fillet_lines(&low_line, low_pick, &high_line, high_pick, radius)?;
    let (_, touch_low) = ends_of(&done.first)?;
    let (touch_high, _) = ends_of(&done.second)?;
    let mut before = path[..=low].to_vec();
    before.push(touch_low);
    let mut after = vec![touch_high];
    after.extend_from_slice(&path[high + 1..]);
    let (before, after) = (piece_through(before)?, piece_through(after)?);
    Ok(vec![match done.arc {
        Some(arc) => Curve::compound(vec![before, arc, after])?,
        None => {
            let mut whole = path[..=low].to_vec();
            whole.push(touch_low);
            whole.extend_from_slice(&path[high + 1..]);
            piece_through(whole)?
        }
    }])
}

/// A polyline's end segment as a line, with the rest of the polyline to put back afterwards.
fn end_segment(curve: &Curve, pick: Point) -> Result<(Curve, Option<PolylineRest>), KernelError> {
    let Curve::Polyline { closed, .. } = curve else {
        return Ok((curve.clone(), None));
    };
    if *closed {
        return Err(bad("pick two neighbouring segments of a closed polyline"));
    }
    let path = curve.sample()?;
    let (index, line) = segment_at(&path, pick)?;
    if index == 0 {
        Ok((line, Some((path[1..].to_vec(), path[1]))))
    } else if index == path.len() - 2 {
        Ok((line, Some((path[..=index].to_vec(), path[index]))))
    } else {
        Err(bad(
            "only the end segments of a polyline can be filleted to another curve",
        ))
    }
}

/// A kept line put back onto the rest of its polyline.
fn rebuilt(kept: Curve, rest: Option<PolylineRest>) -> Result<Curve, KernelError> {
    let Some((mut path, joint)) = rest else {
        return Ok(kept);
    };
    let (far, touch) = ends_of(&kept)?;
    let free = if gap(far, joint) <= SAME_POINT {
        touch
    } else {
        far
    };
    if gap(path[0], joint) <= SAME_POINT {
        path.insert(0, free);
    } else {
        path.push(free);
    }
    piece_through(path)
}

/// Rounds the corner between two curves, keeping the sides picked. Returns the curves that
/// replace them. `same` says both picks are on one polyline.
pub fn fillet_curves(
    first: &Curve,
    first_pick: Point,
    second: &Curve,
    second_pick: Point,
    radius: f64,
    same: bool,
) -> Result<Vec<Curve>, KernelError> {
    if same {
        let Curve::Polyline { closed, .. } = first else {
            return Err(bad("pick two different curves"));
        };
        return fillet_within(&first.sample()?, *closed, (first_pick, second_pick), radius);
    }
    let (line_a, rest_a) = end_segment(first, first_pick)?;
    let (line_b, rest_b) = end_segment(second, second_pick)?;
    let has_arc = |c: &Curve| matches!(c, Curve::Arc { .. });
    let (kept_a, kept_b, arc) = if has_arc(&line_a) || has_arc(&line_b) {
        if radius == 0.0 {
            return Err(bad("a sharp corner needs two lines"));
        }
        let (a, b, arc) = fillet_with_arc(&line_a, first_pick, &line_b, second_pick, radius)?;
        (a, b, Some(arc))
    } else {
        let done = fillet_lines(&line_a, first_pick, &line_b, second_pick, radius)?;
        (done.first, done.second, done.arc)
    };
    let mut made = vec![rebuilt(kept_a, rest_a)?, rebuilt(kept_b, rest_b)?];
    made.extend(arc);
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const X: Point = [1.0, 0.0, 0.0];
    const Y: Point = [0.0, 1.0, 0.0];

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn total(curves: &[Curve]) -> f64 {
        curves.iter().map(|c| c.length().unwrap()).sum()
    }

    fn line(a: Point, b: Point) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn corner() -> Curve {
        Curve::polyline(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false).unwrap()
    }

    #[test]
    fn neighbouring_segments_of_a_polyline_round_into_one_curve() {
        let curve = corner();
        let made =
            fillet_curves(&curve, [1.0, 0.0, 0.0], &curve, [10.0, 9.0, 0.0], 2.0, true).unwrap();
        assert_eq!(made.len(), 1);
        assert!(matches!(made[0], Curve::Compound { .. }));
        assert!(near(total(&made), 16.0 + 2.0 * FRAC_PI_2));
    }

    #[test]
    fn a_longer_polyline_keeps_its_other_corners() {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [20.0, 10.0, 0.0],
        ];
        let curve = Curve::polyline(path, false).unwrap();
        let made =
            fillet_curves(&curve, [1.0, 0.0, 0.0], &curve, [10.0, 9.0, 0.0], 2.0, true).unwrap();
        assert!(near(total(&made), 26.0 + 2.0 * FRAC_PI_2));
    }

    #[test]
    fn a_sharp_corner_within_a_polyline_stays_a_polyline() {
        let curve = corner();
        let made =
            fillet_curves(&curve, [1.0, 0.0, 0.0], &curve, [10.0, 9.0, 0.0], 0.0, true).unwrap();
        assert!(near(total(&made), 20.0));
    }

    #[test]
    fn segments_that_do_not_share_a_corner_are_refused() {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [20.0, 10.0, 0.0],
        ];
        let curve = Curve::polyline(path, false).unwrap();
        assert!(
            fillet_curves(
                &curve,
                [1.0, 0.0, 0.0],
                &curve,
                [15.0, 10.0, 0.0],
                1.0,
                true
            )
            .is_err()
        );
    }

    #[test]
    fn a_closed_polyline_rounds_across_its_seam() {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let curve = Curve::polyline(path, true).unwrap();
        let made =
            fillet_curves(&curve, [5.0, 0.0, 0.0], &curve, [0.0, 5.0, 0.0], 2.0, true).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(total(&made), 36.0 + 2.0 * FRAC_PI_2));
    }

    #[test]
    fn a_polyline_end_rounds_to_a_separate_line() {
        let curve = corner();
        let other = line([10.0, 10.0, 0.0], [20.0, 10.0, 0.0]);
        let made = fillet_curves(
            &curve,
            [10.0, 1.0, 0.0],
            &other,
            [19.0, 10.0, 0.0],
            2.0,
            false,
        )
        .unwrap();
        assert_eq!(made.len(), 3);
        assert!(near(total(&made), 26.0 + 2.0 * FRAC_PI_2));
    }

    #[test]
    fn a_middle_segment_cannot_round_to_a_separate_curve() {
        let path = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [20.0, 10.0, 0.0],
        ];
        let curve = Curve::polyline(path, false).unwrap();
        let other = line([0.0, 5.0, 0.0], [30.0, 5.0, 0.0]);
        assert!(
            fillet_curves(
                &curve,
                [10.0, 4.0, 0.0],
                &other,
                [20.0, 5.0, 0.0],
                1.0,
                false
            )
            .is_err()
        );
    }

    /// A line along y = 0 that runs into a quarter arc around the origin of radius 10.
    fn line_and_arc() -> (Curve, Curve) {
        let arc = Curve::arc([0.0; 3], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], X, Y).unwrap();
        (line([30.0, 0.0, 0.0], [10.0, 0.0, 0.0]), arc)
    }

    #[test]
    fn a_line_and_an_arc_round_with_a_tangent_arc() {
        let (a, b) = line_and_arc();
        let on_arc = [10.0 * 0.5_f64.cos(), 10.0 * 0.5_f64.sin(), 0.0];
        let made = fillet_curves(&a, [28.0, 0.0, 0.0], &b, on_arc, 2.0, false).unwrap();
        assert_eq!(made.len(), 3);
        let round = &made[2];
        let points = round.sample().unwrap();
        // The arc's centre is 2 from the line, and 12 from the big arc's centre.
        let centre_height = 2.0;
        let centre_x = (12.0_f64 * 12.0 - centre_height * centre_height).sqrt();
        let centre = [centre_x, centre_height, 0.0];
        for point in points {
            assert!(near(gap(point, centre), 2.0), "{point:?}");
        }
    }

    #[test]
    fn the_fillet_ends_touch_what_is_kept() {
        let (a, b) = line_and_arc();
        let on_arc = [10.0 * 0.5_f64.cos(), 10.0 * 0.5_f64.sin(), 0.0];
        let made = fillet_curves(&a, [28.0, 0.0, 0.0], &b, on_arc, 2.0, false).unwrap();
        let (start, end) = ends_of(&made[2]).unwrap();
        assert!(gap(made[0].closest_point(start).unwrap(), start) < 1e-6);
        assert!(gap(made[1].closest_point(end).unwrap(), end) < 1e-6);
    }

    #[test]
    fn two_arcs_can_be_rounded() {
        let a = Curve::arc([0.0; 3], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], X, Y).unwrap();
        let b = Curve::arc([18.0, 0.0, 0.0], [18.0, 10.0, 0.0], [8.0, 0.0, 0.0], X, Y).unwrap();
        let on_a = [10.0 * 0.9_f64.cos(), 10.0 * 0.9_f64.sin(), 0.0];
        let on_b = [18.0 - 10.0 * 0.9_f64.cos(), 10.0 * 0.9_f64.sin(), 0.0];
        let made = fillet_curves(&a, on_a, &b, on_b, 1.0, false).unwrap();
        assert_eq!(made.len(), 3);
        let (start, end) = ends_of(&made[2]).unwrap();
        assert!(gap(made[0].closest_point(start).unwrap(), start) < 1e-6);
        assert!(gap(made[1].closest_point(end).unwrap(), end) < 1e-6);
    }

    #[test]
    fn an_arc_needs_a_radius() {
        let (a, b) = line_and_arc();
        assert!(fillet_curves(&a, [28.0, 0.0, 0.0], &b, [8.0, 6.0, 0.0], 0.0, false).is_err());
    }

    #[test]
    fn smooth_curves_are_refused_for_now() {
        let a = line([0.0; 3], X);
        let b = Curve::interpolated(vec![X, [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        assert!(fillet_curves(&a, X, &b, [5.0, 3.0, 0.0], 1.0, false).is_err());
    }
}
