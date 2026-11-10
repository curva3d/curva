//! The points a user can pick up and move to reshape a curve.

use crate::conic::conic;
use crate::{Curve, JOIN_TOLERANCE, KernelError, Point, gap};
use std::f64::consts::TAU;

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

fn scaled(vector: Point, by: f64) -> Point {
    vector.map(|value| value * by)
}

/// Whether a NURBS curve's last control point repeats its first, so the two move as one.
fn repeats_first(points: &[Point]) -> bool {
    points.len() > 2 && gap(points[0], points[points.len() - 1]) <= JOIN_TOLERANCE
}

impl Curve {
    /// The form a curve takes once its points are edited: circles and arcs become NURBS, as
    /// their own description has no points to move. Compound curves must be exploded first.
    fn editable(&self) -> Result<Self, KernelError> {
        match self {
            Self::Circle {
                centre,
                radius,
                x,
                y,
            } => conic(*centre, scaled(*x, *radius), scaled(*y, *radius), TAU),
            Self::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => conic(*centre, scaled(*x, *radius), scaled(*y, *radius), *sweep),
            Self::Compound { .. } => Err(bad("explode a joined curve to edit its points")),
            Self::Point { .. } => Err(bad("a point has no control points")),
            other => Ok(other.clone()),
        }
    }

    /// The points that shape the curve: corners of straight-sided curves, the points a smooth
    /// curve passes through, or NURBS control points. A closed NURBS curve's repeated last
    /// point is left out, as it moves with the first.
    pub fn control_points(&self) -> Result<Vec<Point>, KernelError> {
        Ok(match self.editable()? {
            Self::Line { start, end } => vec![start, end],
            Self::Polyline { points, .. } | Self::Interpolated { points, .. } => points,
            Self::Nurbs { mut points, .. } => {
                if repeats_first(&points) {
                    points.pop();
                }
                points
            }
            _ => return Err(bad("this curve has no control points")),
        })
    }

    /// The curve reshaped with `points` in place of its [`Self::control_points`]. The number
    /// of points must stay the same.
    pub fn with_control_points(&self, points: Vec<Point>) -> Result<Self, KernelError> {
        if points.len() != self.control_points()?.len() {
            return Err(bad("the number of control points changed"));
        }
        match self.editable()? {
            Self::Line { .. } => Self::line(points[0], points[1]),
            Self::Polyline { closed, .. } => Self::polyline(points, closed),
            Self::Interpolated { closed, .. } => Self::interpolated(points, closed),
            Self::Nurbs {
                degree,
                knots,
                points: held,
                weights,
            } => {
                let mut points = points;
                if repeats_first(&held) {
                    points.push(points[0]);
                }
                Self::nurbs(degree, knots, points, weights)
            }
            _ => Err(bad("this curve has no control points")),
        }
    }

    /// The curve with one control point taken away. A line cannot lose a point, and a NURBS
    /// curve keeps at least one more point than its degree.
    pub fn without_control_point(&self, index: usize) -> Result<Self, KernelError> {
        let mut points = self.control_points()?;
        if index >= points.len() {
            return Err(bad("there is no such control point"));
        }
        points.remove(index);
        match self.editable()? {
            Self::Polyline { closed, .. } if points.len() >= 2 => Self::polyline(points, closed),
            Self::Interpolated { closed, .. } if points.len() >= 2 => {
                Self::interpolated(points, closed)
            }
            Self::Nurbs { .. } => self.to_points_curve(points),
            _ => Err(bad("the curve needs the points it has")),
        }
    }

    /// The curve with a new control point inserted before `index`, at `at`.
    pub fn with_control_point_inserted(
        &self,
        index: usize,
        at: Point,
    ) -> Result<Self, KernelError> {
        let mut points = self.control_points()?;
        if index > points.len() {
            return Err(bad("there is no such control point"));
        }
        points.insert(index, at);
        match self.editable()? {
            Self::Line { .. } => Self::polyline(points, false),
            Self::Polyline { closed, .. } => Self::polyline(points, closed),
            Self::Interpolated { closed, .. } => Self::interpolated(points, closed),
            Self::Nurbs { .. } => self.to_points_curve(points),
            _ => Err(bad("this curve has no control points")),
        }
    }

    /// A NURBS curve of the same degree, with evenly spaced knots, on new control points.
    fn to_points_curve(&self, points: Vec<Point>) -> Result<Self, KernelError> {
        let Self::Nurbs {
            degree,
            points: held,
            ..
        } = self.editable()?
        else {
            return Err(bad("not a NURBS curve"));
        };
        let mut points = points;
        if repeats_first(&held) {
            points.push(points[0]);
        }
        let degree = degree.min(points.len().saturating_sub(1)).max(1);
        let inner = points.len() - degree - 1;
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..=inner).map(|k| k as f64));
        knots.extend(std::iter::repeat_n((inner + 1) as f64, degree + 1));
        Self::nurbs(degree, knots, points, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Curve, gap};

    #[test]
    fn a_polyline_moves_one_corner_and_keeps_the_rest() {
        let curve =
            Curve::polyline(vec![[0.0; 3], [5.0, 0.0, 0.0], [5.0, 5.0, 0.0]], false).unwrap();
        let mut points = curve.control_points().unwrap();
        points[1] = [6.0, -1.0, 0.0];
        let moved = curve.with_control_points(points.clone()).unwrap();
        assert_eq!(moved.sample().unwrap(), points);
    }

    #[test]
    fn a_circle_shows_nurbs_points_and_stays_closed_when_one_moves() {
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let mut points = circle.control_points().unwrap();
        assert!(points.len() >= 6, "{points:?}");
        points[0][0] += 1.0;
        let moved = circle.with_control_points(points).unwrap();
        assert!(matches!(moved, Curve::Nurbs { .. }));
        let path = moved.sample().unwrap();
        assert!(gap(path[0], path[path.len() - 1]) < 1e-9);
    }

    #[test]
    fn points_can_be_removed_and_inserted() {
        let curve = Curve::interpolated(
            vec![[0.0; 3], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0], [3.0, 1.0, 0.0]],
            false,
        )
        .unwrap();
        let fewer = curve.without_control_point(1).unwrap();
        assert_eq!(fewer.control_points().unwrap().len(), 3);
        let more = curve
            .with_control_point_inserted(2, [1.5, 2.0, 0.0])
            .unwrap();
        assert_eq!(more.control_points().unwrap()[2], [1.5, 2.0, 0.0]);
        let line = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        assert!(line.without_control_point(0).is_err());
        assert!(matches!(
            line.with_control_point_inserted(1, [0.5, 1.0, 0.0])
                .unwrap(),
            Curve::Polyline { .. }
        ));
    }

    #[test]
    fn a_nurbs_curve_loses_a_point_but_keeps_its_ends() {
        let curve = Curve::nurbs(
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            vec![
                [0.0; 3],
                [1.0, 2.0, 0.0],
                [2.0, -1.0, 0.0],
                [3.0, 2.0, 0.0],
                [4.0, 0.0, 0.0],
            ],
            Vec::new(),
        )
        .unwrap();
        let fewer = curve.without_control_point(2).unwrap();
        let path = fewer.sample().unwrap();
        assert!(gap(path[0], [0.0; 3]) < 1e-9);
        assert!(gap(path[path.len() - 1], [4.0, 0.0, 0.0]) < 1e-9);
    }

    #[test]
    fn a_joined_curve_must_be_exploded_first() {
        let joined = Curve::compound(vec![
            Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap(),
            Curve::line([1.0, 0.0, 0.0], [1.0, 1.0, 0.0]).unwrap(),
        ])
        .unwrap();
        assert!(joined.control_points().is_err());
    }
}
