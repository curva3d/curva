//! Curves part of the way from one curve to another.

use crate::{Curve, KernelError, Point, dot, sub};

/// How many points a general tween passes through.
const TWEEN_POINTS: usize = 32;

fn blend(a: Point, b: Point, t: f64) -> Point {
    std::array::from_fn(|axis| a[axis] + (b[axis] - a[axis]) * t)
}

fn blend_all(a: &[Point], b: &[Point], t: f64) -> Vec<Point> {
    a.iter().zip(b).map(|(p, q)| blend(*p, *q, t)).collect()
}

/// `b`'s points turned round the loop, and reversed if that fits better, so that each lines up
/// with the matching point of `a` once both loops are centred. This keeps tweens from twisting.
fn lined_up(a: &[Point], b: &[Point]) -> Vec<Point> {
    let centre = |points: &[Point]| -> Point {
        let count = points.len().max(1) as f64;
        std::array::from_fn(|axis| points.iter().map(|p| p[axis]).sum::<f64>() / count)
    };
    let (middle_a, middle_b) = (centre(a), centre(b));
    let count = b.len();
    let mut best = (f64::INFINITY, b.to_vec());
    for reversed in [false, true] {
        let ordered: Vec<Point> = if reversed {
            b.iter().rev().copied().collect()
        } else {
            b.to_vec()
        };
        for shift in 0..count {
            let misfit: f64 = (0..count)
                .map(|i| {
                    let p = sub(a[i], middle_a);
                    let q = sub(ordered[(i + shift) % count], middle_b);
                    let d = sub(p, q);
                    dot(d, d)
                })
                .sum();
            if misfit < best.0 {
                let turned = (0..count).map(|i| ordered[(i + shift) % count]).collect();
                best = (misfit, turned);
            }
        }
    }
    best.1
}

/// The corners of a straight-sided curve and whether it closes, if it is one.
fn corners(curve: &Curve) -> Option<(Vec<Point>, bool)> {
    match curve {
        Curve::Line { start, end } => Some((vec![*start, *end], false)),
        Curve::Polyline { points, closed } => Some((points.clone(), *closed)),
        _ => None,
    }
}

/// Whether a curve ends where it starts.
fn closes(curve: &Curve) -> Result<bool, KernelError> {
    Ok(matches!(curve, Curve::Circle { .. }) || curve.is_closed()?)
}

/// `count` curves evenly spaced between `from` and `to`, not including either.
///
/// Straight-sided curves with the same number of corners blend corner by corner and stay
/// straight-sided. Two circles stay circles. Anything else blends through evenly spaced points.
pub fn tween_curves(from: &Curve, to: &Curve, count: usize) -> Result<Vec<Curve>, KernelError> {
    if count == 0 {
        return Err(KernelError("ask for at least one curve".to_owned()));
    }
    let steps = (1..=count).map(|step| step as f64 / (count + 1) as f64);
    if let (
        Curve::Circle {
            centre: c1,
            radius: r1,
            x: x1,
            y: y1,
        },
        Curve::Circle {
            centre: c2,
            radius: r2,
            x: x2,
            y: y2,
        },
    ) = (from, to)
        && x1 == x2
        && y1 == y2
    {
        return steps
            .map(|t| Curve::circle_on(blend(*c1, *c2, t), r1 + (r2 - r1) * t, *x1, *y1))
            .collect();
    }
    if let (Some((a, a_closed)), Some((b, b_closed))) = (corners(from), corners(to))
        && a.len() == b.len()
        && a_closed == b_closed
    {
        return steps
            .map(|t| {
                let points = blend_all(&a, &b, t);
                if points.len() == 2 {
                    Curve::line(points[0], points[1])
                } else {
                    Curve::polyline(points, a_closed)
                }
            })
            .collect();
    }
    let closed = closes(from)? && closes(to)?;
    let pieces = if closed {
        TWEEN_POINTS
    } else {
        TWEEN_POINTS - 1
    };
    let mut a = from.divided(pieces)?;
    let mut b = to.divided(pieces)?;
    if closed {
        // The seam point comes back at the end; an interpolated closed curve adds it itself.
        a.truncate(TWEEN_POINTS);
        b.truncate(TWEEN_POINTS);
        b = lined_up(&a, &b);
    }
    steps
        .map(|t| Curve::interpolated(blend_all(&a, &b, t), closed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_starting_elsewhere_and_running_backwards_is_lined_up() {
        let a = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let lifted = |p: Point| [p[0], p[1], 5.0];
        let b = [lifted(a[2]), lifted(a[1]), lifted(a[0]), lifted(a[3])];
        assert_eq!(lined_up(&a, &b), a.map(lifted));
    }

    #[test]
    fn lines_blend_end_to_end() {
        let a = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([0.0, 10.0, 0.0], [10.0, 20.0, 0.0]).unwrap();
        let made = tween_curves(&a, &b, 1).unwrap();
        assert_eq!(
            made,
            [Curve::line([0.0, 5.0, 0.0], [10.0, 10.0, 0.0]).unwrap()]
        );
    }

    #[test]
    fn two_circles_give_circles_of_growing_size() {
        let a = Curve::circle([0.0; 3], 1.0).unwrap();
        let b = Curve::circle([0.0, 0.0, 9.0], 4.0).unwrap();
        let made = tween_curves(&a, &b, 2).unwrap();
        assert_eq!(made[0], Curve::circle([0.0, 0.0, 3.0], 2.0).unwrap());
        assert_eq!(made[1], Curve::circle([0.0, 0.0, 6.0], 3.0).unwrap());
    }

    #[test]
    fn a_square_and_a_circle_blend_through_points_and_stay_closed() {
        let square = Curve::polyline(
            vec![
                [-2.0, -2.0, 0.0],
                [2.0, -2.0, 0.0],
                [2.0, 2.0, 0.0],
                [-2.0, 2.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let circle = Curve::circle([0.0, 0.0, 4.0], 2.0).unwrap();
        let made = tween_curves(&square, &circle, 1).unwrap();
        let path = made[0].sample().unwrap();
        assert!(path.iter().all(|p| (p[2] - 2.0).abs() < 1e-6));
        let (first, last) = (path[0], path[path.len() - 1]);
        assert!(crate::gap(first, last) < 1e-6);
    }

    #[test]
    fn none_is_refused() {
        let a = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        assert!(tween_curves(&a, &a, 0).is_err());
    }
}
