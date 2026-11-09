//! The cases that start from a freeform surface given by control points, plus join and section.

use super::{basis, fail, option, point};
use crate::output;
use kernel_eval::{Attempt, Case};
use solidcore::Kernel;
use solidcore::surface::from_parameters::from_parameters;
use solidcore::topology::brep::sheet::sheet_on;
use solidcore_convert::brep::to_record;
use solidcore_ops::boolean::union_all;
use solidcore_ops::split::{Tool, split_faces};
use solidcore_ops::stitch::stitch;
use solidcore_ops::sweep::{Curve, DraftCorner, extrude, face_section, planar_faces};
use solidcore_ops::{Body, offset_sheet};
use solidcore_record::brep::Surface;
use solidcore_ssi::{Engine, intersect};

const BEZIER_KNOTS: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
const WHOLE: [[f64; 2]; 2] = [[0.0, 1.0], [0.0, 1.0]];
/// Spacing of the source-surface samples used to measure distance from it.
const SOURCE_SAMPLES: usize = 400;
/// Most offset-mesh vertices checked.
const CHECKED_VERTICES: usize = 400;
const SECTION_TOLERANCE: f64 = 1e-4;
/// Steps used to measure a spline section's length.
const LENGTH_STEPS: usize = 2000;

fn entry(name: &str, value: String) -> (String, String) {
    (name.to_owned(), value)
}

/// The 4 by 4 control net of the case's `surface` input, row by row.
fn net(case: &Case) -> Result<Vec<Vec<[f64; 3]>>, String> {
    let text = case
        .input_points
        .first()
        .ok_or("harness: no control points")?;
    let points: Vec<[f64; 3]> = text
        .split_whitespace()
        .skip(1)
        .map(point)
        .collect::<Result<_, _>>()?;
    if points.len() != 16 {
        return Err("harness: expected 16 control points".to_owned());
    }
    Ok(points.chunks(4).map(<[[f64; 3]]>::to_vec).collect())
}

fn spline(case: &Case) -> Result<Surface, String> {
    Ok(Surface::Spline {
        u_knots: BEZIER_KNOTS.to_vec(),
        v_knots: BEZIER_KNOTS.to_vec(),
        points: net(case)?,
    })
}

/// The case's control net as a one-face sheet.
pub fn sheet(case: &Case) -> Result<Body, String> {
    let mut kernel = Kernel::default();
    let surface = solidcore_convert::surface(&spline(case)?).map_err(fail)?;
    let (made, _) = from_parameters(&kernel, &surface, WHOLE).map_err(fail)?;
    let (mut brep, _) = sheet_on(&mut kernel, made, 1e-6).map_err(fail)?;
    let (record, _) = to_record(&mut kernel, &mut brep).map_err(fail)?;
    Body::from_record(&record, true)
        .map(|(body, _)| body)
        .map_err(fail)
}

pub fn offset(case: &Case) -> Result<Body, String> {
    let body = sheet(case)?;
    offset_sheet(&body, option(case, "distance")?).map_err(fail)?;
    Ok(body)
}

fn dense_source(case: &Case) -> Result<Vec<[f64; 3]>, String> {
    let net = net(case)?;
    let mut samples = Vec::with_capacity((SOURCE_SAMPLES + 1).pow(2));
    for iu in 0..=SOURCE_SAMPLES {
        let u = iu as f64 / SOURCE_SAMPLES as f64;
        let across: Vec<f64> = (0..4).map(|i| basis(i, 3, &BEZIER_KNOTS, u)).collect();
        for iv in 0..=SOURCE_SAMPLES {
            let v = iv as f64 / SOURCE_SAMPLES as f64;
            let mut at = [0.0; 3];
            for (row, weight_u) in net.iter().zip(&across) {
                for (j, control) in row.iter().enumerate() {
                    let weight = weight_u * basis(j, 3, &BEZIER_KNOTS, v);
                    for axis in 0..3 {
                        at[axis] += weight * control[axis];
                    }
                }
            }
            samples.push(at);
        }
    }
    Ok(samples)
}

/// How far the offset's mesh stays from the source surface, away from its boundary effects.
/// Approximate: measured against dense samples of the source, good to about 0.01 mm.
pub fn offset_gap(case: &Case, body: &Body) -> Result<Vec<(String, String)>, String> {
    let source = dense_source(case)?;
    let (mesh, _) = output::mesh_of(body)?;
    let step = (mesh.positions.len() / CHECKED_VERTICES).max(1);
    let (mut near, mut far) = (f64::MAX, 0.0_f64);
    for p in mesh.positions.iter().step_by(step) {
        let closest = source
            .iter()
            .map(|s| (s[0] - p.x).powi(2) + (s[1] - p.y).powi(2) + (s[2] - p.z).powi(2))
            .fold(f64::MAX, f64::min)
            .sqrt();
        near = near.min(closest);
        far = far.max(closest);
    }
    Ok(vec![
        entry("nearest-to-source-approx", format!("{near:.4}")),
        entry("furthest-from-source-approx", format!("{far:.4}")),
    ])
}

/// Splits the sheet with the cutter curve stretched along the projection direction.
pub fn split(case: &Case) -> Result<Body, String> {
    let body = sheet(case)?;
    let (_, order) = body.to_record().map_err(fail)?;
    let Curve::Nurbs { points, knots, .. } = super::curve(case, "cutter")? else {
        return Err("harness: the cutter should be a spline".to_owned());
    };
    let wall: Vec<Vec<[f64; 3]>> = points
        .iter()
        .map(|p| vec![[p[0], p[1], 40.0], [p[0], p[1], -40.0]])
        .collect();
    let tool = Tool {
        surface: Surface::Spline {
            u_knots: knots,
            v_knots: vec![0.0, 0.0, 1.0, 1.0],
            points: wall,
        },
        domain: WHOLE,
    };
    Ok(split_faces(&body, &order, &[0], &[tool])
        .map_err(fail)?
        .map_or(body, |(cut, _)| cut))
}

/// The uncut surface's area, for comparison with a split.
pub fn source_area(case: &Case) -> Result<f64, String> {
    sheet(case)?.area(1e-6).map_err(fail)
}

/// Extrudes each wall and sews the four into one body.
pub fn join(case: &Case) -> Result<Body, String> {
    let mut walls = Vec::new();
    for text in &case.inputs {
        let mut parts = text.split_whitespace().skip(2);
        let (start, end) = match (parts.next(), parts.next()) {
            (Some(a), Some(b)) => (point(a)?, point(b)?),
            _ => return Err("harness: wall needs two points".to_owned()),
        };
        let height: f64 = super::field(text, "height")?.parse().map_err(fail)?;
        let line = Curve::Line { start, end };
        walls.push(
            extrude(
                &[line],
                [0.0, 0.0, 1.0],
                height,
                0.0,
                DraftCorner::default(),
                false,
            )
            .map_err(fail)?,
        );
    }
    let height: f64 = super::field(&case.inputs[0], "height")?
        .parse()
        .map_err(fail)?;
    for level in [0.0, height] {
        let outline: Vec<Curve> = case
            .inputs
            .iter()
            .map(|text| {
                let mut parts = text.split_whitespace().skip(2);
                let a = point(parts.next().ok_or("harness: wall start")?)?;
                let b = point(parts.next().ok_or("harness: wall end")?)?;
                Ok(Curve::Line {
                    start: [a[0], a[1], level],
                    end: [b[0], b[1], level],
                })
            })
            .collect::<Result<_, String>>()?;
        walls.push(planar_faces(&outline).map_err(fail)?);
    }
    let merged = union_all(walls).map_err(fail)?;
    stitch(merged, 1e-6).map(|(body, _)| body).map_err(fail)
}

/// Intersects the surface with the cylinder's side.
pub fn intersection(case: &Case) -> Result<Attempt, String> {
    let kernel = Kernel::default();
    let text = case
        .inputs
        .iter()
        .find(|entry| entry.starts_with("cylinder "))
        .ok_or("harness: no cylinder")?;
    let (base, top) = (
        point(super::field(text, "base")?)?,
        point(super::field(text, "top")?)?,
    );
    let radius: f64 = super::field(text, "radius")?.parse().map_err(fail)?;
    let solid = solidcore_ops::make_cylinder(base, radius, top[2] - base[2]).map_err(fail)?;
    let (record, _) = solid.to_record().map_err(fail)?;
    let side = record
        .faces
        .iter()
        .find(|face| matches!(face.surface, Surface::Cylinder { .. }))
        .ok_or("harness: no cylindrical face")?;
    let cylinder = solidcore_convert::surface(&side.surface).map_err(fail)?;
    let (cylinder, cylinder_domain) =
        from_parameters(&kernel, &cylinder, under_a_turn(side.domain)).map_err(fail)?;
    let freeform = solidcore_convert::surface(&spline(case)?).map_err(fail)?;
    let (freeform, freeform_domain) = from_parameters(&kernel, &freeform, WHOLE).map_err(fail)?;
    let found = intersect(
        Engine::Port,
        &kernel,
        &freeform,
        &freeform_domain,
        &cylinder,
        &cylinder_domain,
        [true, true],
        None,
        None,
    )
    .map_err(fail)?;
    let mut total = 0.0;
    for made in &found.curves.curves_3d {
        total += made
            .length(&kernel, &made.natural_interval(), 1e-6)
            .map_err(fail)?;
    }
    Ok(Attempt {
        measurements: vec![
            entry("curve-count", found.curves.curves_3d.len().to_string()),
            entry("length", format!("{total:.6}")),
        ],
        image: None,
    })
}

fn curve_length(curve: &Curve, interval: [f64; 2]) -> f64 {
    match curve {
        Curve::Line { start, end } => (0..3)
            .map(|axis| (end[axis] - start[axis]).powi(2))
            .sum::<f64>()
            .sqrt(),
        Curve::Arc {
            radius, start, end, ..
        } => radius * (end - start),
        Curve::Nurbs {
            degree,
            points,
            knots,
        } => {
            let at = |t: f64| {
                let (mut sum, mut total) = ([0.0; 3], 0.0);
                for (i, p) in points.iter().enumerate() {
                    let weight = basis(i, *degree, knots, t) * p[3];
                    total += weight;
                    for axis in 0..3 {
                        sum[axis] += weight * p[axis];
                    }
                }
                sum.map(|value| value / total)
            };
            let mut length = 0.0;
            let mut last = at(interval[0]);
            for step in 1..=LENGTH_STEPS {
                let next =
                    at(interval[0]
                        + (interval[1] - interval[0]) * step as f64 / LENGTH_STEPS as f64);
                length += (0..3)
                    .map(|axis| (next[axis] - last[axis]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                last = next;
            }
            length
        }
    }
}

/// Curves and total length of the body's section at a height.
fn section_at(body: &Body, height: f64) -> Result<(usize, f64), String> {
    let (_, order) = body.to_record().map_err(fail)?;
    let (mut count, mut total) = (0, 0.0);
    for face in 0..order.faces.len() {
        // A face the plane misses is an expected answer here, not a failure.
        if let Ok(pieces) = face_section(
            body,
            &order,
            face,
            [0.0, 0.0, height],
            [0.0, 0.0, 1.0],
            SECTION_TOLERANCE,
        ) {
            for (curve, interval) in &pieces {
                count += 1;
                total += curve_length(curve, *interval);
            }
        }
    }
    Ok((count, total))
}

/// The section at the case's plane and contours every `spacing` up the body.
pub fn section_measures(case: &Case, body: &Body) -> Result<Vec<(String, String)>, String> {
    let (count, length) = section_at(body, 15.0)?;
    let spacing = option(case, "contour-spacing")?;
    let (low, high) = body.bounds().map_err(fail)?;
    let (mut levels, mut curves) = (0, 0);
    let mut height = low[2] + spacing;
    while height < high[2] {
        levels += 1;
        curves += section_at(body, height)?.0;
        height += spacing;
    }
    Ok(vec![
        entry("section-curve-count", count.to_string()),
        entry("section-length", format!("{length:.6}")),
        entry("contour-levels", levels.to_string()),
        entry("contour-curve-count", curves.to_string()),
    ])
}

/// Keeps a full cylinder's sweep just under one turn, which is the most the constructor takes.
fn under_a_turn(domain: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let [u, v] = domain;
    let most = std::f64::consts::TAU * (1.0 - 1e-12);
    [[u[0], u[1].min(u[0] + most)], v]
}
