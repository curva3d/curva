//! Curva's curves and bodies to and from the engine's portable records, so that solidcore
//! operations that take and give records can be used directly.

use crate::body::{Body, failed};
use crate::{Curve, KernelError, Point, Spline};
use solidcore_ops::Body as EngineBody;
use solidcore_ops::Order;
use solidcore_record::brep::{Brep as Record, Curve as RecordCurve};

/// A Curva curve as the engine's record curve: always its exact NURBS form, so a piece of a
/// circle stays that piece (a record circle has no ends).
pub fn to_record_curve(curve: &Curve) -> Result<RecordCurve, KernelError> {
    if let Curve::Line { start, end } = curve {
        return Ok(RecordCurve::Line {
            start: *start,
            end: *end,
        });
    }
    to_record_spline(curve)
}

/// A Curva curve as the engine's record spline, lines included, for operations that take
/// splines only.
pub fn to_record_spline(curve: &Curve) -> Result<RecordCurve, KernelError> {
    let spline = Spline::of(curve)?;
    let rational = spline.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12);
    Ok(if rational {
        RecordCurve::RationalSpline {
            knots: spline.knots,
            homogeneous: spline.points,
        }
    } else {
        RecordCurve::Spline {
            knots: spline.knots,
            points: spline.points.iter().map(|p| [p[0], p[1], p[2]]).collect(),
        }
    })
}

/// The engine's record curve as a Curva curve, cut to `interval` (in the record curve's own
/// parameter) when one is given.
pub fn from_record_curve(
    curve: &RecordCurve,
    interval: Option<[f64; 2]>,
) -> Result<Curve, KernelError> {
    let bad = |why: &str| KernelError(format!("an engine curve Curva cannot hold: {why}"));
    let spline = match curve {
        RecordCurve::Line { start, end } => {
            let [a, b] = interval.unwrap_or([0.0, 1.0]);
            let at =
                |t: f64| -> Point { std::array::from_fn(|k| start[k] + t * (end[k] - start[k])) };
            return Curve::line(at(a), at(b));
        }
        RecordCurve::Polyline { points } => {
            if interval.is_some() {
                Spline::polyline(points)?
            } else {
                let closed =
                    points.len() > 2 && crate::gap(points[0], points[points.len() - 1]) < 1e-12;
                let mut points = points.clone();
                if closed {
                    points.pop();
                }
                return Curve::polyline(points, closed);
            }
        }
        RecordCurve::Circle { frame, radius } => {
            let [a, b] = interval.unwrap_or([0.0, std::f64::consts::TAU]);
            let whole = Curve::circle_on(frame.origin, *radius, frame.x_axis, frame.y_axis)?;
            if (b - a - std::f64::consts::TAU).abs() < 1e-12 && a.abs() < 1e-12 {
                return Ok(whole);
            }
            let at = |t: f64| -> Point {
                std::array::from_fn(|k| {
                    frame.origin[k]
                        + radius * (t.cos() * frame.x_axis[k] + t.sin() * frame.y_axis[k])
                })
            };
            // The arc from a to b about the frame's normal, as Curva's arc takes its plane.
            let x = std::array::from_fn(|k| (at(a)[k] - frame.origin[k]) / radius);
            let y = crate::cross(crate::cross(frame.x_axis, frame.y_axis), x);
            return Curve::arc(frame.origin, at(a), at(b), x, y);
        }
        RecordCurve::Ellipse { frame, radii } => {
            let whole = Curve::ellipse(
                frame.origin,
                frame.x_axis.map(|v| v * radii[0]),
                frame.y_axis.map(|v| v * radii[1]),
            )?;
            if interval.is_none() {
                return Ok(whole);
            }
            Spline::of(&whole)?
        }
        RecordCurve::Spline { knots, points } => {
            let degree = knots.len().saturating_sub(points.len() + 1);
            Spline {
                degree,
                knots: knots.clone(),
                points: points.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
            }
        }
        RecordCurve::RationalSpline { knots, homogeneous } => {
            let degree = knots.len().saturating_sub(homogeneous.len() + 1);
            Spline {
                degree,
                knots: knots.clone(),
                points: homogeneous.clone(),
            }
        }
        RecordCurve::SurfaceIntersection { leader, .. } => {
            return from_record_curve(leader, interval);
        }
    };
    let mut spline = spline.clamped().map_err(|_| bad("unclamped knots"))?;
    if let Some([a, b]) = interval {
        let (lo, hi) = spline.domain();
        if b > a + 1e-12 && (a > lo + 1e-12 || b < hi - 1e-12) {
            if a > lo + 1e-12
                && let Some((_, right)) = spline.split(a)
            {
                spline = right;
            }
            let (_, hi) = spline.domain();
            if b < hi - 1e-12
                && let Some((left, _)) = spline.split(b)
            {
                spline = left;
            }
        }
    }
    spline.to_curve()
}

impl Body {
    /// A body from a record another program made, such as a Rhino file's B-rep turned into the
    /// engine's record: the engine's healer runs on it, as its rules may not all be kept.
    pub fn from_foreign_record(record: &Record) -> Result<Self, KernelError> {
        let canonical = record
            .canonical()
            .map_err(|e| KernelError(format!("a B-rep Curva cannot read: {e}")))?;
        // The healer first; where it has to change the topology, the record as it is.
        let body = match EngineBody::from_record(&canonical.brep, false) {
            Ok((body, _)) => body,
            Err(_) => {
                EngineBody::from_record(&canonical.brep, true)
                    .map_err(|e| failed("reading a B-rep", e))?
                    .0
            }
        };
        Self::made(&body)
    }

    /// The body as the engine's portable record, for writing it in another format.
    pub fn portable_record(&self) -> Result<Record, KernelError> {
        self.record()
    }
}

impl Body {
    /// A body from the engine's record, as engine operations give them.
    pub fn from_record(record: &Record) -> Result<Self, KernelError> {
        let (body, _) =
            EngineBody::from_record(record, true).map_err(|e| failed("reading a body", e))?;
        Self::made(&body)
    }

    /// The engine's body in place and the order that names its faces, edges and points as
    /// Curva numbers them.
    pub(crate) fn engine_with_order(&self) -> Result<(EngineBody, Order), KernelError> {
        let body = self.engine_body()?;
        let (_, order) = body.to_record().map_err(|e| failed("reading a body", e))?;
        Ok((body, order))
    }
}

#[cfg(test)]
mod tests {
    use super::{from_record_curve, to_record_curve};
    use crate::{Curve, Spline, gap};

    #[test]
    fn curves_come_back_as_they_went() {
        let curves = [
            Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap(),
            Curve::circle([1.0, 2.0, 0.0], 5.0).unwrap(),
            Curve::arc(
                [0.0; 3],
                [5.0, 0.0, 0.0],
                [0.0, 5.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .unwrap(),
            Curve::control_point(
                vec![[0.0; 3], [1.0, 2.0, 0.0], [3.0, -1.0, 1.0], [5.0, 0.0, 0.0]],
                3,
                false,
            )
            .unwrap(),
        ];
        for curve in curves {
            let back = from_record_curve(&to_record_curve(&curve).unwrap(), None).unwrap();
            let (a, b) = (Spline::of(&curve).unwrap(), Spline::of(&back).unwrap());
            for i in 0..=20 {
                let t = f64::from(i) / 20.0;
                let (d0, d1) = (a.domain(), b.domain());
                let pa = a.point(d0.0 + (d0.1 - d0.0) * t);
                let pb = b.point(d1.0 + (d1.1 - d1.0) * t);
                assert!(gap(pa, pb) < 1e-9, "{curve:?}");
            }
        }
    }
}
