//! More surface operations: silhouettes, unrolling ruled surfaces, extruding faces, and fillet
//! and chamfer surfaces between two surfaces. Flagged as custom
//! (`docs/kernel/custom-operations.md`).

use crate::body::Body;
use crate::{Combine, Curve, KernelError, Point, cross, dot, gap, sub, unit};

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

impl Body {
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

    /// A ruled, developable surface laid flat on the world XY plane (Rhino's UnrollSrf): its
    /// rulings unfolded strip by strip, keeping their lengths. Surfaces that are not
    /// developable are refused.
    pub fn unrolled(&self) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: unrolling
        let surface = crate::NurbsSurface::of_face(&self.record()?, 0)?;
        let ruled_v = surface.degree[1] == 1 && surface.points[0].len() == 2;
        let ruled_u = surface.degree[0] == 1 && surface.points.len() == 2;
        if !ruled_u && !ruled_v {
            return Err(KernelError(
                "only ruled surfaces can be unrolled exactly".to_owned(),
            ));
        }
        let [[u0, u1], [v0, v1]] = surface.domain();
        let steps = 96;
        // The rulings' ends: along the curved direction, each ruling from one side to the other.
        let ruling = |k: usize| -> (Point, Point) {
            let t = k as f64 / steps as f64;
            if ruled_v {
                let u = u0 + (u1 - u0) * t;
                (surface.point(u, v0), surface.point(u, v1))
            } else {
                let v = v0 + (v1 - v0) * t;
                (surface.point(u0, v), surface.point(u1, v))
            }
        };
        // Check developable: neighbouring rulings lie in one plane.
        for k in 0..steps {
            let (a, b) = ruling(k);
            let (c, d) = ruling(k + 1);
            let n = cross(sub(b, a), sub(c, a));
            let off = dot(unit(n).unwrap_or([0.0; 3]), sub(d, a)).abs();
            if off > 1e-3 * gap(a, b).max(gap(a, c)).max(1.0) {
                return Err(KernelError(
                    "the surface is not developable, so it cannot be unrolled".to_owned(),
                ));
            }
        }
        // Unfold: place each new point by its distances to the two placed before it.
        let flat2 = |p: Point, q: Point, dp: f64, dq: f64, away_from: [f64; 2]| -> [f64; 2] {
            let base = gap(p, q).max(1e-12);
            let a = (dp * dp - dq * dq + base * base) / (2.0 * base);
            let h = (dp * dp - a * a).max(0.0).sqrt();
            let e = [(q[0] - p[0]) / base, (q[1] - p[1]) / base];
            let foot = [p[0] + a * e[0], p[1] + a * e[1]];
            let n = [-e[1], e[0]];
            let one = [foot[0] + h * n[0], foot[1] + h * n[1]];
            let two = [foot[0] - h * n[0], foot[1] - h * n[1]];
            let side = |s: [f64; 2]| (s[0] - p[0]) * n[0] + (s[1] - p[1]) * n[1];
            if side(one).signum() != side(away_from).signum() {
                one
            } else {
                two
            }
        };
        let (a0, b0) = ruling(0);
        let mut left = vec![[0.0, 0.0]];
        let mut right = vec![[0.0, gap(a0, b0)]];
        let mut previous = (a0, b0);
        for k in 1..=steps {
            let (a, b) = ruling(k);
            let (pa, pb) = (left[k - 1], right[k - 1]);
            let p3 = |q: [f64; 2]| [q[0], q[1], 0.0];
            let behind = if k >= 2 {
                left[k - 2]
            } else {
                [-1.0, gap(a0, b0) / 2.0]
            };
            let na = flat2(
                p3(pa),
                p3(pb),
                gap(previous.0, a),
                gap(previous.1, a),
                behind,
            );
            let nb = flat2(p3(na), p3(pb), gap(a, b), gap(previous.1, b), pa);
            left.push(na);
            right.push(nb);
            previous = (a, b);
        }
        let mut outline: Vec<Point> = left.iter().map(|p| [p[0], p[1], 0.0]).collect();
        outline.extend(right.iter().rev().map(|p| [p[0], p[1], 0.0]));
        let ring = Curve::polyline(outline, true)?;
        Body::planar(&[ring])
    }

    /// The flat face `face` pulled out along its normal by `distance` and joined to the solid
    /// (Rhino's ExtrudeFace).
    pub fn face_extruded(&self, face: usize, distance: f64) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: face-extrusion
        let lone = self.faces_only(&[face])?;
        let edges = lone.naked_edges()?;
        let curves = edges
            .into_iter()
            .map(|e| lone.edge_curve(e))
            .collect::<Result<Vec<_>, _>>()?;
        let loops = crate::join_all(&curves)?;
        let mesh = lone.mesh()?;
        let normal = mesh
            .normals
            .first()
            .copied()
            .ok_or_else(|| KernelError("an empty face".to_owned()))?;
        let normal = if distance < 0.0 {
            normal.map(|v| -v)
        } else {
            normal
        };
        let block = Body::extruded(&loops, normal, distance.abs(), true)?;
        let how = if distance >= 0.0 {
            Combine::Union
        } else {
            Combine::Difference
        };
        self.combined(&block, how)?
            .ok_or_else(|| KernelError("nothing was left".to_owned()))
    }

    /// The rolling-ball fillet (or, with `chamfer`, the chamfer) of `radius` between this surface
    /// and `other`, on the sides facing each other (Rhino's FilletSrf and ChamferSrf): its spine
    /// where the two surfaces offset by the radius meet, round arcs (or lines) between where the
    /// ball touches them, skinned.
    pub fn filleted_with(
        &self,
        other: &Body,
        radius: f64,
        chamfer: bool,
    ) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: surface-fillets
        let towards = |from: &Body, to: &Body| -> Result<Body, KernelError> {
            let target = to.centroid().or_else(|_| {
                to.bounds()
                    .map(|(l, h)| std::array::from_fn(|i| (l[i] + h[i]) / 2.0))
            })?;
            let mut best: Option<(f64, Body)> = None;
            for sign in [1.0, -1.0] {
                let body = from.engine_body()?;
                if solidcore_ops::offset_sheet(&body, sign * radius).is_err() {
                    continue;
                }
                let moved = Body::made(&body)?;
                let (_, _, near) = moved.nearest_face(target)?;
                let d = gap(near, target);
                if best.as_ref().is_none_or(|(b, _)| d < *b) {
                    best = Some((d, moved));
                }
            }
            best.map(|(_, b)| b)
                .ok_or_else(|| KernelError("could not offset a surface by the radius".to_owned()))
        };
        let (a_off, b_off) = (towards(self, other)?, towards(other, self)?);
        let spines = a_off.intersections(&b_off)?;
        let spine = spines.first().ok_or_else(|| {
            KernelError("the surfaces are too far apart for that radius".to_owned())
        })?;
        let centres = spine.divided(24)?;
        let mut sections = Vec::new();
        for c in centres {
            let (_, _, a) = self.nearest_face(c)?;
            let (_, _, b) = other.nearest_face(c)?;
            let section = if chamfer {
                Curve::line(a, b)?
            } else {
                let x =
                    unit(sub(a, c)).ok_or_else(|| KernelError("a degenerate fillet".to_owned()))?;
                let n = unit(cross(x, sub(b, c)))
                    .ok_or_else(|| KernelError("a degenerate fillet".to_owned()))?;
                let y = cross(n, x);
                Curve::arc(c, a, plus(c, unit(sub(b, c)).unwrap_or(x), gap(a, c)), x, y)?
            };
            sections.push(section);
        }
        Body::lofted(&sections, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    #[test]
    fn a_boxs_silhouette_from_above_is_its_outline() {
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap();
        let outline = cube.silhouette([0.0, 0.0, -1.0]).unwrap();
        let length: f64 = outline.iter().map(|c| c.length().unwrap()).sum();
        // The vertical faces are edge-on: their top and bottom edges both turn, 2 × 10.
        assert!(length >= 10.0 - 1e-6, "{length}");
    }

    #[test]
    fn a_cone_strip_unrolls_keeping_its_area() {
        let low = Curve::arc(
            [0.0; 3],
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let high = Curve::arc(
            [0.0, 0.0, 3.0],
            [2.0, 0.0, 3.0],
            [0.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let strip = Body::ruled(&low, &high, false).unwrap();
        let flat = strip.unrolled().unwrap();
        let (a, b) = (strip.area().unwrap(), flat.area().unwrap());
        assert!((a - b).abs() < 0.01 * a, "{a} against {b}");
    }

    #[test]
    fn a_boxs_top_extruded_makes_it_taller() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let mesh = cube.mesh().unwrap();
        let top = mesh
            .triangle_faces
            .iter()
            .zip(&mesh.triangles)
            .find(|(_, t)| {
                t.iter()
                    .all(|i| (mesh.positions[*i as usize][2] - 1.0).abs() < 1e-9)
            })
            .map(|(f, _)| *f)
            .unwrap();
        let taller = cube.face_extruded(top, 0.5).unwrap();
        assert!(
            (taller.volume().unwrap() - 1.5).abs() < 1e-6,
            "{}",
            taller.volume().unwrap()
        );
    }

    #[test]
    fn a_fillet_between_two_planes_at_right_angles() {
        let floor = Body::ruled(
            &Curve::line([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]).unwrap(),
            false,
        )
        .unwrap();
        let wall = Body::ruled(
            &Curve::line([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 0.0, 10.0], [10.0, 0.0, 10.0]).unwrap(),
            false,
        )
        .unwrap();
        let fillet = floor.filleted_with(&wall, 2.0, false).unwrap();
        // A quarter cylinder of radius 2 along 10: area π r / 2 × 10.
        let area = fillet.area().unwrap();
        let expected = std::f64::consts::PI * 2.0 / 2.0 * 10.0;
        assert!(
            (area - expected).abs() < 0.02 * expected,
            "{area} against {expected}"
        );
        let chamfer = floor.filleted_with(&wall, 2.0, true).unwrap();
        assert!((chamfer.area().unwrap() - 2.0 * 2f64.sqrt() * 10.0).abs() < 0.2);
    }
}
