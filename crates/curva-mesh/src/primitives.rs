//! Mesh primitives, as Rhino's MeshBox, MeshPlane, MeshSphere, MeshCylinder, MeshCone and
//! MeshTorus make them (spec `docs/specs/mesh-and-subd.md` §1): the same face counts and cap
//! styles, faces pointing outwards, and normals that shade them as Rhino does (flat on a box's
//! sides and a cylinder's caps, smooth around a sphere).
//!
//! Each is built in a [`Frame`]: a place and three directions, so a command can set one on any
//! construction plane, along a curve or between picked points. Counts below a shape's least are
//! raised to it, as Rhino's commands do.

use crate::{Attribute, Mesh, Point};
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Where a primitive is built: its origin and three directions of unit length at right angles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The origin: a box's or plane's corner, a sphere's or torus's centre, a cylinder's or cone's
    /// base centre.
    pub origin: Point,
    /// The first direction.
    pub x: Point,
    /// The second direction.
    pub y: Point,
    /// The third direction, `x` × `y`; a cylinder or cone rises along it.
    pub z: Point,
}

impl Frame {
    /// The world's own frame at the origin.
    #[must_use]
    pub fn world() -> Self {
        Self {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        }
    }

    /// The point `a`, `b`, `c` along its directions from its origin.
    #[must_use]
    pub fn at(&self, a: f64, b: f64, c: f64) -> Point {
        std::array::from_fn(|i| self.origin[i] + a * self.x[i] + b * self.y[i] + c * self.z[i])
    }

    /// A direction given in the frame's terms, in the world's.
    fn direction(&self, a: f64, b: f64, c: f64) -> Point {
        std::array::from_fn(|i| a * self.x[i] + b * self.y[i] + c * self.z[i])
    }
}

/// How MeshSphere lays out its faces (Rhino's `UV`, `Quads` and `Triangles` styles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SphereStyle {
    /// Rings of quads with a fan of triangles at each pole.
    Uv {
        /// Faces from pole to pole (at least 2).
        vertical: u32,
        /// Faces around (at least 3).
        around: u32,
    },
    /// A cube's six squares, each split in four `subdivisions` times (0 to 6, as Rhino), pushed
    /// out to the sphere.
    Quads {
        /// How many times each square splits.
        subdivisions: u32,
    },
    /// An icosahedron's twenty triangles, each split in four `subdivisions` times (0 to 5), pushed
    /// out to the sphere.
    Triangles {
        /// How many times each triangle splits.
        subdivisions: u32,
    },
}

/// Builds a mesh from points, faces and a normal for each corner.
struct Builder {
    points: Vec<Point>,
    faces: Vec<Vec<u32>>,
    normals: Vec<Point>,
}

impl Builder {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            faces: Vec::new(),
            normals: Vec::new(),
        }
    }

    fn point(&mut self, p: Point) -> u32 {
        self.points.push(p);
        u32::try_from(self.points.len() - 1).unwrap_or(u32::MAX)
    }

    fn face(&mut self, corners: &[u32], normals: &[Point]) {
        self.faces.push(corners.to_vec());
        self.normals.extend_from_slice(normals);
    }

    /// The mesh, with normals kept per point where every corner of a point agrees.
    fn mesh(self) -> Mesh {
        // The builders make faces of distinct, existing, finite points.
        let mesh = Mesh::new(self.points, &self.faces).unwrap_or_default();
        let corners = Attribute::PerCorner(self.normals);
        let normals = match per_vertex(&mesh, &corners) {
            Some(per_point) => Attribute::PerVertex(per_point),
            None => corners,
        };
        mesh.clone().with_normals(normals).unwrap_or(mesh)
    }
}

/// Per-corner normals as one per point, when each point's corners agree.
fn per_vertex(mesh: &Mesh, corners: &Attribute<Point>) -> Option<Vec<Point>> {
    let Attribute::PerCorner(values) = corners else {
        return None;
    };
    let mut out: Vec<Option<Point>> = vec![None; mesh.vertex_count()];
    for (c, &v) in mesh.corners().iter().enumerate() {
        match out[v as usize] {
            None => out[v as usize] = Some(values[c]),
            Some(seen) if seen == values[c] => {}
            Some(_) => return None,
        }
    }
    Some(out.into_iter().map(Option::unwrap_or_default).collect())
}

fn unit(v: Point) -> Point {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 { v.map(|c| c / l) } else { v }
}

fn neg(v: Point) -> Point {
    v.map(|c| -c)
}

/// A grid of quads on one flat side, `nu` by `nv`, from `corner` along `u` and `v` (its outside
/// is `u` × `v`); points on the side's edges are shared with the sides next to it through `at`.
fn grid(
    b: &mut Builder,
    at: &mut HashMap<[i64; 3], u32>,
    place: impl Fn(u32, u32) -> ([i64; 3], Point),
    (nu, nv): (u32, u32),
    normal: Point,
) {
    let mut index = |i: u32, j: u32, b: &mut Builder| {
        let (key, p) = place(i, j);
        *at.entry(key).or_insert_with(|| b.point(p))
    };
    for j in 0..nv {
        for i in 0..nu {
            let q = [
                index(i, j, b),
                index(i + 1, j, b),
                index(i + 1, j + 1, b),
                index(i, j + 1, b),
            ];
            b.face(&q, &[normal; 4]);
        }
    }
}

impl Mesh {
    /// A box from the frame's origin, `size` along its three directions, with `counts` faces
    /// along each (at least 1), as Rhino's MeshBox. Its sides are flat-shaded: each corner has its
    /// side's normal.
    #[must_use]
    pub fn mesh_box(frame: &Frame, size: [f64; 3], counts: [u32; 3]) -> Self {
        let n = counts.map(|c| c.max(1));
        let mut b = Builder::new();
        // Points are keyed by their grid numbers along the three directions, so sides share edges.
        let mut at: HashMap<[i64; 3], u32> = HashMap::new();
        let point = |g: [u32; 3]| {
            let f = |k: usize| size[k] * f64::from(g[k]) / f64::from(n[k]);
            (g.map(i64::from), frame.at(f(0), f(1), f(2)))
        };
        let [nx, ny, nz] = n;
        let (x, y, z) = (frame.x, frame.y, frame.z);
        // Each side as (fixed axis and value, the two running axes in outward order, normal).
        let sides: [(usize, u32, usize, usize, Point); 6] = [
            (2, 0, 1, 0, neg(z)),
            (2, nz, 0, 1, z),
            (1, 0, 0, 2, neg(y)),
            (1, ny, 2, 0, y),
            (0, 0, 2, 1, neg(x)),
            (0, nx, 1, 2, x),
        ];
        for (fixed, value, u, v, normal) in sides {
            let place = |i: u32, j: u32| {
                let mut g = [0u32; 3];
                g[fixed] = value;
                g[u] = i;
                g[v] = j;
                point(g)
            };
            grid(&mut b, &mut at, place, (n[u], n[v]), normal);
        }
        outward(b.mesh(), size[0] * size[1] * size[2])
    }

    /// A flat rectangle from the frame's origin, `size` along its first two directions, with
    /// `counts` faces along each (at least 1), facing the third direction, as Rhino's MeshPlane.
    #[must_use]
    pub fn mesh_plane(frame: &Frame, size: [f64; 2], counts: [u32; 2]) -> Self {
        let [nx, ny] = counts.map(|c| c.max(1));
        let mut b = Builder::new();
        let mut at = HashMap::new();
        let place = |i: u32, j: u32| {
            let (a, c) = (
                size[0] * f64::from(i) / f64::from(nx),
                size[1] * f64::from(j) / f64::from(ny),
            );
            ([i64::from(i), i64::from(j), 0], frame.at(a, c, 0.0))
        };
        grid(&mut b, &mut at, place, (nx, ny), frame.z);
        outward(b.mesh(), size[0] * size[1])
    }

    /// A sphere about the frame's origin, as Rhino's MeshSphere in each of its styles, with a
    /// normal at each point pointing straight out.
    #[must_use]
    pub fn mesh_sphere(frame: &Frame, radius: f64, style: SphereStyle) -> Self {
        let directions_and_faces = match style {
            SphereStyle::Uv { vertical, around } => uv_sphere(vertical.max(2), around.max(3)),
            SphereStyle::Quads { subdivisions } => quad_sphere(subdivisions.min(6)),
            SphereStyle::Triangles { subdivisions } => ico_sphere(subdivisions.min(5)),
        };
        let (directions, faces) = directions_and_faces;
        let mut b = Builder::new();
        for d in &directions {
            let p = frame.at(radius * d[0], radius * d[1], radius * d[2]);
            b.point(p);
        }
        for f in &faces {
            let normals: Vec<Point> = f
                .iter()
                .map(|&v| {
                    let d = directions[v as usize];
                    frame.direction(d[0], d[1], d[2])
                })
                .collect();
            b.face(f, &normals);
        }
        outward(b.mesh(), radius)
    }

    /// A cylinder standing on the frame's origin along its third direction, as Rhino's
    /// MeshCylinder: `vertical` faces up its side (at least 1) and `around` it (at least 3),
    /// closed at both ends when `solid`, the ends filled with quads when `quad_caps` and `around`
    /// is even, with triangles otherwise. The side is smooth-shaded and the ends flat.
    #[must_use]
    pub fn mesh_cylinder(
        frame: &Frame,
        radius: f64,
        height: f64,
        (vertical, around): (u32, u32),
        solid: bool,
        quad_caps: bool,
    ) -> Self {
        let (nv, na) = (vertical.max(1), around.max(3));
        let mut b = Builder::new();
        let ring = |b: &mut Builder, z: f64| -> Vec<u32> {
            (0..na)
                .map(|k| {
                    let a = TAU * f64::from(k) / f64::from(na);
                    b.point(frame.at(radius * a.cos(), radius * a.sin(), z))
                })
                .collect()
        };
        let rings: Vec<Vec<u32>> = (0..=nv)
            .map(|j| ring(&mut b, height * f64::from(j) / f64::from(nv)))
            .collect();
        side(&mut b, frame, &rings, na, |a| [a.cos(), a.sin(), 0.0]);
        if solid {
            let quads = quad_caps && na % 2 == 0;
            let bottom = b.point(frame.origin);
            cap(&mut b, bottom, &rings[0], neg(frame.z), true, quads);
            let top = b.point(frame.at(0.0, 0.0, height));
            cap(&mut b, top, &rings[nv as usize], frame.z, false, quads);
        }
        outward(b.mesh(), radius * height)
    }

    /// A cone standing on the frame's origin with its point `height` along the third direction,
    /// as Rhino's MeshCone: `vertical` faces from the base to the point (at least 1) and `around`
    /// it (at least 3), closed at the base when `solid`, with quads there when `quad_caps` and
    /// `around` is even. The side is smooth-shaded and the base flat.
    #[must_use]
    pub fn mesh_cone(
        frame: &Frame,
        radius: f64,
        height: f64,
        (vertical, around): (u32, u32),
        solid: bool,
        quad_caps: bool,
    ) -> Self {
        let (nv, na) = (vertical.max(1), around.max(3));
        let mut b = Builder::new();
        // Rings from the base up, shrinking to the point; the point is one vertex.
        let rings: Vec<Vec<u32>> = (0..nv)
            .map(|j| {
                let t = f64::from(j) / f64::from(nv);
                (0..na)
                    .map(|k| {
                        let a = TAU * f64::from(k) / f64::from(na);
                        let r = radius * (1.0 - t);
                        b.point(frame.at(r * a.cos(), r * a.sin(), height * t))
                    })
                    .collect()
            })
            .collect();
        // The side's normal leans up by the cone's half-angle.
        let lean = |a: f64| [height * a.cos(), height * a.sin(), radius];
        side(&mut b, frame, &rings, na, lean);
        let apex = b.point(frame.at(0.0, 0.0, height));
        let top = &rings[nv as usize - 1];
        for k in 0..na as usize {
            let next = (k + 1) % na as usize;
            let a = |k: usize| TAU * k as f64 / f64::from(na);
            let middle = TAU * (k as f64 + 0.5) / f64::from(na);
            let n = |a: f64| {
                let l = lean(a);
                unit(frame.direction(l[0], l[1], l[2]))
            };
            b.face(
                &[top[k], top[next], apex],
                &[n(a(k)), n(a(next)), n(middle)],
            );
        }
        if solid {
            let centre = b.point(frame.origin);
            cap(
                &mut b,
                centre,
                &rings[0],
                neg(frame.z),
                true,
                quad_caps && na % 2 == 0,
            );
        }
        outward(b.mesh(), radius * height)
    }

    /// A torus about the frame's origin, its ring in the frame's first two directions: `major` is
    /// the ring's radius and `minor` the tube's, with `vertical` faces round the tube and `around`
    /// faces round the ring (each at least 3), as Rhino's MeshTorus.
    #[must_use]
    pub fn mesh_torus(
        frame: &Frame,
        major: f64,
        minor: f64,
        (vertical, around): (u32, u32),
    ) -> Self {
        let (nv, na) = (vertical.max(3), around.max(3));
        let mut b = Builder::new();
        let mut normal_at = Vec::new();
        for k in 0..na {
            let a = TAU * f64::from(k) / f64::from(na);
            for j in 0..nv {
                let t = TAU * f64::from(j) / f64::from(nv);
                let (r, z) = (major + minor * t.cos(), minor * t.sin());
                b.point(frame.at(r * a.cos(), r * a.sin(), z));
                normal_at.push(frame.direction(t.cos() * a.cos(), t.cos() * a.sin(), t.sin()));
            }
        }
        let at = |k: u32, j: u32| (k % na) * nv + j % nv;
        for k in 0..na {
            for j in 0..nv {
                let q = [at(k, j), at(k + 1, j), at(k + 1, j + 1), at(k, j + 1)];
                let normals = q.map(|v| normal_at[v as usize]);
                b.face(&q, &normals);
            }
        }
        outward(b.mesh(), major * minor)
    }
}

/// Quads between successive rings of a cylinder or cone, smooth-shaded with `normal(angle)` (in
/// the frame's terms).
fn side(
    b: &mut Builder,
    frame: &Frame,
    rings: &[Vec<u32>],
    na: u32,
    normal: impl Fn(f64) -> Point,
) {
    let n = |k: usize| {
        let d = normal(TAU * k as f64 / f64::from(na));
        unit(frame.direction(d[0], d[1], d[2]))
    };
    for pair in rings.windows(2) {
        let (low, high) = (&pair[0], &pair[1]);
        for k in 0..na as usize {
            let next = (k + 1) % na as usize;
            b.face(
                &[low[k], low[next], high[next], high[k]],
                &[n(k), n(next), n(next), n(k)],
            );
        }
    }
}

/// An end filled from its centre: a fan of triangles, or of quads taking two edges each.
fn cap(b: &mut Builder, centre: u32, ring: &[u32], normal: Point, bottom: bool, quads: bool) {
    let n = ring.len();
    let step = if quads { 2 } else { 1 };
    for k in (0..n).step_by(step) {
        let mut face = vec![centre];
        face.extend((0..=step).map(|s| ring[(k + s) % n]));
        if bottom {
            face[1..].reverse();
        }
        let normals = vec![normal; face.len()];
        b.face(&face, &normals);
    }
}

/// The mesh turned to face out when a size given was negative (a mirrored frame turns it too).
fn outward(mesh: Mesh, sign: f64) -> Mesh {
    if sign < 0.0 { mesh.flipped() } else { mesh }
}

/// A UV sphere's unit directions and faces: the south pole, rings from south to north each
/// starting on the first direction and running towards the second, then the north pole, as
/// Rhino orders them.
fn uv_sphere(vertical: u32, around: u32) -> (Vec<Point>, Vec<Vec<u32>>) {
    let mut d = vec![[0.0, 0.0, -1.0]];
    for r in 1..vertical {
        let phi =
            std::f64::consts::PI * f64::from(r) / f64::from(vertical) - std::f64::consts::FRAC_PI_2;
        for k in 0..around {
            let theta = TAU * f64::from(k) / f64::from(around);
            d.push([phi.cos() * theta.cos(), phi.cos() * theta.sin(), phi.sin()]);
        }
    }
    d.push([0.0, 0.0, 1.0]);
    let north = u32::try_from(d.len() - 1).unwrap_or(u32::MAX);
    let at = |r: u32, k: u32| 1 + (r - 1) * around + k % around;
    let mut faces = Vec::new();
    for k in 0..around {
        faces.push(vec![0, at(1, k + 1), at(1, k)]);
    }
    for r in 1..vertical - 1 {
        for k in 0..around {
            faces.push(vec![at(r, k), at(r, k + 1), at(r + 1, k + 1), at(r + 1, k)]);
        }
    }
    for k in 0..around {
        faces.push(vec![at(vertical - 1, k), at(vertical - 1, k + 1), north]);
    }
    (d, faces)
}

/// Splits each face in four `times` times, pushing new points out to the unit sphere: a
/// triangle by its edges' midpoints, a quad by those and its centre.
fn subdivide(
    mut d: Vec<Point>,
    mut faces: Vec<Vec<u32>>,
    times: u32,
) -> (Vec<Point>, Vec<Vec<u32>>) {
    for _ in 0..times {
        let mut middles: HashMap<(u32, u32), u32> = HashMap::new();
        let mut middle = |a: u32, b: u32, d: &mut Vec<Point>| {
            *middles.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (p, q) = (d[a as usize], d[b as usize]);
                d.push(unit(std::array::from_fn(|i| p[i] + q[i])));
                u32::try_from(d.len() - 1).unwrap_or(u32::MAX)
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for f in &faces {
            let m: Vec<u32> = (0..f.len())
                .map(|k| middle(f[k], f[(k + 1) % f.len()], &mut d))
                .collect();
            if f.len() == 3 {
                next.push(vec![f[0], m[0], m[2]]);
                next.push(vec![m[0], f[1], m[1]]);
                next.push(vec![m[2], m[1], f[2]]);
                next.push(vec![m[0], m[1], m[2]]);
            } else {
                let sum = f.iter().fold([0.0; 3], |s, &v| {
                    let p = d[v as usize];
                    [s[0] + p[0], s[1] + p[1], s[2] + p[2]]
                });
                d.push(unit(sum));
                let c = u32::try_from(d.len() - 1).unwrap_or(u32::MAX);
                for k in 0..4 {
                    next.push(vec![f[k], m[k], c, m[(k + 3) % 4]]);
                }
            }
        }
        faces = next;
    }
    (d, faces)
}

/// A cube on the unit sphere, its six squares split into 2^`times` by 2^`times`.
fn quad_sphere(times: u32) -> (Vec<Point>, Vec<Vec<u32>>) {
    // Each square becomes a grid of 2^times by 2^times at equal angles seen from the centre, then
    // pushed out to the sphere, the most even of the usual layouts. Rhino's own layout is none of
    // the usual ones (its points sit between an even grid's and this one's); sizes agree to 0.3%.
    let n: i64 = 1 << times;
    let corner = |i: u32| -> [i64; 3] { [1, 2, 4].map(|bit| if i & bit == 0 { -n } else { n }) };
    let faces = vec![
        vec![0, 2, 3, 1],
        vec![4, 5, 7, 6],
        vec![0, 1, 5, 4],
        vec![2, 6, 7, 3],
        vec![0, 4, 6, 2],
        vec![1, 3, 7, 5],
    ];
    let mut d: Vec<Point> = Vec::new();
    let mut at: HashMap<[i64; 3], u32> = HashMap::new();
    let mut out = Vec::new();
    for f in &faces {
        let [a, b, _, e] = [f[0], f[1], f[2], f[3]].map(corner);
        let mut point = |i: i64, j: i64| -> u32 {
            // Grid steps of 2 between corners 2n apart, so every point is a whole number.
            let key: [i64; 3] =
                std::array::from_fn(|k| a[k] + (b[k] - a[k]) / n * i + (e[k] - a[k]) / n * j);
            *at.entry(key).or_insert_with(|| {
                #[allow(clippy::cast_precision_loss)]
                let angle = |c: i64| (std::f64::consts::FRAC_PI_4 * c as f64 / n as f64).tan();
                d.push(unit(key.map(angle)));
                u32::try_from(d.len() - 1).unwrap_or(u32::MAX)
            })
        };
        for j in 0..n {
            for i in 0..n {
                out.push(vec![
                    point(i, j),
                    point(i + 1, j),
                    point(i + 1, j + 1),
                    point(i, j + 1),
                ]);
            }
        }
    }
    (d, out)
}

/// An icosahedron on the unit sphere, its twenty triangles split `times` times.
fn ico_sphere(times: u32) -> (Vec<Point>, Vec<Vec<u32>>) {
    let g = f64::midpoint(1.0, 5.0_f64.sqrt());
    let raw = [
        [-1.0, g, 0.0],
        [1.0, g, 0.0],
        [-1.0, -g, 0.0],
        [1.0, -g, 0.0],
        [0.0, -1.0, g],
        [0.0, 1.0, g],
        [0.0, -1.0, -g],
        [0.0, 1.0, -g],
        [g, 0.0, -1.0],
        [g, 0.0, 1.0],
        [-g, 0.0, -1.0],
        [-g, 0.0, 1.0],
    ];
    let d: Vec<Point> = raw.iter().map(|&p| unit(p)).collect();
    let faces: Vec<Vec<u32>> = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ]
    .iter()
    .map(|f| f.to_vec())
    .collect();
    subdivide(d, faces, times)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rhino 8's numbers for the same shapes (RhinoCommon's `Mesh.CreateFrom…`, 17 November
    // 2026). Rhino keeps hard edges as separate points at one place; Curva keeps one point with a
    // normal per corner, so Curva's point counts are Rhino's topology vertex counts.

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 5e-7
    }

    fn quads(m: &Mesh) -> usize {
        m.faces().filter(|f| f.len() == 4).count()
    }

    fn assert_good(m: &Mesh) {
        let report = m.check(1e-9);
        assert!(report.is_good(), "{report:?}");
        assert!(m.is_closed());
    }

    #[test]
    fn a_box_matches_rhinos() {
        let m = Mesh::mesh_box(&Frame::world(), [2.0, 3.0, 4.0], [2, 3, 4]);
        assert_eq!((m.vertex_count(), m.face_count(), quads(&m)), (54, 52, 52));
        assert_eq!(m.volume(), Some(24.0));
        assert!(close(m.area(), 52.0));
        assert_good(&m);
        let Attribute::PerCorner(n) = m.normals() else {
            panic!("flat sides: a normal per corner")
        };
        assert_eq!(n[0], [0.0, 0.0, -1.0]);
        let one = Mesh::mesh_box(&Frame::world(), [2.0, 3.0, 4.0], [0, 1, 1]);
        assert_eq!(
            (one.vertex_count(), one.face_count()),
            (8, 6),
            "counts raised to 1"
        );
    }

    #[test]
    fn a_plane_matches_rhinos() {
        let m = Mesh::mesh_plane(&Frame::world(), [4.0, 3.0], [4, 3]);
        assert_eq!((m.vertex_count(), m.face_count()), (20, 12));
        assert!(close(m.area(), 12.0));
        assert_eq!(
            m.normals(),
            &Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 20])
        );
        assert!(m.faces().all(|f| {
            let p: Vec<Point> = f.iter().map(|&v| m.vertices()[v as usize]).collect();
            crate::triangulate::newell_normal(&p)[2] > 0.0
        }));
    }

    #[test]
    fn the_three_sphere_styles_match_rhinos() {
        let uv = Mesh::mesh_sphere(
            &Frame::world(),
            1.0,
            SphereStyle::Uv {
                vertical: 10,
                around: 12,
            },
        );
        assert_eq!(
            (uv.vertex_count(), uv.face_count(), quads(&uv)),
            (110, 120, 96)
        );
        assert!(close(uv.volume().unwrap(), 3.902113), "{:?}", uv.volume());
        assert!(close(uv.area(), 12.130779));
        assert_good(&uv);
        let q = Mesh::mesh_sphere(&Frame::world(), 1.0, SphereStyle::Quads { subdivisions: 2 });
        assert_eq!((q.vertex_count(), q.face_count(), quads(&q)), (98, 96, 96));
        // Rhino places its points its own way: within 0.3% of its 3.923928 and 12.154314.
        assert!(
            (q.volume().unwrap() / 3.923928 - 1.0).abs() < 0.003,
            "{:?}",
            q.volume()
        );
        assert!((q.area() / 12.154314 - 1.0).abs() < 0.003, "{}", q.area());
        assert_good(&q);
        let t = Mesh::mesh_sphere(
            &Frame::world(),
            1.0,
            SphereStyle::Triangles { subdivisions: 1 },
        );
        assert_eq!((t.vertex_count(), t.face_count()), (42, 80));
        assert!(close(t.volume().unwrap(), 3.658712), "{:?}", t.volume());
        assert!(close(t.area(), 11.665931));
        assert_good(&t);
        assert!(
            matches!(uv.normals(), Attribute::PerVertex(_)),
            "smooth: one normal per point"
        );
    }

    #[test]
    fn cylinders_match_rhinos_with_each_cap_style() {
        let tri = Mesh::mesh_cylinder(&Frame::world(), 1.0, 2.0, (3, 8), true, false);
        assert_eq!(
            (tri.vertex_count(), tri.face_count(), quads(&tri)),
            (34, 40, 24)
        );
        assert!(close(tri.area(), 17.902724));
        // The octagonal prism holds 2√2 × 2. Rhino's own reports 3.771236: one of its caps faces
        // in (Rhino calls it not oriented); Curva's face out.
        assert!(close(tri.volume().unwrap(), 4.0 * 2f64.sqrt()));
        assert_good(&tri);
        let quad = Mesh::mesh_cylinder(&Frame::world(), 1.0, 2.0, (3, 8), true, true);
        assert_eq!((quad.face_count(), quads(&quad)), (32, 32));
        assert_good(&quad);
        let odd = Mesh::mesh_cylinder(&Frame::world(), 1.0, 2.0, (3, 7), true, true);
        assert_eq!(
            odd.face_count(),
            35,
            "an odd count always gets triangles, as Rhino"
        );
        let open = Mesh::mesh_cylinder(&Frame::world(), 1.0, 2.0, (3, 8), false, false);
        assert_eq!((open.vertex_count(), open.face_count()), (32, 24));
        assert!(close(open.area(), 12.24587));
        assert!(!open.is_closed());
    }

    #[test]
    fn cones_match_rhinos() {
        let tri = Mesh::mesh_cone(&Frame::world(), 1.0, 2.0, (3, 8), true, false);
        assert_eq!(
            (tri.vertex_count(), tri.face_count(), quads(&tri)),
            (26, 32, 16)
        );
        assert!(close(tri.volume().unwrap(), 1.885618));
        assert!(close(tri.area(), 9.573079));
        assert_good(&tri);
        let quad = Mesh::mesh_cone(&Frame::world(), 1.0, 2.0, (3, 8), true, true);
        assert_eq!((quad.face_count(), quads(&quad)), (28, 20));
        assert_good(&quad);
        let open = Mesh::mesh_cone(&Frame::world(), 1.0, 2.0, (3, 8), false, false);
        assert_eq!((open.vertex_count(), open.face_count()), (25, 24));
        assert!(close(open.area(), 6.744652));
    }

    #[test]
    fn a_torus_matches_rhinos() {
        let m = Mesh::mesh_torus(&Frame::world(), 3.0, 1.0, (6, 8));
        assert_eq!((m.vertex_count(), m.face_count(), quads(&m)), (48, 48, 48));
        assert!(close(m.volume().unwrap(), 44.090815));
        assert!(close(m.area(), 106.058778));
        assert_good(&m);
    }

    #[test]
    fn shapes_on_a_tilted_frame_far_away_keep_their_sizes() {
        let s = 0.5_f64.sqrt();
        let frame = Frame {
            origin: [1.0e6, -2.0e6, 300.0],
            x: [s, s, 0.0],
            y: [-s, s, 0.0],
            z: [0.0, 0.0, 1.0],
        };
        let b = Mesh::mesh_box(&frame, [2.0, 3.0, 4.0], [2, 3, 4]);
        assert!((b.volume().unwrap() - 24.0).abs() < 1e-6);
        let t = Mesh::mesh_torus(&frame, 3.0, 1.0, (6, 8));
        assert!((t.volume().unwrap() - 44.090815).abs() < 1e-5);
        assert_good(&t);
    }

    #[test]
    fn negative_sizes_still_face_out() {
        let b = Mesh::mesh_box(&Frame::world(), [-2.0, 3.0, 4.0], [1, 1, 1]);
        assert_eq!(b.volume(), Some(24.0));
    }
}
