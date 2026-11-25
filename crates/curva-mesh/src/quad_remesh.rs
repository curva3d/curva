//! QuadRemesh: a mesh or a SubD's smooth surface turned into an even grid of quads, on quadrs
//! (decision 0033, spec `docs/specs/mesh-and-subd.md` §3.5). This file is the only one that calls
//! it.
//!
//! quadrs follows Instant Meshes (Jakob, Tarini, Panozzo and Sorkine-Hornung, "Instant
//! field-aligned meshes", SIGGRAPH Asia 2015): it lays a field of directions over the surface and
//! grows quads along it. Given the same mesh and the same seed it makes the same quads every
//! time, on every system: Curva uses a patched copy of quadrs 1.0.0 (`vendor/quadrs`), as 1.0.0
//! walks its hash maps in a different order on each run and uses each system's own sines and
//! exponentials, which can differ in the last digit. Points go in measured from the centre, so
//! a mesh far from the origin keeps its digits.

use crate::{Mesh, Point, SubD};
use std::collections::BTreeMap;
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
    /// Keep hard edges, as Rhino's `DetectHardEdges`: edges where the faces meet at more than
    /// [`HARD_EDGE_ANGLE`] degrees, with points along them and on their corners.
    pub hard_edges: bool,
}

/// Where faces meet at more than this many degrees, the edge is hard. Measured on Rhino 8's
/// QuadRemesh: a fold of 30° is smoothed over and one of 31° kept.
pub const HARD_EDGE_ANGLE: f64 = 30.0;

impl QuadRemeshOptions {
    /// About `count` quads, with Curva's fixed seed and hard edges kept, as Rhino's defaults
    /// (Rhino's own count is 2000).
    #[must_use]
    pub fn count(count: usize) -> Self {
        Self {
            target: QuadTarget::Count(count),
            seed: 1,
            hard_edges: true,
        }
    }
}

/// How much longer than `TargetEdgeLength` Rhino 8's quads come out, measured on a 40 × 40
/// sphere of radius 5: 1,064, 244 and 59 quads at 0.5, 1 and 2, sides 1.09 to 1.15 times as long.
pub const RHINO_EDGE_RATIO: f64 = 1.12;

/// A target edge length as the quad count that gives Rhino's quad size over `area`, since
/// quadrs' own edge length makes quads about 0.85 times as long; a count is kept as it is.
fn counted(target: QuadTarget, area: f64) -> QuadTarget {
    match target {
        QuadTarget::EdgeLength(l) if l > 0.0 && area > 0.0 => {
            let side = RHINO_EDGE_RATIO * l;
            QuadTarget::Count((area / (side * side)).round().max(1.0) as usize)
        }
        other => other,
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

/// quadrs run for about `n` faces.
// CUSTOM MESH OPERATION: quad-remesh-count
// quadrs' count is rough (a fifth over is common), so it runs again with the request scaled by
// how far off it came, keeping the nearest of three tries.
fn nearest_count(
    n: usize,
    run: impl Fn(quadrs::RemeshTarget) -> Result<quadrs::RemeshResult, QuadRemeshError>,
) -> Result<quadrs::RemeshResult, QuadRemeshError> {
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
        .ok_or_else(|| QuadRemeshError("no result".to_owned()))
}

impl Mesh {
    /// The mesh remeshed into quads, as Rhino's QuadRemesh does with its output set to a mesh.
    /// The input is taken as triangles; normals, texture coordinates and colours are not carried.
    ///
    /// # Errors
    /// When the mesh has no faces, or quadrs cannot remesh it.
    pub fn quad_remeshed(&self, options: &QuadRemeshOptions) -> Result<Self, QuadRemeshError> {
        // Points at one place are joined first, so an unwelded edge (a MeshBox's, #690) is
        // remeshed across as Rhino does, and the result stays whole.
        self.welded(0.0).0.quad_remeshed_joined(options)
    }

    /// [`Mesh::quad_remeshed`] on a mesh whose points are already joined.
    fn quad_remeshed_joined(&self, options: &QuadRemeshOptions) -> Result<Self, QuadRemeshError> {
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
            settings.crease_angle = options.hard_edges.then_some(HARD_EDGE_ANGLE);
            quadrs::remesh(&input, &settings).map_err(|e| QuadRemeshError(format!("{e:?}")))
        };
        let result = match counted(options.target, self.area()) {
            QuadTarget::EdgeLength(l) => run(quadrs::RemeshTarget::EdgeLength(2.0 * l))?,
            QuadTarget::Count(n) => nearest_count(n, run)?,
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
        let mut points = onto_triangles(&points, self.vertices(), &triangles);
        if options.hard_edges {
            let side = (self.area() / faces.len().max(1) as f64).sqrt();
            onto_hard_edges(&mut points, self.vertices(), &triangles, side);
        }
        let mesh = Mesh::new(points, &faces).map_err(|e| QuadRemeshError(e.to_string()))?;
        Ok(mesh.without_unused_vertices().0)
    }
}

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

fn distance(a: Point, b: Point) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

// CUSTOM MESH OPERATION: quad-remesh-hard-edges
/// The finishing step for hard edges. quadrs, held along them (its `crease_angle`), brings its
/// points within about a fifth of a quad of each hard edge; Rhino's QuadRemesh puts them exactly
/// on, corners included. So the point nearest each corner (where three or more hard edges meet)
/// goes onto it, and points within `0.3 × side` of a hard edge go onto the edge. `side` is about a
/// quad's side.
fn onto_hard_edges(points: &mut [Point], vertices: &[Point], triangles: &[[u32; 3]], side: f64) {
    let normal = |t: &[u32; 3]| {
        let (a, b, c) = (
            vertices[t[0] as usize],
            vertices[t[1] as usize],
            vertices[t[2] as usize],
        );
        let (u, v) = (sub(b, a), sub(c, a));
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = dot(n, n).sqrt().max(f64::MIN_POSITIVE);
        n.map(|x| x / length)
    };
    // Edges by their two points' places, so an unwelded edge counts as one.
    let mut place_of: std::collections::HashMap<[u64; 3], u32> = std::collections::HashMap::new();
    let place = |v: u32, place_of: &mut std::collections::HashMap<[u64; 3], u32>| {
        *place_of
            .entry(vertices[v as usize].map(f64::to_bits))
            .or_insert(v)
    };
    let mut faces_of: BTreeMap<(u32, u32), Vec<Point>> = BTreeMap::new();
    for t in triangles {
        let n = normal(t);
        for k in 0..3 {
            let (a, b) = (
                place(t[k], &mut place_of),
                place(t[(k + 1) % 3], &mut place_of),
            );
            faces_of.entry((a.min(b), a.max(b))).or_default().push(n);
        }
    }
    let limit = HARD_EDGE_ANGLE.to_radians().cos();
    let hard: Vec<(u32, u32)> = faces_of
        .iter()
        .filter(|(_, n)| n.len() == 2 && dot(n[0], n[1]) < limit)
        .map(|(&e, _)| e)
        .collect();
    if hard.is_empty() {
        return;
    }
    let mut ends: BTreeMap<u32, usize> = BTreeMap::new();
    for &(a, b) in &hard {
        *ends.entry(a).or_default() += 1;
        *ends.entry(b).or_default() += 1;
    }
    let mut placed = vec![false; points.len()];
    for (&corner, _) in ends.iter().filter(|&(_, &n)| n >= 3) {
        let c = vertices[corner as usize];
        let nearest = (0..points.len())
            .filter(|&i| !placed[i])
            .min_by(|&i, &j| distance(points[i], c).total_cmp(&distance(points[j], c)));
        if let Some(i) = nearest
            && distance(points[i], c) < 0.5 * side
        {
            points[i] = c;
            placed[i] = true;
        }
    }
    // Hard edges listed by grid cell, so each point looks only at those nearby.
    let cell = |p: Point| -> [i64; 3] { p.map(|x| (x / side).floor() as i64) };
    let mut near: std::collections::HashMap<[i64; 3], Vec<usize>> =
        std::collections::HashMap::new();
    for (k, &(a, b)) in hard.iter().enumerate() {
        let (ca, cb) = (cell(vertices[a as usize]), cell(vertices[b as usize]));
        for x in ca[0].min(cb[0]) - 1..=ca[0].max(cb[0]) + 1 {
            for y in ca[1].min(cb[1]) - 1..=ca[1].max(cb[1]) + 1 {
                for z in ca[2].min(cb[2]) - 1..=ca[2].max(cb[2]) + 1 {
                    near.entry([x, y, z]).or_default().push(k);
                }
            }
        }
    }
    for (i, p) in points.iter_mut().enumerate() {
        if placed[i] {
            continue;
        }
        let mut best = (0.3 * side, None);
        for &k in near.get(&cell(*p)).into_iter().flatten() {
            let (a, b) = (vertices[hard[k].0 as usize], vertices[hard[k].1 as usize]);
            let d = sub(b, a);
            let t = (dot(sub(*p, a), d) / dot(d, d).max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            let on: Point = std::array::from_fn(|j| a[j] + t * d[j]);
            let gap = distance(on, *p);
            if gap < best.0 {
                best = (gap, Some(on));
            }
        }
        if let Some(on) = best.1 {
            *p = on;
        }
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

    /// A sphere of radius 10 from a cube's 16 × 16 grids pushed out from the centre. Only square
    /// roots make it, which every system works out alike, so it is the same on every machine (a
    /// UV sphere's sines and cosines can differ in the last digit from system to system).
    fn cube_sphere() -> Mesh {
        let cube = Mesh::mesh_box(&Frame::world(), [2.0; 3], [16; 3]);
        let points: Vec<Point> = cube
            .vertices()
            .iter()
            .map(|p| {
                let d: Point = std::array::from_fn(|i| p[i] - 1.0);
                let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                d.map(|c| 10.0 * c / r)
            })
            .collect();
        let faces: Vec<Vec<u32>> = cube.faces().map(<[u32]>::to_vec).collect();
        Mesh::new(points, &faces).unwrap()
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
        // Instant Meshes is sensitive to its input's last digits, and moving a mesh rounds them,
        // so a moved mesh's quads grow differently. What must hold is precision: the
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
        let out = cube_sphere()
            .quad_remeshed(&QuadRemeshOptions::count(200))
            .unwrap();
        assert_eq!((out.face_count(), out.vertex_count()), (198, 200));
        let first = out.vertices()[0];
        let pinned = [-4.083_161_280, -7.758_225_646, -4.793_129_703];
        assert!(
            (0..3).all(|i| (first[i] - pinned[i]).abs() < 1e-6),
            "{first:?}"
        );
    }

    /// How far inside a 20 mm box (from the origin) the middle of the deepest face lies, and the
    /// result's volume.
    fn cut_inside_box(options: &QuadRemeshOptions) -> (f64, f64) {
        let cube = Mesh::mesh_box(&Frame::world(), [20.0; 3], [10; 3]);
        let out = cube.quad_remeshed(options).unwrap();
        let p = out.vertices();
        let deepest = out
            .faces()
            .map(|f| {
                let m: Point = std::array::from_fn(|i| {
                    f.iter().map(|&v| p[v as usize][i]).sum::<f64>() / f.len() as f64
                });
                (0..3)
                    .map(|i| m[i].min(20.0 - m[i]))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(0.0, f64::max);
        (deepest, out.volume().unwrap().abs())
    }

    #[test]
    fn a_box_keeps_its_hard_edges_and_corners_as_in_rhino() {
        // Rhino 8's QuadRemesh on the same box, 600 quads, DetectHardEdges: no face inside,
        // volume 8000.0; without it, faces 0.063 inside.
        let (deepest, volume) = cut_inside_box(&QuadRemeshOptions::count(600));
        assert!(deepest < 1e-9, "a face {deepest} inside the box");
        assert!((volume - 8000.0).abs() < 1e-6, "volume {volume}");
        let soft = QuadRemeshOptions {
            hard_edges: false,
            ..QuadRemeshOptions::count(600)
        };
        let (deepest, volume) = cut_inside_box(&soft);
        assert!(deepest > 0.1, "without hard edges the corners round off");
        assert!(volume < 7990.0);
    }

    #[test]
    fn a_fold_is_hard_above_30_degrees_as_in_rhino() {
        // A 20 × 10 plate folded along x = 10, welded.
        let plate = |degrees: f64| {
            let slope = degrees.to_radians().tan();
            let points: Vec<Point> = (0..21)
                .flat_map(|i| {
                    (0..11).map(move |j| {
                        let x = f64::from(i);
                        [
                            x,
                            f64::from(j),
                            if x <= 10.0 { 0.0 } else { (x - 10.0) * slope },
                        ]
                    })
                })
                .collect();
            let faces: Vec<[u32; 4]> = (0..20)
                .flat_map(|i| (0..10).map(move |j| i * 11 + j))
                .map(|a| [a, a + 11, a + 12, a + 1])
                .collect();
            Mesh::new(points, &faces).unwrap()
        };
        let on_fold = |degrees: f64| {
            let out = plate(degrees)
                .quad_remeshed(&QuadRemeshOptions::count(200))
                .unwrap();
            out.vertices()
                .iter()
                .filter(|p| p[0] == 10.0 && p[2] == 0.0)
                .count()
        };
        // Rhino 8 put 22 points on a 31° fold and none on a 30° one.
        assert!(on_fold(45.0) >= 8, "{} points on a 45° fold", on_fold(45.0));
        assert_eq!(on_fold(20.0), 0, "a 20° fold is smoothed over");
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
