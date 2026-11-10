//! Rounding every corner of a polyline at once.

use crate::{Curve, KernelError, Point, fillet_lines, gap};

/// Corners closer to straight than this are left alone.
const STRAIGHT: f64 = 1e-9;

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

/// Whether `b` is a real corner between `a` and `c`, not a point along a straight run.
fn turns(a: Point, b: Point, c: Point) -> bool {
    let (u, v) = (crate::sub(b, a), crate::sub(c, b));
    let across = crate::cross(u, v);
    gap(across, [0.0; 3]) > STRAIGHT * gap(u, [0.0; 3]) * gap(v, [0.0; 3])
}

/// One rounded corner: where the arc leaves the incoming segment, the arc, and where it joins
/// the outgoing one.
struct Rounded {
    enter: Point,
    arc: Curve,
    leave: Point,
}

fn round(before: Point, corner: Point, after: Point, radius: f64) -> Result<Rounded, KernelError> {
    let incoming = Curve::line(before, corner)?;
    let outgoing = Curve::line(corner, after)?;
    let done = fillet_lines(&incoming, before, &outgoing, after, radius)?;
    let enter = *done
        .first
        .sample()?
        .last()
        .ok_or_else(|| bad("an empty line"))?;
    let leave = *done
        .second
        .sample()?
        .first()
        .ok_or_else(|| bad("an empty line"))?;
    let arc = done
        .arc
        .ok_or_else(|| bad("the radius must be more than zero"))?;
    Ok(Rounded { enter, arc, leave })
}

impl Curve {
    /// This polyline with every corner rounded by an arc of `radius`, as one joined curve.
    pub fn fillet_corners(&self, radius: f64) -> Result<Self, KernelError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(bad("the radius must be more than zero"));
        }
        let Self::Polyline { points, closed } = self else {
            return Err(bad("only polylines have corners to round"));
        };
        let mut path = points.clone();
        if *closed && path.len() > 1 && gap(path[0], path[path.len() - 1]) <= STRAIGHT {
            path.pop();
        }
        let count = path.len();
        let corner_at = |index: usize| -> Option<(Point, Point, Point)> {
            let (before, after) = if *closed {
                ((index + count - 1) % count, (index + 1) % count)
            } else if index == 0 || index + 1 == count {
                return None;
            } else {
                (index - 1, index + 1)
            };
            let (a, b, c) = (path[before], path[index], path[after]);
            turns(a, b, c).then_some((a, b, c))
        };
        let mut rounded: Vec<Option<Rounded>> = Vec::with_capacity(count);
        for index in 0..count {
            rounded.push(match corner_at(index) {
                Some((a, b, c)) => Some(round(a, b, c, radius)?),
                None => None,
            });
        }
        if rounded.iter().all(Option::is_none) {
            return Err(bad("the polyline has no corners"));
        }
        // Each segment must be long enough for the arcs at both of its ends.
        let segments = if *closed { count } else { count - 1 };
        for index in 0..segments {
            let next = (index + 1) % count;
            let start = rounded[index].as_ref().map_or(path[index], |r| r.leave);
            let end = rounded[next].as_ref().map_or(path[next], |r| r.enter);
            let used = gap(path[index], start) + gap(end, path[next]);
            if used > gap(path[index], path[next]) + 1e-9 {
                return Err(bad("the radius is too big for the segments"));
            }
        }
        let mut parts = Vec::new();
        let run_start = if *closed {
            rounded[0].as_ref().map_or(path[0], |r| r.leave)
        } else {
            path[0]
        };
        let mut run: Vec<Point> = vec![run_start];
        let finish_run = |run: &mut Vec<Point>, parts: &mut Vec<Self>| -> Result<(), KernelError> {
            run.dedup_by(|a, b| gap(*a, *b) <= STRAIGHT);
            match run.len() {
                0 | 1 => {}
                2 => parts.push(Self::line(run[0], run[1])?),
                _ => parts.push(Self::polyline(run.clone(), false)?),
            }
            Ok(())
        };
        let order: Vec<usize> = if *closed {
            (1..count).chain(std::iter::once(0)).collect()
        } else {
            (1..count).collect()
        };
        for index in order {
            match &rounded[index] {
                Some(corner) => {
                    run.push(corner.enter);
                    finish_run(&mut run, &mut parts)?;
                    parts.push(corner.arc.clone());
                    run = vec![corner.leave];
                }
                None => run.push(path[index]),
            }
        }
        finish_run(&mut run, &mut parts)?;
        Self::compound(parts)
    }
}

#[cfg(test)]
mod tests {
    use crate::Curve;
    use std::f64::consts::PI;

    fn square(closed: bool) -> Curve {
        Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            closed,
        )
        .unwrap()
    }

    #[test]
    fn a_closed_square_gets_four_arcs_and_the_right_length() {
        let rounded = square(true).fillet_corners(2.0).unwrap();
        let Curve::Compound { segments } = &rounded else {
            panic!("expected a joined curve, got {rounded:?}");
        };
        let arcs = segments
            .iter()
            .filter(|s| matches!(s, Curve::Arc { .. }))
            .count();
        assert_eq!(arcs, 4);
        let expected = 4.0 * 6.0 + 2.0 * PI * 2.0;
        assert!((rounded.length().unwrap() - expected).abs() < 1e-6);
    }

    #[test]
    fn an_open_polyline_keeps_its_ends() {
        let rounded = square(false).fillet_corners(1.0).unwrap();
        let path = rounded.sample().unwrap();
        assert_eq!(path[0], [0.0, 0.0, 0.0]);
        let last = path[path.len() - 1];
        assert!((last[0]).abs() < 1e-9 && (last[1] - 10.0).abs() < 1e-9);
        let expected = 9.0 + 8.0 + 9.0 + PI;
        assert!((rounded.length().unwrap() - expected).abs() < 1e-6);
    }

    #[test]
    fn too_big_a_radius_or_no_corners_is_refused() {
        assert!(square(true).fillet_corners(6.0).is_err());
        let straight =
            Curve::polyline(vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], false).unwrap();
        assert!(straight.fillet_corners(0.1).is_err());
        assert!(square(true).fillet_corners(0.0).is_err());
    }
}
