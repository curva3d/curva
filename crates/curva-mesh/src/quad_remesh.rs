//! QuadRemesh: a mesh or a SubD's smooth surface turned into an even grid of quads, on quadrs
//! (decision 0033, spec `docs/specs/mesh-and-subd.md` §3.5). This file is the only one that calls
//! it.
//!
//! quadrs follows Instant Meshes (Jakob, Tarini, Panozzo and Sorkine-Hornung, "Instant
//! field-aligned meshes", SIGGRAPH Asia 2015): it lays a field of directions over the surface and
//! grows quads along it. Given the same mesh and the same seed it makes the same quads every
//! time: Curva uses a patched copy of quadrs 1.0.0 (`vendor/quadrs`), as 1.0.0 itself walks its
//! hash maps in a different order on each run. Points go in measured from the mesh's centre, so
//! a mesh far from the origin keeps its digits (quadrs works in single precision).

use crate::{Mesh, Point, SubD};
use std::fmt;

/// How dense the quads should be, as Rhino's `TargetQuadCount` and `TargetEdgeLength`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QuadTarget {
    /// About this many quads.
    Count(usize),
    /// Quads about this long on each side.
    EdgeLength(f64),
}

/// What QuadRemesh takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadRemeshOptions {
    /// How dense the quads should be.
    pub target: QuadTarget,
    /// The seed that makes the result repeatable; the same seed gives the same quads.
    pub seed: u64,
}

impl QuadRemeshOptions {
    /// About `count` quads, with Curva's fixed seed. Rhino's own default is 2000.
    #[must_use]
    pub fn count(count: usize) -> Self {
        Self {
            target: QuadTarget::Count(count),
            seed: 1,
        }
    }
}

/// Why QuadRemesh could not run. The text is for logs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadRemeshError(pub String);

impl fmt::Display for QuadRemeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QuadRemeshError {}

impl Mesh {
    /// The mesh remeshed into quads, as Rhino's QuadRemesh does with its output set to a mesh.
    /// The input is taken as triangles; normals, texture coordinates and colours are not carried.
    ///
    /// # Errors
    /// When the mesh has no faces, or quadrs cannot remesh it.
    pub fn quad_remeshed(&self, options: &QuadRemeshOptions) -> Result<Self, QuadRemeshError> {
        let triangles = self.triangles().triangles;
        if triangles.is_empty() {
            return Err(QuadRemeshError("there are no faces to remesh".to_owned()));
        }
        let centre = self.bounds().map_or([0.0; 3], |(lo, hi)| {
            std::array::from_fn(|i| f64::midpoint(lo[i], hi[i]))
        });
        let input = quadrs::Mesh {
            vertices: self
                .vertices()
                .iter()
                .map(|p| quadrs::Vec3::new(p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]))
                .collect(),
            faces: triangles
                .iter()
                .map(|t| t.iter().map(|&v| v as usize).collect())
                .collect(),
        };
        // quadrs sizes its quads for the target, then splits each into four as its last step, so
        // it is asked for a quarter of the count and twice the edge length.
        let run = |target| {
            let mut settings = quadrs::RemeshOptions::new(target);
            settings.seed = Some(options.seed);
            quadrs::remesh(&input, &settings).map_err(|e| QuadRemeshError(format!("{e:?}")))
        };
        let result = match options.target {
            QuadTarget::EdgeLength(l) => run(quadrs::RemeshTarget::EdgeLength(2.0 * l))?,
            QuadTarget::Count(n) => {
                // CUSTOM MESH OPERATION: quad-remesh-count
                // quadrs' count is rough (a fifth over is common), so it runs again with the
                // request scaled by how far off it came, keeping the nearest of three tries.
                let wanted = n.max(1) as f64;
                let mut ask = wanted / 4.0;
                let mut best: Option<(f64, quadrs::RemeshResult)> = None;
                for _ in 0..3 {
                    let result = run(quadrs::RemeshTarget::FaceCount(
                        ask.round().max(1.0) as usize
                    ))?;
                    let got = result.mesh.faces.len().max(1) as f64;
                    let miss = (got / wanted - 1.0).abs();
                    if best.as_ref().is_none_or(|(m, _)| miss < *m) {
                        best = Some((miss, result));
                    }
                    if miss <= 0.03 {
                        break;
                    }
                    ask *= wanted / got;
                }
                best.map(|(_, r)| r)
                    .ok_or_else(|| QuadRemeshError("no result".to_owned()))?
            }
        };
        let points: Vec<Point> = result
            .mesh
            .vertices
            .iter()
            .map(|p| [p.x + centre[0], p.y + centre[1], p.z + centre[2]])
            .collect();
        // Faces quadrs could not make whole (a point twice, fewer than three) are left out.
        let faces: Vec<Vec<u32>> = result
            .mesh
            .faces
            .iter()
            .filter_map(|f| {
                let face: Vec<u32> = f
                    .iter()
                    .map(|&v| u32::try_from(v).unwrap_or(u32::MAX))
                    .collect();
                let distinct = (1..face.len()).all(|i| !face[..i].contains(&face[i]));
                (face.len() >= 3 && distinct && face.iter().all(|&v| (v as usize) < points.len()))
                    .then_some(face)
            })
            .collect();
        let points = onto_triangles(&points, self.vertices(), &triangles);
        let mesh = Mesh::new(points, &faces).map_err(|e| QuadRemeshError(e.to_string()))?;
        Ok(mesh.without_unused_vertices().0)
    }
}

// CUSTOM MESH OPERATION: quad-remesh-onto-surface
/// Each point moved to the nearest place on the triangles, as Rhino's QuadRemesh leaves its points
/// on the original: quadrs places them by averaging and leaves them off a curved surface. A grid of
/// cells, each listing the triangles whose bounds reach it, keeps the search near each point.
fn onto_triangles(points: &[Point], vertices: &[Point], triangles: &[[u32; 3]]) -> Vec<Point> {
    let corner = |t: &[u32; 3], k: usize| vertices[t[k] as usize];
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in vertices {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let span = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max).max(1e-12);
    // About as many cells as triangles along the longest side's cube.
    let cells = (triangles.len() as f64).cbrt().clamp(1.0, 64.0);
    let size = span / cells;
    let cell =
        |p: Point| -> [i64; 3] { std::array::from_fn(|i| ((p[i] - lo[i]) / size).floor() as i64) };
    let mut grid: std::collections::HashMap<[i64; 3], Vec<usize>> =
        std::collections::HashMap::new();
    for (n, t) in triangles.iter().enumerate() {
        let (a, b) = (0..3).fold(([i64::MAX; 3], [i64::MIN; 3]), |(a, b), k| {
            let c = cell(corner(t, k));
            (
                std::array::from_fn(|i| a[i].min(c[i])),
                std::array::from_fn(|i| b[i].max(c[i])),
            )
        });
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    grid.entry([x, y, z]).or_default().push(n);
                }
            }
        }
    }
    let reach = (span / size).ceil() as i64 + 1;
    points
        .iter()
        .map(|&p| {
            let home = cell(p);
            let mut best = (f64::INFINITY, p);
            // Rings of cells outwards, until the nearest found is nearer than the next ring.
            for r in 0..=reach {
                for x in -r..=r {
                    for y in -r..=r {
                        for z in -r..=r {
                            if x.abs().max(y.abs()).max(z.abs()) != r {
                                continue;
                            }
                            let key = [home[0] + x, home[1] + y, home[2] + z];
                            for &n in grid.get(&key).into_iter().flatten() {
                                let t = &triangles[n];
                                let q = nearest_on_triangle(
                                    p,
                                    corner(t, 0),
                                    corner(t, 1),
                                    corner(t, 2),
                                );
                                let d = (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f64>();
                                if d < best.0 {
                                    best = (d, q);
                                }
                            }
                        }
                    }
                }
                if best.0.sqrt() <= r as f64 * size {
                    break;
                }
            }
            best.1
        })
        .collect()
}

/// The nearest point to `p` on triangle `abc` (Ericson, "Real-Time Collision Detection", 5.1.5).
fn nearest_on_triangle(p: Point, a: Point, b: Point, c: Point) -> Point {
    let sub = |u: Point, v: Point| -> Point { std::array::from_fn(|i| u[i] - v[i]) };
    let dot = |u: Point, v: Point| (0..3).map(|i| u[i] * v[i]).sum::<f64>();
    let along = |o: Point, d: Point, t: f64| -> Point { std::array::from_fn(|i| o[i] + t * d[i]) };
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return along(a, ab, d1 / (d1 - d3));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return along(a, ac, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return along(b, sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    std::array::from_fn(|i| a[i] + ab[i] * v + ac[i] * w)
}

impl SubD {
    /// A mesh remeshed into quads as a SubD, Rhino's QuadRemesh default output: the quads become
    /// the cage.
    ///
    /// # Errors
    /// When the mesh cannot be remeshed.
    pub fn quad_remeshed_from(
        mesh: &Mesh,
        options: &QuadRemeshOptions,
    ) -> Result<Self, QuadRemeshError> {
        Ok(Self::new(mesh.quad_remeshed(options)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame, SphereStyle};

    fn sphere() -> Mesh {
        Mesh::mesh_sphere(
            &Frame::world(),
            10.0,
            SphereStyle::Uv {
                vertical: 40,
                around: 60,
            },
        )
    }

    fn quads(m: &Mesh) -> usize {
        m.faces().filter(|f| f.len() == 4).count()
    }

    #[test]
    fn a_sphere_becomes_mostly_quads_near_the_count_asked_for() {
        let out = sphere()
            .quad_remeshed(&QuadRemeshOptions::count(400))
            .unwrap();
        let n = out.face_count();
        assert!(
            (360..=440).contains(&n),
            "{n} faces for 400 asked (spec: within 10%)"
        );
        assert!(
            quads(&out) * 10 >= n * 9,
            "{} of {n} are quads",
            quads(&out)
        );
        assert!(out.is_closed());
        let ball = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
        let kept = out.volume().unwrap().abs() / ball;
        assert!(kept > 0.95 && kept < 1.02, "kept {kept}");
    }

    #[test]
    fn the_same_seed_gives_the_same_quads() {
        let options = QuadRemeshOptions::count(200);
        assert_eq!(
            sphere().quad_remeshed(&options),
            sphere().quad_remeshed(&options)
        );
    }

    #[test]
    fn far_from_the_origin_the_quads_stay_on_the_surface() {
        let near = sphere()
            .quad_remeshed(&QuadRemeshOptions::count(200))
            .unwrap();
        let far = sphere()
            .translated([1.0e6, -2.0e6, 3.0e5])
            .quad_remeshed(&QuadRemeshOptions::count(200))
            .unwrap();
        // quadrs works in single precision, and Instant Meshes is sensitive to its input's last
        // digits, so a moved mesh's quads grow differently. What must hold is precision: the
        // points sit on the original as closely far away as at the origin: within the input
        // sphere's own faceting (at most 0.0214 inside radius 10, at its faces' middles).
        let centre = [1.0e6, -2.0e6, 3.0e5];
        for (out, at) in [(&near, [0.0; 3]), (&far, centre)] {
            assert!(out.is_closed());
            assert!(quads(out) * 10 >= out.face_count() * 9);
            for p in out.vertices() {
                let r = (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>().sqrt();
                assert!((r - 10.0).abs() < 0.025, "a point {r} from the centre");
            }
        }
    }

    #[test]
    fn a_subd_comes_out_with_the_quads_as_its_cage() {
        let subd = SubD::quad_remeshed_from(&sphere(), &QuadRemeshOptions::count(200)).unwrap();
        assert!(subd.cage().face_count() > 100);
        assert_eq!(subd.creases().count(), 0, "a closed cage has no border");
    }

    /// The upgrade guard (spec §3.4): quadrs' result pinned, so moving to another quadrs (the
    /// author's release with the fix, `docs/mesh/custom-operations.md`) fails here until the
    /// change is looked at and the numbers are updated.
    #[test]
    fn the_result_is_pinned_for_upgrades() {
        let out = sphere()
            .quad_remeshed(&QuadRemeshOptions::count(200))
            .unwrap();
        assert_eq!((out.face_count(), out.vertex_count()), (210, 212));
        let first = out.vertices()[0];
        let pinned = [1.781_249_447, -1.201_569_620, -9.760_140_391];
        assert!(
            (0..3).all(|i| (first[i] - pinned[i]).abs() < 1e-6),
            "{first:?}"
        );
    }

    #[test]
    fn an_empty_mesh_is_refused() {
        assert!(
            Mesh::default()
                .quad_remeshed(&QuadRemeshOptions::count(10))
                .is_err()
        );
    }
}
