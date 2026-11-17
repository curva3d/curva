//! Curves to and from the geometry engine, and splines through points.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn engine_error(error: impl fmt::Debug) -> KernelError {
    KernelError(format!("{error:?}"))
}
pub(crate) fn spline(points: &[Point], closed: bool) -> Result<CoreCurve, KernelError> {
    spline_data(points, closed).map(CoreCurve::from)
}

pub(crate) fn spline_data(points: &[Point], closed: bool) -> Result<BSplineCurve, KernelError> {
    let mut through: Vec<Point3> = points
        .iter()
        .map(|p| Point3::new(p[0], p[1], p[2]))
        .collect();
    // The engine closes a curve onto a repeated first point; Curva stores each point once.
    if closed && let Some(first) = through.first().copied() {
        through.push(first);
    }
    let degree = 3.min(through.len().saturating_sub(1));
    BSplineCurve::interpolate_points(
        &through,
        None,
        degree,
        None,
        None,
        closed,
        ParamMethod::ChordLength,
    )
    .map_err(engine_error)
}

/// Builds the engine's curve from NURBS data. Rational control points go to it weighted.
pub(crate) fn nurbs_data(
    degree: usize,
    knots: &[f64],
    points: &[Point],
    weights: &[f64],
) -> Result<BSplineCurve, KernelError> {
    let control: Vec<CPoint> = points
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let w = weights.get(index).copied();
            let scale = w.unwrap_or(1.0);
            CPoint {
                x: p[0] * scale,
                y: p[1] * scale,
                z: p[2] * scale,
                w: w.unwrap_or(NOW),
            }
        })
        .collect();
    BSplineCurve::from_data(
        3,
        points.len().saturating_sub(1),
        knots.len().saturating_sub(1),
        degree,
        &control,
        knots,
    )
    .map_err(engine_error)
}

/// The engine curve as Curva NURBS data.
pub(crate) fn curve_from_engine(engine: &BSplineCurve) -> Result<Curve, KernelError> {
    let degree = engine.degree().map_err(engine_error)?;
    let (values, counts) = engine.knots(None).map_err(engine_error)?;
    let knots = values
        .iter()
        .zip(&counts)
        .flat_map(|(value, count)| std::iter::repeat_n(*value, *count))
        .collect();
    let (polygon, weights) = engine.control_polygon().map_err(engine_error)?;
    let points = polygon.iter().map(|p| [p.x, p.y, p.z]).collect();
    Curve::nurbs(degree, knots, points, weights)
}
pub(crate) fn nurbs_engine_curve(
    degree: usize,
    knots: &[f64],
    points: &[Point],
    weights: &[f64],
) -> Result<CoreCurve, KernelError> {
    nurbs_data(degree, knots, points, weights).map(CoreCurve::from)
}

/// Evenly spaced points along an engine curve, by its parameter.
pub(crate) fn sampled(curve: &CoreCurve, steps: usize) -> Result<Vec<Point>, KernelError> {
    let kernel = Kernel::default();
    let interval = curve.natural_interval();
    let mut out = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let t = interval.min() + interval.length() * step as f64 / steps as f64;
        let at = curve.evaluate_point(&kernel, t).map_err(engine_error)?;
        out.push([at.x, at.y, at.z]);
    }
    Ok(out)
}
