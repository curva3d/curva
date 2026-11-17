//! Points and curves on surfaces: the nearest point on a face, isocurves, pulling and drawing
//! curves on surfaces, offsetting along them, and silhouettes. Flagged as custom
//! (`docs/kernel/custom-operations.md`) unless the engine does the work.

use crate::body::{Body, failed};
use crate::surface::NurbsSurface;
use crate::surface_edit::plus;
use crate::{Curve, KernelError, Point, Spline, cross, dot, gap, sub, unit};
use solidcore_ops::face_surface::FaceSurface;

impl Body {
    /// The point of face `face` nearest `p`, with its (u, v): a coarse search for a start, then
    /// the engine's exact nearest point.
    fn nearest_on_face(&self, face: usize, p: Point) -> Result<([f64; 2], Point), KernelError> {
        let record = self.record()?;
        let surface = FaceSurface::of(&record, face).map_err(|e| failed("reading a face", e))?;
        let [[u0, u1], [v0, v1]] = surface.domain();
        let mut best = ([u0, v0], f64::INFINITY);
        for i in 0..=12 {
            for j in 0..=12 {
                let uv = [
                    u0 + (u1 - u0) * f64::from(i) / 12.0,
                    v0 + (v1 - v0) * f64::from(j) / 12.0,
                ];
                if let Ok(q) = surface.point(uv)
                    && gap(q, p) < best.1
                {
                    best = (uv, gap(q, p));
                }
            }
        }
        let found = surface
            .closest_point(p, best.0)
            .map_err(|e| failed("finding the nearest point", e))?;
        Ok((found.uv, found.point))
    }

    /// The face nearest a point, with the (u, v) and the point on it.
    pub fn nearest_face(&self, p: Point) -> Result<(usize, [f64; 2], Point), KernelError> {
        let count = self.face_count()?;
        let mut best: Option<(usize, [f64; 2], Point)> = None;
        for face in 0..count {
            if let Ok((uv, q)) = self.nearest_on_face(face, p)
                && best.as_ref().is_none_or(|(_, _, b)| gap(q, p) < gap(*b, p))
            {
                best = Some((face, uv, q));
            }
        }
        best.ok_or_else(|| KernelError("no face to measure from".to_owned()))
    }

    /// The isocurve of face `face` through its point nearest `p`: along u, or along v when
    /// `along_v` (Rhino's ExtractIsocurve).
    pub fn isocurve_through(
        &self,
        face: usize,
        p: Point,
        along_v: bool,
    ) -> Result<Curve, KernelError> {
        // CUSTOM KERNEL OPERATION: isocurves
        let (uv, _) = self.nearest_on_face(face, p)?;
        let surface = NurbsSurface::of_face(&self.record()?, face)?;
        let iso = if along_v {
            surface.iso_u(uv[0])
        } else {
            surface.iso_v(uv[1])
        };
        iso.to_curve()
    }

    /// A curve pulled onto the body: each of its points to the nearest point of the surfaces,
    /// fitted through (Rhino's Pull).
    pub fn pulled(&self, curve: &Curve) -> Result<Curve, KernelError> {
        // CUSTOM KERNEL OPERATION: curves-on-surfaces
        let points = curve.divided(48)?;
        let on: Vec<Point> = points
            .iter()
            .map(|p| self.nearest_face(*p).map(|(_, _, q)| q))
            .collect::<Result<_, _>>()?;
        let closed = curve.is_closed()?;
        Curve::interpolated(
            if closed {
                on[..on.len() - 1].to_vec()
            } else {
                on
            },
            closed,
        )
    }

    /// A curve through points on face `face`, interpolated in the face's (u, v) so it lies on
    /// it (Rhino's InterpCrvOnSrf).
    pub fn curve_on_face(&self, face: usize, points: &[Point]) -> Result<Curve, KernelError> {
        let uvs: Vec<Point> = points
            .iter()
            .map(|p| {
                self.nearest_on_face(face, *p)
                    .map(|(uv, _)| [uv[0], uv[1], 0.0])
            })
            .collect::<Result<_, _>>()?;
        let in_uv = Spline::of(&Curve::interpolated(uvs, false)?)?;
        let surface =
            FaceSurface::of(&self.record()?, face).map_err(|e| failed("reading a face", e))?;
        let (t0, t1) = in_uv.domain();
        let lifted: Vec<Point> = (0..=64)
            .map(|k| {
                let uv = in_uv.point(t0 + (t1 - t0) * f64::from(k) / 64.0);
                surface
                    .point([uv[0], uv[1]])
                    .map_err(|e| failed("evaluating a face", e))
            })
            .collect::<Result<_, _>>()?;
        Curve::interpolated(lifted, false)
    }

    /// A curve lying on the body moved along the surfaces by `distance`, to the side of `toward`
    /// (Rhino's OffsetCrvOnSrf).
    pub fn offset_on_surface(
        &self,
        curve: &Curve,
        distance: f64,
        toward: Point,
    ) -> Result<Curve, KernelError> {
        let spline = Spline::of(curve)?;
        let (a, b) = spline.domain();
        let mut moved = Vec::new();
        let mut side = 0.0;
        for k in 0..=48 {
            let t = a + (b - a) * f64::from(k) / 48.0;
            let p = spline.point(t);
            let (face, uv, _) = self.nearest_face(p)?;
            let surface =
                FaceSurface::of(&self.record()?, face).map_err(|e| failed("reading a face", e))?;
            let n = surface
                .normal(uv)
                .map_err(|e| failed("evaluating a face", e))?;
            let along = unit(spline.derivative(t)).unwrap_or([1.0, 0.0, 0.0]);
            let mut out = unit(cross(n, along)).unwrap_or([0.0; 3]);
            if k == 0 {
                side = dot(sub(toward, p), out).signum();
                if side == 0.0 {
                    side = 1.0;
                }
            }
            out = out.map(|v| v * side);
            let (_, _, back) = self.nearest_face(plus(p, out, distance))?;
            moved.push(back);
        }
        let closed = curve.is_closed()?;
        Curve::interpolated(
            if closed {
                moved[..moved.len() - 1].to_vec()
            } else {
                moved
            },
            closed,
        )
    }

    /// The outline of the body seen along `view`: where its faces turn from facing the viewer
    /// to facing away, and its open borders (Rhino's Silhouette), from its display mesh.
    pub fn silhouette(&self, view: Point) -> Result<Vec<Curve>, KernelError> {
        // CUSTOM KERNEL OPERATION: silhouettes
        let mesh = self.mesh()?;
        let normal = |t: &[u32; 3]| {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
            cross(sub(b, a), sub(c, a))
        };
        // Each mesh edge (by its two corners' positions, rounded) and the triangles on it.
        let key = |p: Point| p.map(|v| (v * 1e6).round() as i64);
        let mut by_edge: std::collections::HashMap<([i64; 3], [i64; 3]), Vec<usize>> =
            std::collections::HashMap::new();
        for (index, t) in mesh.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (
                    key(mesh.positions[t[k] as usize]),
                    key(mesh.positions[t[(k + 1) % 3] as usize]),
                );
                let edge = if a < b { (a, b) } else { (b, a) };
                by_edge.entry(edge).or_default().push(index);
            }
        }
        let mut lines = Vec::new();
        let back = |k: [i64; 3]| k.map(|v| v as f64 / 1e6);
        for ((a, b), triangles) in &by_edge {
            let outline = match triangles.as_slice() {
                [only] => {
                    let _ = only;
                    true
                }
                [first, second] => {
                    let (n1, n2) = (
                        normal(&mesh.triangles[*first]),
                        normal(&mesh.triangles[*second]),
                    );
                    (dot(n1, view) > 0.0) != (dot(n2, view) > 0.0)
                }
                _ => false,
            };
            if outline && let Ok(line) = Curve::line(back(*a), back(*b)) {
                lines.push(line);
            }
        }
        crate::join_all(&lines)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    fn sheet(size: f64, z: f64) -> Body {
        let a = Curve::line([0.0, 0.0, z], [size, 0.0, z]).unwrap();
        let b = Curve::line([0.0, size, z], [size, size, z]).unwrap();
        Body::ruled(&a, &b, false).unwrap()
    }

    #[test]
    fn isocurves_lie_on_the_surface() {
        let s = sheet(4.0, 0.0);
        let iso = s.isocurve_through(0, [1.0, 2.0, 5.0], false).unwrap();
        let points = iso.sample().unwrap();
        assert!(points.iter().all(|p| p[2].abs() < 1e-9));
        assert!((iso.length().unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn curves_pulled_and_made_on_surfaces() {
        let s = sheet(6.0, 2.0);
        let pulled = s
            .pulled(&Curve::line([1.0, 1.0, 5.0], [5.0, 1.0, 0.0]).unwrap())
            .unwrap();
        assert!(
            pulled
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let on = s
            .curve_on_face(0, &[[1.0, 1.0, 9.0], [3.0, 4.0, 9.0], [5.0, 2.0, 9.0]])
            .unwrap();
        assert!(
            on.sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let base = Curve::line([1.0, 1.0, 2.0], [5.0, 1.0, 2.0]).unwrap();
        let moved = s.offset_on_surface(&base, 1.0, [3.0, 5.0, 2.0]).unwrap();
        let p = moved.sample().unwrap()[0];
        assert!(
            (p[1] - 2.0).abs() < 1e-3 && (p[2] - 2.0).abs() < 1e-6,
            "{p:?}"
        );
    }

    #[test]
    fn a_boxs_silhouette_from_above_is_its_outline() {
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap();
        let outline = cube.silhouette([0.0, 0.0, -1.0]).unwrap();
        let length: f64 = outline.iter().map(|c| c.length().unwrap()).sum();
        // The vertical faces are edge-on: their top and bottom edges both turn, 2 × 10.
        assert!(length >= 10.0 - 1e-6, "{length}");
    }
}
