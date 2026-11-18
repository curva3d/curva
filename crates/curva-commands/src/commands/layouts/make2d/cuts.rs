//! Cutting Make2D's inputs by the chosen clipping planes: solids split by the engine,
//! curves cut where they cross, and sections as the hidden-line drawing takes them.

use super::cross;
use curva_document::clipping::PlaneCut;
use curva_kernel::{Body, Curve, KernelError, Point};

/// How far a point lies on the side a cut keeps (below zero: cut away).
pub(super) fn ahead(cut: &PlaneCut, p: Point) -> f64 {
    (0..3).map(|i| cut.keep[i] * (p[i] - cut.origin[i])).sum()
}

/// A body with what a clipping plane cuts away taken off: the part on the kept side, the whole
/// body when the plane does not divide it and it lies on that side, or nothing when it lies
/// wholly cut away. Cut by a square sheet on the plane, well beyond the body, through the
/// engine's split.
pub(super) fn clipped_body(body: &Body, cut: &PlaneCut) -> Result<Option<Body>, KernelError> {
    // Wholly on one side: kept whole or cut away, as the engine's split would leave it (#317).
    match body.side_of_plane(cut.origin, cut.keep)? {
        Some(true) => return Ok(Some(body.clone())),
        Some(false) => return Ok(None),
        None => {}
    }
    let (low, high) = body.bounds()?;
    let centre: Point = std::array::from_fn(|i| (low[i] + high[i]) / 2.0);
    let size = (0..3)
        .map(|i| (high[i] - low[i]).powi(2))
        .sum::<f64>()
        .sqrt()
        .max(1.0);
    let n = cut.keep;
    let other = if n[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = {
        let c = cross(n, other);
        let l = (0..3).map(|i| c[i] * c[i]).sum::<f64>().sqrt();
        c.map(|v| v / l)
    };
    let v = cross(n, u);
    let off = ahead(cut, centre);
    let middle: Point = std::array::from_fn(|i| centre[i] - n[i] * off);
    let corner = |a: f64, b: f64| -> Point {
        std::array::from_fn(|i| middle[i] + (u[i] * a + v[i] * b) * 2.0 * size)
    };
    let square = Curve::polyline(
        vec![
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ],
        true,
    )?;
    let sheet = Body::planar(&[square])?;
    let middle_of = |b: &Body| -> Result<f64, KernelError> {
        let (lo, hi) = b.bounds()?;
        Ok(ahead(cut, std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)))
    };
    Ok(match body.split_by(&sheet)? {
        Some((behind, front)) => Some(if middle_of(&behind)? > middle_of(&front)? {
            behind
        } else {
            front
        }),
        None if off >= 0.0 => Some(body.clone()),
        None => None,
    })
}

/// A section as the hidden-line drawing takes it: a straight-sided one (the engine gives a
/// planar cut through flat faces as one degree-1 curve) as its lines, others as they are.
pub(super) fn straight_pieces(curve: &Curve) -> Vec<Curve> {
    match curve {
        Curve::Nurbs {
            degree: 1,
            points,
            weights,
            ..
        } if weights.is_empty() => points
            .windows(2)
            .filter_map(|pair| Curve::line(pair[0], pair[1]).ok())
            .collect(),
        other => other.exploded().unwrap_or_else(|| vec![other.clone()]),
    }
}

/// The pieces of a curve on the side every cut keeps (but `skip`, the plane a section lies
/// on): cut where its samples cross a plane, the crossings found along the samples and put on
/// the curve, and the pieces whose middles are cut away dropped.
pub(super) fn clipped_curve(curve: &Curve, cuts: &[PlaneCut], skip: Option<u64>) -> Vec<Curve> {
    let mut pieces = vec![curve.clone()];
    for cut in cuts.iter().filter(|c| Some(c.frame) != skip) {
        let mut kept = Vec::new();
        for piece in pieces {
            let Ok(points) = piece.sample() else {
                continue;
            };
            let sides: Vec<f64> = points.iter().map(|p| ahead(cut, *p)).collect();
            let tolerance = 1e-9;
            if sides.iter().all(|d| *d >= -tolerance) {
                kept.push(piece);
                continue;
            }
            if sides.iter().all(|d| *d <= tolerance) {
                continue;
            }
            let crossings: Vec<Point> = points
                .windows(2)
                .zip(sides.windows(2))
                .filter(|(_, d)| (d[0] < 0.0) != (d[1] < 0.0) && (d[0] - d[1]).abs() > 0.0)
                .map(|(p, d)| {
                    let t = d[0] / (d[0] - d[1]);
                    let at: Point = std::array::from_fn(|i| p[0][i] + (p[1][i] - p[0][i]) * t);
                    piece.closest_point(at).unwrap_or(at)
                })
                .collect();
            let parts = piece
                .cut_at(&crossings)
                .unwrap_or_else(|_| vec![piece.clone()]);
            for part in parts {
                let Ok(samples) = part.sample() else {
                    continue;
                };
                if ahead(cut, samples[samples.len() / 2]) >= 0.0 {
                    kept.push(part);
                }
            }
        }
        pieces = kept;
    }
    pieces
}
