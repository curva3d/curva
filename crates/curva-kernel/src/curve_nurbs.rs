//! Lines, polylines, circles, arcs and compound curves as exact NURBS curves, as Rhino 8's
//! ToNURBS makes them (#550, measured by the mesh stream): a line is degree 1 with 2 points, a
//! circle degree 2 rational with 9 points (double knots at the quarters), and a line joined to a
//! half circle one degree-2 curve of 7 points, the line raised to degree 2.

use crate::{Curve, KernelError, Point};
use std::f64::consts::{FRAC_PI_2, TAU};

/// A NURBS curve's parts, a weight for every point.
pub(crate) struct Parts {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<Point>,
    weights: Vec<f64>,
}

impl Parts {
    /// The curve, with no weights kept when all are 1.
    fn curve(self) -> Result<Curve, KernelError> {
        let weights = if self.weights.iter().all(|w| (w - 1.0).abs() < 1e-15) {
            Vec::new()
        } else {
            self.weights
        };
        Curve::nurbs(self.degree, self.knots, self.points, weights)
    }

    /// Straight segments through `points`, degree 1, parameter counting the points as Rhino's
    /// polylines do.
    fn straight(points: &[Point]) -> Self {
        let count = points.len();
        let mut knots = vec![0.0];
        #[allow(clippy::cast_precision_loss)]
        knots.extend((0..count).map(|i| i as f64));
        #[allow(clippy::cast_precision_loss)]
        knots.push((count - 1) as f64);
        Self {
            degree: 1,
            knots,
            points: points.to_vec(),
            weights: vec![1.0; count],
        }
    }

    /// An arc of `sweep` radians from `x` towards `y`: spans of a quarter turn at most, each
    /// with its middle point where the end tangents meet, weighted by the cosine of half its
    /// turn; the parameter is the angle.
    fn arc(centre: Point, radius: f64, x: Point, y: Point, sweep: f64) -> Self {
        // A little over a quarter (rounding) still takes one span.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let spans = ((sweep / FRAC_PI_2) - 1e-9).ceil().max(1.0) as usize;
        #[allow(clippy::cast_precision_loss)]
        let step = sweep / spans as f64;
        let at = |angle: f64, out: f64| -> Point {
            let (s, c) = angle.sin_cos();
            std::array::from_fn(|k| centre[k] + out * (x[k] * c + y[k] * s))
        };
        let middle = (step / 2.0).cos();
        let mut parts = Self {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0],
            points: vec![at(0.0, radius)],
            weights: vec![1.0],
        };
        for span in 1..=spans {
            #[allow(clippy::cast_precision_loss)]
            let end = step * span as f64;
            parts.points.push(at(end - step / 2.0, radius / middle));
            parts.weights.push(middle);
            parts.points.push(at(end, radius));
            parts.weights.push(1.0);
            parts.knots.extend([end, end]);
        }
        parts.knots.push(sweep);
        parts
    }

    /// The same curve raised to `degree`, for straight pieces (degree 1) only: each segment's
    /// points evenly along it. `None` for a curved piece of a lower degree.
    fn raised(self, degree: usize) -> Option<Self> {
        if self.degree == degree {
            return Some(self);
        }
        if self.degree != 1 {
            return None;
        }
        let mut raised = Self {
            degree,
            knots: vec![self.knots[1]; degree + 1],
            points: vec![self.points[0]],
            weights: vec![1.0],
        };
        for (k, pair) in self.points.windows(2).enumerate() {
            for step in 1..=degree {
                #[allow(clippy::cast_precision_loss)]
                let t = step as f64 / degree as f64;
                raised.points.push(std::array::from_fn(|i| {
                    pair[0][i] + t * (pair[1][i] - pair[0][i])
                }));
                raised.weights.push(1.0);
            }
            raised.knots.extend(vec![self.knots[k + 2]; degree]);
        }
        raised.knots.push(*raised.knots.last()?);
        Some(raised)
    }

    /// `next` joined on to its end, its parameter carried on and its weights scaled to meet:
    /// the join is a knot of full multiplicity, as Rhino joins curves end to end.
    fn joined(mut self, next: Self) -> Self {
        let shift = self.knots.last().copied().unwrap_or(0.0) - next.knots[0];
        let scale = self.weights.last().copied().unwrap_or(1.0) / next.weights[0];
        self.knots.pop();
        self.knots
            .extend(next.knots[self.degree + 1..].iter().map(|k| k + shift));
        self.points.extend(next.points.into_iter().skip(1));
        self.weights
            .extend(next.weights.into_iter().skip(1).map(|w| w * scale));
        self
    }
}

impl Curve {
    /// The parts of this curve as an exact NURBS curve.
    pub(crate) fn nurbs_parts(&self) -> Result<Parts, KernelError> {
        Ok(match self {
            Self::Line { start, end } => Parts::straight(&[*start, *end]),
            Self::Polyline { points, closed } => {
                let mut all = points.clone();
                if *closed {
                    all.push(points[0]);
                }
                Parts::straight(&all)
            }
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => Parts::arc(*centre, *radius, *x, *y, TAU),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => Parts::arc(*centre, *radius, *x, *y, *sweep),
            Self::Compound { segments } => compound_parts(segments)?,
            Self::Point { .. } => {
                return Err(KernelError("a point is not a curve".to_owned()));
            }
            Self::Nurbs { .. } | Self::Interpolated { .. } => {
                let Self::Nurbs {
                    degree,
                    knots,
                    points,
                    weights,
                } = self.to_nurbs()?
                else {
                    return Err(KernelError("not a NURBS curve".to_owned()));
                };
                let weights = if weights.is_empty() {
                    vec![1.0; points.len()]
                } else {
                    weights
                };
                Parts {
                    degree,
                    knots,
                    points,
                    weights,
                }
            }
        })
    }

    /// Lines, polylines, circles, arcs and compound curves as exact NURBS curves (#550).
    pub(crate) fn kinds_to_nurbs(&self) -> Result<Self, KernelError> {
        self.nurbs_parts()?.curve()
    }
}

/// A compound curve's parts joined end to end, straight ones raised to the highest degree.
fn compound_parts(segments: &[Curve]) -> Result<Parts, KernelError> {
    let parts = segments
        .iter()
        .map(Curve::nurbs_parts)
        .collect::<Result<Vec<_>, _>>()?;
    let degree = parts.iter().map(|p| p.degree).max().unwrap_or(1);
    let mut raised = parts.into_iter().map(|p| {
        p.raised(degree).ok_or_else(|| {
            KernelError("a curved part of a lower degree cannot be raised yet".to_owned())
        })
    });
    let first = raised
        .next()
        .ok_or_else(|| KernelError("an empty compound curve".to_owned()))??;
    raised.try_fold(first, |joined, next| Ok(joined.joined(next?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nurbs(curve: &Curve) -> (usize, usize, bool) {
        match curve.to_nurbs().unwrap() {
            Curve::Nurbs {
                degree,
                points,
                weights,
                ..
            } => (degree, points.len(), !weights.is_empty()),
            _ => panic!("not NURBS"),
        }
    }

    fn on_circle(curve: &Curve, centre: Point, radius: f64) {
        for p in curve.to_nurbs().unwrap().sample().unwrap() {
            let r = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt();
            assert!((r - radius).abs() < 1e-9, "{p:?} at {r}");
        }
    }

    #[test]
    fn they_become_nurbs_as_rhino_makes_them() {
        // Rhino 8: a line is degree 1 with 2 points; a circle degree 2, rational, 9 points.
        let line = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        assert_eq!(nurbs(&line), (1, 2, false));
        let circle = Curve::circle([1.0, 2.0, 0.0], 5.0).unwrap();
        assert_eq!(nurbs(&circle), (2, 9, true));
        on_circle(&circle, [1.0, 2.0, 0.0], 5.0);
        let length = circle.to_nurbs().unwrap().length().unwrap();
        assert!((length - TAU * 5.0).abs() < 1e-6, "{length}");
        // A line joined to a half circle: one degree-2 curve of 7 points.
        let half = Curve::Arc {
            centre: [10.0, 5.0, 0.0],
            radius: 5.0,
            x: [0.0, -1.0, 0.0],
            y: [1.0, 0.0, 0.0],
            sweep: std::f64::consts::PI,
        };
        let joined = Curve::compound(vec![line.clone(), half]).unwrap();
        assert_eq!(nurbs(&joined), (2, 7, true));
        let length = joined.to_nurbs().unwrap().length().unwrap();
        assert!(
            (length - (10.0 + std::f64::consts::PI * 5.0)).abs() < 1e-6,
            "{length}"
        );
        // A closed polyline: degree 1, back to its start.
        let square = Curve::polyline(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            true,
        )
        .unwrap();
        assert_eq!(nurbs(&square), (1, 5, false));
        assert!((square.to_nurbs().unwrap().length().unwrap() - 4.0).abs() < 1e-9);
        // An arc of 100 degrees takes two spans.
        let arc = Curve::Arc {
            centre: [0.0; 3],
            radius: 2.0,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            sweep: 100f64.to_radians(),
        };
        assert_eq!(nurbs(&arc), (2, 5, true));
        on_circle(&arc, [0.0; 3], 2.0);
    }
}
