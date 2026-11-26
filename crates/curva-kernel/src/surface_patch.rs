//! A patch over a boundary of any number of sides, as Rhino's Patch makes it (#684): one smooth
//! surface fitted over the boundary's best-fit plane, then trimmed back to the boundary. The
//! surface's heights off that plane are fitted to points along the boundary by least squares,
//! with a stiffness that keeps the net smooth where no boundary passes (Rhino's `SpanCount` and
//! `Stiffness`).

use crate::body::Body;
use crate::rebuild::solve;
use crate::surface::NurbsSurface;
use crate::{Curve, KernelError, Point, cross, dot, sub, unit};

/// Points taken along each boundary curve for the fit.
const SAMPLES_PER_CURVE: usize = 48;

/// How far the fitted surface reaches past the boundary, as a share of its size.
const MARGIN: f64 = 0.05;

/// The boundary's best-fit plane: its centre, and its X, Y and normal directions.
struct Frame {
    centre: Point,
    x: Point,
    y: Point,
    normal: Point,
}

impl Frame {
    /// The plane through `points` (a closed run), its normal by Newell's method.
    fn of(points: &[Point]) -> Option<Self> {
        let count = points.len() as f64;
        let centre: Point =
            std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / count);
        let mut n = [0.0; 3];
        for (i, p) in points.iter().enumerate() {
            let q = points[(i + 1) % points.len()];
            n[0] += (p[1] - q[1]) * (p[2] + q[2]);
            n[1] += (p[2] - q[2]) * (p[0] + q[0]);
            n[2] += (p[0] - q[0]) * (p[1] + q[1]);
        }
        let normal = unit(n)?;
        let x = unit(cross(normal, [1.0, 0.0, 0.0]))
            .or_else(|| unit(cross(normal, [0.0, 1.0, 0.0])))?;
        let y = cross(normal, x);
        Some(Self {
            centre,
            x,
            y,
            normal,
        })
    }

    /// `p` as (across, along, height) in the plane.
    fn local(&self, p: Point) -> [f64; 3] {
        let d = sub(p, self.centre);
        [dot(d, self.x), dot(d, self.y), dot(d, self.normal)]
    }

    fn point(&self, a: f64, b: f64, h: f64) -> Point {
        std::array::from_fn(|k| self.centre[k] + self.x[k] * a + self.y[k] * b + self.normal[k] * h)
    }
}

/// Uniform clamped cubic knots over 0..1 with `spans` spans.
fn knots(spans: usize) -> Vec<f64> {
    let mut k = vec![0.0; 4];
    k.extend((1..spans).map(|i| i as f64 / spans as f64));
    k.extend([1.0; 4]);
    k
}

/// Each control point's own place along a knot vector (its Greville abscissa).
fn greville(knots: &[f64], count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| knots[i + 1..=i + 3].iter().sum::<f64>() / 3.0)
        .collect()
}

impl Body {
    /// One smooth surface fitted over the closed boundary `curves` and trimmed to it, with
    /// `spans` spans each way and smoothing `stiffness` (Rhino's Patch, 10 and 1).
    pub fn fitted_patch(
        curves: &[Curve],
        spans: usize,
        stiffness: f64,
    ) -> Result<Body, KernelError> {
        let bad = |why: &str| KernelError(format!("patching: {why}"));
        let points: Vec<Point> = curves
            .iter()
            .map(|c| c.divided(SAMPLES_PER_CURVE))
            .collect::<Result<Vec<_>, _>>()?
            .concat();
        let frame = Frame::of(&points).ok_or_else(|| bad("the boundary has no plane"))?;
        let local: Vec<[f64; 3]> = points.iter().map(|p| frame.local(*p)).collect();
        let range = |k: usize| {
            let lo = local.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
            let hi = local.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max);
            let pad = (hi - lo) * MARGIN;
            (lo - pad, hi + pad)
        };
        let ((a0, a1), (b0, b1)) = (range(0), range(1));
        if a1 - a0 < 1e-9 || b1 - b0 < 1e-9 {
            return Err(bad("the boundary is a line"));
        }
        // A flat boundary is filled exactly, by the plane it lies in.
        let size = (a1 - a0).max(b1 - b0);
        if local.iter().all(|p| p[2].abs() < 1e-9 * size)
            && let Ok(flat) = Body::planar(curves)
        {
            return Ok(flat);
        }
        let spans = spans.max(1);
        let knots = knots(spans);
        let n = spans + 3;
        let heights = fitted_heights(&local, [a0, a1, b0, b1], &knots, n, stiffness)?;
        let g = greville(&knots, n);
        let net: Vec<Vec<Point>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        let a = a0 + g[i] * (a1 - a0);
                        let b = b0 + g[j] * (b1 - b0);
                        frame.point(a, b, heights[i * n + j])
                    })
                    .collect()
            })
            .collect();
        let sheet = NurbsSurface::new([3, 3], knots.clone(), knots, &net, None)?.to_body()?;
        trimmed_to(&sheet, curves)
    }
}

/// The control net's heights off the plane: as near the boundary's points as least squares
/// makes them, with `stiffness` against bends in the net. The box is the plane's `[a0, a1, b0, b1]`.
fn fitted_heights(
    local: &[[f64; 3]],
    [a0, a1, b0, b1]: [f64; 4],
    knots: &[f64],
    n: usize,
    stiffness: f64,
) -> Result<Vec<f64>, KernelError> {
    let basis = |t: f64| -> Vec<f64> {
        (0..n)
            .map(|i| crate::interp::basis(knots, i, 3, t.clamp(0.0, 1.0)))
            .collect()
    };
    // Least squares on the heights: the boundary's, and a stiffness on the net's bends.
    let m = n * n;
    let mut normal = vec![vec![0.0; m]; m];
    let mut right = vec![0.0; m];
    for p in local {
        let (bu, bv) = (
            basis((p[0] - a0) / (a1 - a0)),
            basis((p[1] - b0) / (b1 - b0)),
        );
        let row: Vec<(usize, f64)> = (0..n)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| (i * n + j, bu[i] * bv[j]))
            .filter(|(_, w)| *w != 0.0)
            .collect();
        for &(r, wr) in &row {
            right[r] += wr * p[2];
            for &(c, wc) in &row {
                normal[r][c] += wr * wc;
            }
        }
    }
    let weight = stiffness * local.len() as f64 / m as f64;
    let mut bend = |cells: [usize; 3]| {
        let coef = [1.0, -2.0, 1.0];
        for (x, cx) in cells.iter().zip(coef) {
            for (y, cy) in cells.iter().zip(coef) {
                normal[*x][*y] += weight * cx * cy;
            }
        }
    };
    for i in 0..n {
        for j in 0..n {
            if i + 2 < n {
                bend([i * n + j, (i + 1) * n + j, (i + 2) * n + j]);
            }
            if j + 2 < n {
                bend([i * n + j, i * n + j + 1, i * n + j + 2]);
            }
        }
    }
    solve(normal, right)
}
/// `sheet` cut along the boundary pulled onto it, keeping the piece inside.
fn trimmed_to(sheet: &Body, curves: &[Curve]) -> Result<Body, KernelError> {
    let pulled = curves
        .iter()
        .map(|c| sheet.pulled(c))
        .collect::<Result<Vec<_>, _>>()?;
    let cut = sheet.split_by_curves(0, &pulled)?;
    // Inside: the piece with the smallest box. The rest reaches out to the sheet's edges.
    let size = |b: &Body| {
        b.bounds()
            .map_or(f64::INFINITY, |(low, high)| crate::gap(low, high))
    };
    (0..cut.face_count()?)
        .map(|k| cut.faces_only(&[k]))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min_by(|p, q| size(p).total_cmp(&size(q)))
        .ok_or_else(|| KernelError("patching: the boundary cut nothing".to_owned()))
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    #[test]
    fn a_hexagon_with_a_corner_lifted_is_one_trimmed_surface() {
        // Rhino's Patch makes one surface over any boundary (#684).
        let corners: Vec<[f64; 3]> = (0..6)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 6.0;
                [
                    10.0 * a.cos(),
                    10.0 * a.sin(),
                    if k == 0 { 3.0 } else { 0.0 },
                ]
            })
            .collect();
        let boundary = Curve::polyline(corners, true).unwrap();
        let patch = Body::fitted_patch(&[boundary], 10, 1.0).unwrap();
        assert_eq!(patch.face_count().unwrap(), 1);
        // About the flat hexagon's area, 259.81, a little more for the lift.
        let area = patch.area().unwrap();
        assert!((259.0..275.0).contains(&area), "{area}");
        let (low, high) = patch.bounds().unwrap();
        assert!(
            low[0] > -10.5 && high[0] < 10.5 && high[2] < 3.5,
            "{low:?} {high:?}"
        );
    }

    #[test]
    fn a_flat_boundary_is_filled_exactly() {
        let corners: Vec<[f64; 3]> = (0..6)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 6.0;
                [10.0 * a.cos(), 10.0 * a.sin(), 0.0]
            })
            .collect();
        let patch =
            Body::fitted_patch(&[Curve::polyline(corners, true).unwrap()], 10, 1.0).unwrap();
        let area = patch.area().unwrap();
        // The hexagon's own area, 1.5 · √3 · 10².
        assert!((area - 150.0 * 3f64.sqrt()).abs() < 1e-6, "{area}");
    }
}
