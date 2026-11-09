//! Runs the evaluation cases on solidcore and writes the results as tab-separated text.
//!
//! Usage: `solidcore-runner <cases folder> <output file>`
//!
//! Cases not connected yet are listed as `not-wired`, so they cannot be mistaken for solidcore
//! refusing them. Where a case has an exact answer, it is recorded beside the measured value.

use kernel_eval::{Attempt, Case, Kernel, Outcome, Record, load_cases, report, run_case};
use solidcore_ops::boolean::boolean;
use solidcore_ops::sweep::{Curve, SweepFrame, loft, revolve, sweep_placed};
use solidcore_ops::{
    Blend, Body, BooleanOp, CheckLevel, Corners, Turn, blend, blend_ordered, make_box,
    make_cylinder, shell,
};
use std::f64::consts::{FRAC_1_SQRT_2, PI};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// The cases this runner can perform so far.
const WIRED: [u32; 12] = [4, 5, 6, 8, 13, 14, 15, 16, 18, 19, 20, 22];

/// Accuracy asked of solidcore's volume and area integrals.
const ACCURACY: f64 = 1e-6;

/// How close, relatively, a position must be to count as on a given height.
const HEIGHT_TOLERANCE: f64 = 1e-6;

struct Solidcore;

impl Kernel for Solidcore {
    fn name(&self) -> &str {
        "solidcore"
    }

    fn run(&self, case: &Case) -> Result<Attempt, String> {
        let body = perform(case)?;
        let mut attempt = measure(case, &body)?;
        attempt.measurements.extend(extra(case));
        Ok(attempt)
    }
}

type V3 = [f64; 3];

fn add(a: V3, b: V3, scale: f64) -> V3 {
    [
        a[0] + b[0] * scale,
        a[1] + b[1] * scale,
        a[2] + b[2] * scale,
    ]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: V3) -> V3 {
    add([0.0; 3], a, 1.0 / dot(a, a).sqrt())
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// B-spline basis function by the Cox-de Boor recursion (*The NURBS Book*, eq. 2.5, p. 50).
fn basis(i: usize, degree: usize, knots: &[f64], t: f64) -> f64 {
    if degree == 0 {
        let last = knots[knots.len() - 1];
        let inside = knots[i] <= t && t < knots[i + 1];
        let at_end = t == last && knots[i] < last && knots[i + 1] == last;
        return f64::from(u8::from(inside || at_end));
    }
    let ratio = |top: f64, bottom: f64| if bottom == 0.0 { 0.0 } else { top / bottom };
    ratio(t - knots[i], knots[i + degree] - knots[i]) * basis(i, degree - 1, knots, t)
        + ratio(
            knots[i + degree + 1] - t,
            knots[i + degree + 1] - knots[i + 1],
        ) * basis(i + 1, degree - 1, knots, t)
}

/// Solves a square system with one right-hand side per coordinate, with partial pivoting.
fn solve(mut matrix: Vec<Vec<f64>>, mut values: Vec<V3>) -> Result<Vec<V3>, String> {
    let n = matrix.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))
            .ok_or("harness: empty system")?;
        if matrix[pivot][column].abs() < 1e-12 {
            return Err("harness: interpolation system is singular".to_owned());
        }
        matrix.swap(column, pivot);
        values.swap(column, pivot);
        for row in column + 1..n {
            let factor = matrix[row][column] / matrix[column][column];
            let pivot_row = matrix[column].clone();
            for (cell, value) in matrix[row].iter_mut().zip(&pivot_row).skip(column) {
                *cell -= factor * value;
            }
            values[row] = add(values[row], values[column], -factor);
        }
    }
    for column in (0..n).rev() {
        let mut value = values[column];
        for k in column + 1..n {
            value = add(value, values[k], -matrix[column][k]);
        }
        values[column] = add([0.0; 3], value, 1.0 / matrix[column][column]);
    }
    Ok(values)
}

/// Curve through the points: chord-length parameters and averaged knots, degree 3 or as high as
/// the points allow, as Rhino's InterpCrv (*The NURBS Book*, section 9.2.1, eqs. 9.5 and 9.8).
fn interpolate(points: &[V3]) -> Result<Curve, String> {
    let n = points.len();
    if n < 2 {
        return Err("harness: too few points".to_owned());
    }
    let degree = 3.min(n - 1);
    let mut params = vec![0.0; n];
    for i in 1..n {
        let step = add(points[i], points[i - 1], -1.0);
        params[i] = params[i - 1] + dot(step, step).sqrt();
    }
    let total = params[n - 1];
    params.iter_mut().for_each(|value| *value /= total);
    let mut knots = vec![0.0; degree + 1];
    for j in 1..n - degree {
        knots.push(params[j..j + degree].iter().sum::<f64>() / degree as f64);
    }
    knots.extend(vec![1.0; degree + 1]);
    let matrix = params
        .iter()
        .map(|&t| (0..n).map(|i| basis(i, degree, &knots, t)).collect())
        .collect();
    let solved = solve(matrix, points.to_vec())?;
    Ok(Curve::Nurbs {
        degree,
        points: solved.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
        knots,
    })
}

/// One quadratic span in a section's plane: control point, its weight and end point.
type Span = ([f64; 2], f64, [f64; 2]);

/// A closed planar section as one rational quadratic curve, so all sections have the same form.
fn planar_loop(centre: V3, normal: V3, start: [f64; 2], spans: &[Span]) -> Curve {
    let n = unit(normal);
    let seed = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = unit(add(seed, n, -dot(seed, n)));
    let v = cross(n, u);
    let place = |p: [f64; 2], weight: f64| {
        let at = add(add(centre, u, p[0]), v, p[1]);
        [at[0], at[1], at[2], weight]
    };
    let mut points = vec![place(start, 1.0)];
    let mut knots = vec![0.0; 3];
    for (index, (control, weight, end)) in spans.iter().enumerate() {
        points.push(place(*control, *weight));
        points.push(place(*end, 1.0));
        let knot = (index + 1) as f64;
        knots.extend([knot, knot]);
    }
    knots.push(spans.len() as f64);
    Curve::Nurbs {
        degree: 2,
        points,
        knots,
    }
}

/// A line as a quadratic span, to sit beside the arcs in one curve.
fn straight(from: [f64; 2], to: [f64; 2]) -> Span {
    ([(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0], 1.0, to)
}

fn section(text: &str) -> Result<Curve, String> {
    let number = |key: &str| -> Result<f64, String> { field(text, key)?.parse().map_err(fail) };
    let centre = point(field(text, "centre")?)?;
    let normal = field(text, "normal").map_or(Ok([0.0, 0.0, 1.0]), point)?;
    let ellipse = |rx: f64, ry: f64| {
        let spans = [
            ([rx, ry], FRAC_1_SQRT_2, [0.0, ry]),
            ([-rx, ry], FRAC_1_SQRT_2, [-rx, 0.0]),
            ([-rx, -ry], FRAC_1_SQRT_2, [0.0, -ry]),
            ([rx, -ry], FRAC_1_SQRT_2, [rx, 0.0]),
        ];
        planar_loop(centre, normal, [rx, 0.0], &spans)
    };
    match text.split_whitespace().nth(1) {
        Some("circle") => Ok(ellipse(number("radius")?, number("radius")?)),
        Some("ellipse") => Ok(ellipse(number("rx")?, number("ry")?)),
        Some("rounded-rectangle") => {
            let (w, d, r) = (
                number("width")? / 2.0,
                number("depth")? / 2.0,
                number("radius")?,
            );
            let k = FRAC_1_SQRT_2;
            let spans = [
                straight([w, 0.0], [w, d - r]),
                ([w, d], k, [w - r, d]),
                straight([w - r, d], [r - w, d]),
                ([-w, d], k, [-w, d - r]),
                straight([-w, d - r], [-w, r - d]),
                ([-w, -d], k, [r - w, -d]),
                straight([r - w, -d], [w - r, -d]),
                ([w, -d], k, [w, r - d]),
                straight([w, r - d], [w, 0.0]),
            ];
            Ok(planar_loop(centre, normal, [w, 0.0], &spans))
        }
        other => Err(format!("harness: section kind {other:?} not connected")),
    }
}

/// The curve of a named input.
fn curve(case: &Case, label: &str) -> Result<Curve, String> {
    let text = case
        .inputs
        .iter()
        .find(|entry| entry.split_whitespace().next() == Some(label))
        .ok_or_else(|| format!("harness: no input `{label}`"))?;
    if text.split_whitespace().nth(1) == Some("interp-curve") {
        let points: Vec<V3> = text
            .split_whitespace()
            .skip(2)
            .map(point)
            .collect::<Result<_, _>>()?;
        interpolate(&points)
    } else {
        section(text)
    }
}

/// Lofts the named inputs, one curve per section.
fn loft_of(case: &Case, labels: &[&str], cap: bool) -> Result<Body, String> {
    let sections = labels
        .iter()
        .map(|label| curve(case, label).map(|made| vec![made]))
        .collect::<Result<Vec<_>, _>>()?;
    loft(&sections, 3, cap).map_err(fail)
}

fn lofted_a(case: &Case) -> Result<Body, String> {
    if case.id == 15 {
        loft_of(case, &["s1", "s2", "s3"], true)
    } else {
        loft_of(case, &["a-s1", "a-s2", "a-s3"], true)
    }
}

fn second_solid(case: &Case) -> Result<Body, String> {
    if case.id == 15 {
        input(case, "cutter")
    } else {
        loft_of(case, &["b-s1", "b-s2", "b-s3"], true)
    }
}

fn revolved(case: &Case, degrees: f64) -> Result<Body, String> {
    revolve(
        &[curve(case, "profile")?],
        [0.0; 3],
        [0.0, 0.0, 1.0],
        degrees.to_radians(),
        false,
    )
    .map_err(fail)
}

fn volume_of(a: &Body, b: &Body, op: BooleanOp) -> Result<f64, String> {
    let made = boolean(a.copy().map_err(fail)?, b.copy().map_err(fail)?, op).map_err(fail)?;
    match made {
        Some(body) => body
            .mass_properties(ACCURACY)
            .map_err(fail)
            .map(|m| m.volume),
        None => Ok(0.0),
    }
}

/// How far the solids' volumes are from adding up, as a percentage. Holds for any correct boolean,
/// so it is a check that needs no reference answer.
fn volume_balance(case: &Case) -> Result<f64, String> {
    let (a, b) = (lofted_a(case)?, second_solid(case)?);
    let volume = |body: &Body| {
        body.mass_properties(ACCURACY)
            .map_err(fail)
            .map(|m| m.volume)
    };
    let inside = volume_of(&a, &b, BooleanOp::Intersect)?;
    let (whole, sum) = if case.id == 15 {
        (
            volume(&a)?,
            volume_of(&a, &b, BooleanOp::Subtract)? + inside,
        )
    } else {
        (
            volume(&a)? + volume(&b)?,
            volume_of(&a, &b, BooleanOp::Union)? + inside,
        )
    };
    Ok((sum - whole).abs() / whole * 100.0)
}

/// Further measurements for the cases that have them; a failure is recorded, not fatal.
fn extra(case: &Case) -> Vec<(String, String)> {
    match case.id {
        8 => {
            let area = revolved(case, 270.0)
                .and_then(|body| body.area(ACCURACY).map_err(fail))
                .map_or_else(
                    |error| format!("failed: {error}"),
                    |area| format!("{area:.6}"),
                );
            vec![("area-at-270".to_owned(), area)]
        }
        15 | 16 => {
            let balance = volume_balance(case).map_or_else(
                |error| format!("failed: {error}"),
                |gap| format!("{gap:.6}"),
            );
            vec![("volume-balance-percent".to_owned(), balance)]
        }
        _ => Vec::new(),
    }
}

/// Finds `key=value` in a line of space-separated parameters.
fn field<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    text.split_whitespace()
        .find_map(|part| part.strip_prefix(key)?.strip_prefix('='))
        .ok_or_else(|| format!("harness: `{key}` missing in `{text}`"))
}

fn point(text: &str) -> Result<[f64; 3], String> {
    let inner = text.trim_matches(['(', ')']);
    let values: Vec<f64> = inner
        .split(',')
        .map(|value| value.parse::<f64>().map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    <[f64; 3]>::try_from(values).map_err(|_| format!("harness: not a point: `{text}`"))
}

fn option(case: &Case, key: &str) -> Result<f64, String> {
    case.options
        .iter()
        .find_map(|entry| entry.strip_prefix(key)?.strip_prefix('='))
        .ok_or_else(|| format!("harness: option `{key}` missing"))?
        .parse()
        .map_err(|_| format!("harness: option `{key}` is not a number"))
}

fn fail(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

/// Builds a named input. Cylinders are upright because solidcore's constructor is about +Z.
fn input(case: &Case, label: &str) -> Result<Body, String> {
    let text = case
        .inputs
        .iter()
        .find(|entry| entry.split_whitespace().next() == Some(label))
        .ok_or_else(|| format!("harness: no input `{label}`"))?;
    match text.split_whitespace().nth(1) {
        Some("box") => {
            let min = point(field(text, "min")?)?;
            let max = point(field(text, "max")?)?;
            make_box(min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]).map_err(fail)
        }
        Some("cylinder") => {
            let base = point(field(text, "base")?)?;
            let top = point(field(text, "top")?)?;
            let radius: f64 = field(text, "radius")?.parse().map_err(fail)?;
            make_cylinder(base, radius, top[2] - base[2]).map_err(fail)
        }
        other => Err(format!("harness: input kind {other:?} not connected")),
    }
}

fn combine(case: &Case, op: BooleanOp) -> Result<Body, String> {
    boolean(input(case, "a")?, input(case, "b")?, op)
        .map_err(fail)?
        .ok_or_else(|| "the result is empty".to_owned())
}

/// Rounds or bevels every edge of the case's input.
fn blend_all(case: &Case, label: &str, kind: Blend, size: f64) -> Result<Body, String> {
    let body = input(case, label)?;
    let edges: Vec<usize> = (0..body.counts().map_err(fail)?[1]).collect();
    blend(&body, kind, &edges, &[size], &[], Corners::Filled).map_err(fail)?;
    Ok(body)
}

fn perform(case: &Case) -> Result<Body, String> {
    match case.id {
        4 => loft_of(case, &["c1", "c2", "c3", "c4"], false),
        5 => loft_of(case, &["s1", "s2", "s3"], false),
        6 => {
            let (rail, profile) = (curve(case, "rail")?, curve(case, "profile")?);
            sweep_placed(&[profile], &[rail], SweepFrame::Follow, false)
                .map(|(body, _, _)| body)
                .map_err(fail)
        }
        8 => revolved(case, 360.0),
        15 => boolean(lofted_a(case)?, second_solid(case)?, BooleanOp::Subtract)
            .map_err(fail)?
            .ok_or_else(|| "the result is empty".to_owned()),
        16 => boolean(lofted_a(case)?, second_solid(case)?, BooleanOp::Intersect)
            .map_err(fail)?
            .ok_or_else(|| "the result is empty".to_owned()),
        13 => combine(case, BooleanOp::Subtract),
        14 => combine(case, BooleanOp::Union),
        18 => {
            let body = input(case, "box")?;
            let (_, order) = body.to_record().map_err(fail)?;
            let top: Vec<usize> = (0..order.faces.len())
                .filter(|&face| {
                    matches!(body.face_plane_normal(&order, face),
                        Ok(Some(normal)) if normal.value[2] > 0.9)
                })
                .collect();
            // Negative because the wall grows inward, keeping the box's outer size as Rhino does.
            shell(&body, &order, &top, -option(case, "thickness")?).map_err(fail)?;
            Ok(body)
        }
        19 => blend_all(case, "box", Blend::Round, option(case, "radius")?),
        20 => {
            let body = input(case, "cylinder")?;
            let (_, order) = body.to_record().map_err(fail)?;
            let top = point(field(&case.inputs[0], "top")?)?[2];
            let rim: Vec<usize> = (0..order.edges.len())
                .filter(|&edge| {
                    matches!(body.edge_middle(&order, edge),
                        Ok(middle) if (middle[2] - top).abs() < HEIGHT_TOLERANCE * top)
                })
                .collect();
            let radius = option(case, "radius")?;
            blend_ordered(
                &body,
                &order,
                Blend::Round,
                &rim,
                &[radius],
                &[],
                Corners::Filled,
                Turn::Mitre,
            )
            .map_err(fail)?;
            Ok(body)
        }
        22 => blend_all(case, "box", Blend::Chamfer, option(case, "distance")?),
        other => Err(format!("harness: case {other} not connected")),
    }
}

/// The exact volume, for cases simple enough to work out by hand.
fn exact_volume(id: u32) -> Option<f64> {
    let (side, radius) = (20.0_f64, 2.0_f64);
    let flat = side - 2.0 * radius;
    match id {
        // Box less the part of the cylinder inside it.
        13 => Some(side.powi(3) - PI * 25.0 * side),
        14 => Some(2000.0),
        // The cavity is 16 by 16 and 18 deep, the floor staying 2 thick.
        18 => Some(8000.0 - 16.0 * 16.0 * 18.0),
        // Core, six slabs, twelve quarter-cylinders and eight eighth-spheres.
        19 => Some(
            flat.powi(3)
                + 6.0 * flat.powi(2) * radius
                + 3.0 * PI * radius.powi(2) * flat
                + 4.0 / 3.0 * PI * radius.powi(3),
        ),
        // Pappus: the corner sliver removed, turned about the axis.
        20 => {
            let sliver = radius.powi(2) * (1.0 - PI / 4.0);
            let inset = radius * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI);
            Some(PI * 100.0 * side - 2.0 * PI * (10.0 - inset) * sliver)
        }
        _ => None,
    }
}

fn measure(case: &Case, body: &Body) -> Result<Attempt, String> {
    let mass = body.mass_properties(ACCURACY).map_err(fail)?;
    let [faces, edges, vertices] = body.counts().map_err(fail)?;
    let check = body.check(CheckLevel::All).map_err(fail)?;
    let centre = mass.centroid;
    let mut measurements = vec![
        ("volume".to_owned(), format!("{:.6}", mass.volume)),
        ("area".to_owned(), format!("{:.6}", mass.area)),
        (
            "centre".to_owned(),
            format!("({:.4},{:.4},{:.4})", centre[0], centre[1], centre[2]),
        ),
        (
            "topology".to_owned(),
            format!("{faces}F/{edges}E/{vertices}V"),
        ),
        (
            "closed".to_owned(),
            body.is_manifold_solid().map_err(fail)?.to_string(),
        ),
        (
            "solids".to_owned(),
            body.solid_count().map_err(fail)?.to_string(),
        ),
        ("valid".to_owned(), check.is_valid().to_string()),
        (
            "failed-checks".to_owned(),
            format!("{:?}", check.failed_tests()),
        ),
    ];
    if let Some(exact) = exact_volume(case.id) {
        let percent = (mass.volume - exact).abs() / exact * 100.0;
        measurements.push(("exact-volume".to_owned(), format!("{exact:.6}")));
        measurements.push(("difference-percent".to_owned(), format!("{percent:.6}")));
    }
    Ok(Attempt {
        measurements,
        image: None,
    })
}

fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let usage = "usage: solidcore-runner <cases folder> <output file>";
    let folder = arguments.next().ok_or(usage)?;
    let output = arguments.next().ok_or(usage)?;
    let cases = load_cases(Path::new(&folder)).map_err(|error| error.to_string())?;
    let kernel: Arc<dyn Kernel> = Arc::new(Solidcore);
    let limit = Duration::from_secs(120);
    let records: Vec<Record> = cases
        .iter()
        .filter(|case| WIRED.contains(&case.id))
        .map(|case| run_case(Arc::clone(&kernel), case.clone(), limit))
        .collect();
    let mut text = report("solidcore", &records);
    for case in cases.iter().filter(|case| !WIRED.contains(&case.id)) {
        // Writing to a String cannot fail.
        let _ = writeln!(text, "solidcore\t{}\tnot-wired\t\t\t\t", case.id);
    }
    std::fs::write(&output, &text).map_err(|error| error.to_string())?;
    print!("{text}");
    let bad = records
        .iter()
        .filter(|record| record.outcome != Outcome::Works)
        .count();
    println!("{} run, {bad} did not produce a result", records.len());
    Ok(())
}
