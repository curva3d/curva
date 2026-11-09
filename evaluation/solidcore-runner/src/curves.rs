//! The curve cases, which call solidcore's core directly because its operations layer has no
//! curve commands.

use super::{option, point};
use kernel_eval::{Attempt, Case};
use solidcore::curve::composite_curve::{
    CompositeCurve, OffsetCornerType, OffsetDirection, OffsetParams,
};
use solidcore::curve::curve::Curve;
use solidcore::curve::curve::p8::CurveIntersectOptions;
use solidcore::geom::solution_array::{ObjectKind, SolutionObject};
use solidcore::nurbs::b_spline_curve::BSplineCurve;
use solidcore::nurbs::crv_fit::p1::ParamMethod;
use solidcore::query::api_queries::curve_closest_point;
use solidcore::{Kernel, Point3, Vec3};

/// Accuracy asked of curve lengths.
const LENGTH_ACCURACY: f64 = 1e-6;

/// Tolerance for where curves are taken to meet, in millimetres.
const MEET_TOLERANCE: f64 = 1e-3;

/// Points checked along each offset piece.
const SAMPLES: usize = 64;

/// The solver only needs some handle type to label its answers; these answers are read by number.
#[derive(Clone, Copy, PartialEq)]
struct Handle;

impl SolutionObject for Handle {
    fn kind(&self) -> ObjectKind {
        ObjectKind::Curve
    }
}

fn fail(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

fn points_of(text: &str) -> Result<Vec<Point3>, String> {
    text.split_whitespace()
        .skip(2)
        .map(|part| point(part).map(|p| Point3::new(p[0], p[1], p[2])))
        .collect()
}

fn spline(text: &str, wanted_degree: usize) -> Result<Curve, String> {
    let points = points_of(text)?;
    let degree = wanted_degree.min(points.len().saturating_sub(1));
    BSplineCurve::interpolate_points(
        &points,
        None,
        degree,
        None,
        None,
        false,
        ParamMethod::ChordLength,
    )
    .map(Curve::from)
    .map_err(fail)
}

fn named<'a>(case: &'a Case, label: &str) -> Result<&'a str, String> {
    case.inputs
        .iter()
        .find(|entry| entry.split_whitespace().next() == Some(label))
        .map(String::as_str)
        .ok_or_else(|| format!("harness: no input `{label}`"))
}

fn length(kernel: &Kernel, curve: &Curve) -> Result<f64, String> {
    curve
        .length(kernel, &curve.natural_interval(), LENGTH_ACCURACY)
        .map_err(fail)
}

fn distance(kernel: &Kernel, curve: &Curve, at: Point3) -> Result<f64, String> {
    curve_closest_point(kernel, curve, at)
        .map(|found| found.closest.distance)
        .map_err(fail)
}

fn entry(name: &str, value: String) -> (String, String) {
    (name.to_owned(), value)
}

fn interpolated(case: &Case) -> Result<Vec<(String, String)>, String> {
    let kernel = Kernel::default();
    let text = named(case, "points")?;
    let curve = spline(text, 3)?;
    let mut worst = 0.0_f64;
    for at in points_of(text)? {
        worst = worst.max(distance(&kernel, &curve, at)?);
    }
    Ok(vec![
        entry("length", format!("{:.6}", length(&kernel, &curve)?)),
        entry("furthest-input-point", format!("{worst:.9}")),
    ])
}

fn offset(case: &Case) -> Result<Vec<(String, String)>, String> {
    let kernel = Kernel::default();
    let source = spline(named(case, "curve")?, 3)?;
    let wanted = option(case, "distance")?;
    let composite =
        CompositeCurve::with_curves(3, vec![source.clone()], false, None, None).map_err(fail)?;
    let params = OffsetParams {
        xsect_tol_3d: MEET_TOLERANCE,
        approx_tol_3d: MEET_TOLERANCE / 4.0,
        plane_normal: Vec3::new(0.0, 0.0, 1.0),
        corner: OffsetCornerType::FilletCorner,
        trim_results: true,
        offset_distance: wanted,
        match_parameterization: false,
    };
    let pieces = composite
        .create_trimmed_offsets(&kernel, OffsetDirection::BothSides, &params)
        .map_err(fail)?;
    let (mut total, mut near, mut far) = (0.0, f64::MAX, 0.0_f64);
    for piece in &pieces {
        total += length(&kernel, &piece.curve)?;
        let interval = piece.curve.natural_interval();
        for step in 0..=SAMPLES {
            let t = interval.min() + interval.length() * step as f64 / SAMPLES as f64;
            let at = piece.curve.evaluate_point(&kernel, t).map_err(fail)?;
            let gap = distance(&kernel, &source, at)?;
            near = near.min(gap);
            far = far.max(gap);
        }
    }
    Ok(vec![
        entry("curve-count", pieces.len().to_string()),
        entry("length", format!("{total:.6}")),
        entry("nearest-to-source", format!("{near:.6}")),
        entry("furthest-from-source", format!("{far:.6}")),
        entry("asked-distance", format!("{wanted:.6}")),
    ])
}

fn intersection(case: &Case) -> Result<Vec<(String, String)>, String> {
    let kernel = Kernel::default();
    let a = spline(named(case, "a")?, 3)?;
    let b = spline(named(case, "b")?, 3)?;
    let found = a
        .global_curve_intersect::<Handle>(
            &kernel,
            &a.natural_interval(),
            &b,
            &b.natural_interval(),
            MEET_TOLERANCE,
            &CurveIntersectOptions::default(),
        )
        .map_err(fail)?;
    let mut places = Vec::new();
    let mut worst = 0.0_f64;
    for index in 0..found.len() {
        let solution = found.get_at(index).map_err(fail)?;
        // Parameter 0 belongs to the first curve; the answer names no objects to look it up on.
        let t = solution.start.param(0).map_err(fail)?;
        let at = a.evaluate_point(&kernel, t).map_err(fail)?;
        places.push(format!("({:.4},{:.4},{:.4})", at.x, at.y, at.z));
        worst = worst
            .max(distance(&kernel, &a, at)?)
            .max(distance(&kernel, &b, at)?);
    }
    Ok(vec![
        entry("point-count", found.len().to_string()),
        entry("point-positions", places.join(" ")),
        entry("furthest-from-either-curve", format!("{worst:.9}")),
    ])
}

/// Runs a curve case, or returns `None` for any other case.
pub fn run(case: &Case) -> Option<Result<Attempt, String>> {
    let done = match case.id {
        1 => interpolated(case),
        2 => offset(case),
        3 => intersection(case),
        _ => return None,
    };
    Some(done.map(|measurements| Attempt {
        measurements,
        image: None,
    }))
}
