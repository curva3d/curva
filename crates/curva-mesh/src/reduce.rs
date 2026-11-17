//! ReduceMesh: fewer triangles keeping the shape, by quadric error edge collapse (Garland and
//! Heckbert, "Surface Simplification Using Quadric Error Metrics", SIGGRAPH 1997).
//!
//! Each point carries a quadric: the sum of the squared distances to the planes of the triangles
//! around it. The edge whose two ends can merge into one point with the least added distance is
//! collapsed first, the merged point placed where that distance is least (§4 and §5 of the
//! paper). Open edges get an extra plane at right angles to their face, heavily weighted, so a
//! border keeps its outline (§6.2). A collapse that would fold a triangle over, or join two parts
//! of the mesh that only touch, is skipped.
//!
//! The result is triangles. Normals are worked out again when the mesh had them; texture
//! coordinates and colours do not come through yet.

use crate::check::distance;
use crate::triangulate::newell_normal;
use crate::{Mesh, Point};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// A symmetric 4×4 matrix, upper triangle by rows: a00 a01 a02 a03 a11 a12 a13 a22 a23 a33.
type Quadric = [f64; 10];

/// How much more an open edge's plane counts than a face's, as Garland and Heckbert suggest.
const BORDER_WEIGHT: f64 = 1000.0;

impl Mesh {
    /// The mesh with about `target` triangles, as Rhino's ReduceMesh, and how many faces went.
    /// A mesh with no more than `target` triangles comes back as triangles, unchanged in shape. It
    /// may stop above `target` when no more edges can go without spoiling the mesh.
    // CUSTOM MESH OPERATION: reduce-mesh
    #[must_use]
    pub fn reduced(&self, target: usize) -> (Self, usize) {
        let mut work = Work::of(self);
        work.collapse_to(target);
        let reduced = work.mesh();
        let reduced = if self.normals().is_none() {
            reduced
        } else {
            reduced.with_rebuilt_normals()
        };
        let removed = self.face_count().saturating_sub(reduced.face_count());
        (reduced, removed)
    }
}

/// The mesh being reduced: points by place, triangles, and which triangles meet at each point.
struct Work {
    points: Vec<Point>,
    alive: Vec<bool>,
    version: Vec<u32>,
    locked: Vec<bool>,
    quadrics: Vec<Quadric>,
    triangles: Vec<[usize; 3]>,
    live: Vec<bool>,
    around: Vec<Vec<usize>>,
    count: usize,
}

/// An edge waiting to collapse, cheapest first.
struct Candidate {
    cost: f64,
    ends: (usize, usize),
    versions: (u32, u32),
    to: Point,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Candidate {
    // Reversed, so the heap gives the cheapest first; ties go to the lower-numbered edge, so the
    // result never depends on anything but the mesh.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.ends.cmp(&self.ends))
    }
}

impl Work {
    fn of(mesh: &Mesh) -> Self {
        let place = mesh.places();
        let mut number = vec![usize::MAX; mesh.vertex_count()];
        let mut points = Vec::new();
        for (v, &p) in place.iter().enumerate() {
            if number[p as usize] == usize::MAX {
                number[p as usize] = points.len();
                points.push(mesh.vertices()[v]);
            }
            number[v] = number[p as usize];
        }
        let triangles: Vec<[usize; 3]> = mesh
            .triangles()
            .triangles
            .iter()
            .map(|t| t.map(|v| number[v as usize]))
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .collect();
        let n = points.len();
        let mut around = vec![Vec::new(); n];
        for (i, t) in triangles.iter().enumerate() {
            for &v in t {
                around[v].push(i);
            }
        }
        let mut work = Self {
            alive: vec![true; n],
            version: vec![0; n],
            locked: vec![false; n],
            quadrics: vec![[0.0; 10]; n],
            live: vec![true; triangles.len()],
            count: triangles.len(),
            points,
            triangles,
            around,
        };
        work.start_quadrics();
        work
    }

    fn start_quadrics(&mut self) {
        let mut edges: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (i, t) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        for t in &self.triangles {
            let corners = t.map(|v| self.points[v]);
            let n = newell_normal(&corners);
            let area = distance(n, [0.0; 3]) / 2.0;
            if area > 0.0 {
                let q = plane(unit(n), corners[0], area);
                for &v in t {
                    add(&mut self.quadrics[v], &q);
                }
            }
        }
        for (&(a, b), faces) in &edges {
            match faces.len() {
                1 => {
                    let t = self.triangles[faces[0]];
                    let n = unit(newell_normal(&t.map(|v| self.points[v])));
                    let (pa, pb) = (self.points[a], self.points[b]);
                    let along = sub(pb, pa);
                    let side = unit(cross(along, n));
                    let length2 = dot(along, along);
                    let q = plane(side, pa, BORDER_WEIGHT * length2);
                    add(&mut self.quadrics[a], &q);
                    add(&mut self.quadrics[b], &q);
                }
                2 => {}
                // Edges with three or more faces stay put: no collapse can keep them right.
                _ => {
                    self.locked[a] = true;
                    self.locked[b] = true;
                }
            }
        }
    }

    fn collapse_to(&mut self, target: usize) {
        let mut heap = BinaryHeap::new();
        for t in 0..self.triangles.len() {
            for k in 0..3 {
                let (a, b) = (self.triangles[t][k], self.triangles[t][(k + 1) % 3]);
                if a < b {
                    self.push(&mut heap, a, b);
                } else if !self.has_edge_one_way(b, a) {
                    // An open edge seen only this way round.
                    self.push(&mut heap, b, a);
                }
            }
        }
        while self.count > target {
            let Some(c) = heap.pop() else { break };
            let (a, b) = c.ends;
            if !self.alive[a]
                || !self.alive[b]
                || self.version[a] != c.versions.0
                || self.version[b] != c.versions.1
            {
                continue;
            }
            if !self.can_collapse(a, b, c.to) {
                continue;
            }
            self.collapse(a, b, c.to);
            for w in self.neighbours(a) {
                self.push(&mut heap, a.min(w), a.max(w));
            }
        }
    }

    /// Whether some live triangle runs a→b.
    fn has_edge_one_way(&self, a: usize, b: usize) -> bool {
        self.around[a].iter().any(|&t| {
            let tri = self.triangles[t];
            (0..3).any(|k| tri[k] == a && tri[(k + 1) % 3] == b)
        })
    }

    fn push(&self, heap: &mut BinaryHeap<Candidate>, a: usize, b: usize) {
        if self.locked[a] || self.locked[b] {
            return;
        }
        let mut q = self.quadrics[a];
        add(&mut q, &self.quadrics[b]);
        let (pa, pb) = (self.points[a], self.points[b]);
        let middle = [0, 1, 2].map(|i| (pa[i] + pb[i]) / 2.0);
        // The best point solves the quadric's 3×3 system (paper §4); where that is singular (a
        // flat or straight neighbourhood), the best of the ends and the middle.
        let to = solve(&q, pa, pb).unwrap_or_else(|| {
            [pa, pb, middle]
                .into_iter()
                .min_by(|x, y| error(&q, *x).total_cmp(&error(&q, *y)))
                .unwrap_or(middle)
        });
        heap.push(Candidate {
            cost: error(&q, to),
            ends: (a, b),
            versions: (self.version[a], self.version[b]),
            to,
        });
    }

    fn neighbours(&self, v: usize) -> Vec<usize> {
        let mut out: Vec<usize> = self.around[v]
            .iter()
            .flat_map(|&t| self.triangles[t])
            .filter(|&w| w != v)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Whether a and b can merge at `to` without spoiling the mesh: the two may share only the
    /// points opposite their shared edge (else two parts that only touch would join), and no
    /// triangle around them may fold over or vanish.
    fn can_collapse(&self, a: usize, b: usize, to: Point) -> bool {
        let shared: Vec<usize> = self.around[a]
            .iter()
            .copied()
            .filter(|t| self.triangles[*t].contains(&b))
            .collect();
        if shared.is_empty() {
            return false;
        }
        let opposite: Vec<usize> = shared
            .iter()
            .flat_map(|&t| self.triangles[t])
            .filter(|&w| w != a && w != b)
            .collect();
        let na = self.neighbours(a);
        let common = self
            .neighbours(b)
            .into_iter()
            .filter(|w| *w != a && na.contains(w))
            .count();
        if common != opposite.len() {
            return false;
        }
        // Two border points joined across the inside would pinch the mesh.
        if shared.len() == 2 && self.on_border(a) && self.on_border(b) {
            return false;
        }
        for (v, other) in [(a, b), (b, a)] {
            for &t in &self.around[v] {
                let tri = self.triangles[t];
                if tri.contains(&other) {
                    continue;
                }
                let before = newell_normal(&tri.map(|w| self.points[w]));
                let after = newell_normal(&tri.map(|w| if w == v { to } else { self.points[w] }));
                let size = distance(before, [0.0; 3]);
                if dot(before, after) <= 0.0 || distance(after, [0.0; 3]) < 1e-12 * size {
                    return false;
                }
            }
        }
        true
    }

    fn on_border(&self, v: usize) -> bool {
        self.neighbours(v).into_iter().any(|w| {
            self.around[v]
                .iter()
                .filter(|&&t| self.triangles[t].contains(&w))
                .count()
                == 1
        })
    }

    fn collapse(&mut self, a: usize, b: usize, to: Point) {
        self.points[a] = to;
        let qb = self.quadrics[b];
        add(&mut self.quadrics[a], &qb);
        let moved = std::mem::take(&mut self.around[b]);
        for t in moved {
            if !self.live[t] {
                continue;
            }
            if self.triangles[t].contains(&a) {
                self.live[t] = false;
                self.count -= 1;
            } else {
                for v in &mut self.triangles[t] {
                    if *v == b {
                        *v = a;
                    }
                }
                self.around[a].push(t);
            }
        }
        let live = &self.live;
        self.around[a].retain(|&t| live[t]);
        for w in self.neighbours(a) {
            let live = &self.live;
            self.around[w].retain(|&t| live[t]);
        }
        self.alive[b] = false;
        self.version[a] += 1;
    }

    fn mesh(&self) -> Mesh {
        let mut number = vec![u32::MAX; self.points.len()];
        let mut points = Vec::new();
        let mut faces = Vec::with_capacity(self.count);
        for (t, tri) in self.triangles.iter().enumerate() {
            if !self.live[t] {
                continue;
            }
            faces.push(tri.map(|v| {
                if number[v] == u32::MAX {
                    number[v] = u32::try_from(points.len()).unwrap_or(u32::MAX);
                    points.push(self.points[v]);
                }
                number[v]
            }));
        }
        // Every triangle has three different points, all finite.
        Mesh::new(points, &faces).unwrap_or_default()
    }
}

/// The quadric of the plane through `at` facing the unit direction `n`, times `weight`.
fn plane(n: Point, at: Point, weight: f64) -> Quadric {
    let d = -dot(n, at);
    let p = [n[0], n[1], n[2], d];
    let mut q = [0.0; 10];
    let mut k = 0;
    for i in 0..4 {
        for j in i..4 {
            q[k] = weight * p[i] * p[j];
            k += 1;
        }
    }
    q
}

fn add(q: &mut Quadric, r: &Quadric) {
    for (a, b) in q.iter_mut().zip(r) {
        *a += b;
    }
}

/// The quadric's value at a point: vᵀQv with v = (x, y, z, 1).
fn error(q: &Quadric, p: Point) -> f64 {
    let [x, y, z] = p;
    q[0] * x * x
        + 2.0 * q[1] * x * y
        + 2.0 * q[2] * x * z
        + 2.0 * q[3] * x
        + q[4] * y * y
        + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

/// The point where the quadric is least, by Cramer's rule, if the system is well conditioned
/// against the size of the edge it serves.
fn solve(q: &Quadric, pa: Point, pb: Point) -> Option<Point> {
    let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
    let rhs = [-q[3], -q[6], -q[8]];
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(m);
    let scale = m.iter().flatten().fold(0.0_f64, |s, v| s.max(v.abs()));
    if d.abs() <= 1e-9 * scale.powi(3) {
        return None;
    }
    let mut out = [0.0; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let mut mi = m;
        for r in 0..3 {
            mi[r][i] = rhs[r];
        }
        *o = det(mi) / d;
    }
    // A point far from the edge means a nearly flat system: fall back to the edge's own points.
    let reach = distance(pa, pb).max(f64::MIN_POSITIVE) * 10.0;
    let middle = [0, 1, 2].map(|i| (pa[i] + pb[i]) / 2.0);
    (out.iter().all(|c| c.is_finite()) && distance(out, middle) <= reach).then_some(out)
}

fn unit(v: Point) -> Point {
    let l = distance(v, [0.0; 3]);
    if l > 0.0 { v.map(|c| c / l) } else { v }
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Attribute;
    use crate::measure::tests::uv_sphere;
    use crate::mesh::tests::cube;

    /// A flat square grid of `n` × `n` quads, 10 units across.
    fn grid(n: u32) -> Mesh {
        let step = 10.0 / f64::from(n);
        let points: Vec<Point> = (0..=n)
            .flat_map(|j| (0..=n).map(move |i| [f64::from(i) * step, f64::from(j) * step, 0.0]))
            .collect();
        let at = |i: u32, j: u32| j * (n + 1) + i;
        let faces: Vec<[u32; 4]> = (0..n)
            .flat_map(|j| {
                (0..n).map(move |i| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)])
            })
            .collect();
        Mesh::new(points, &faces).unwrap()
    }

    #[test]
    fn a_sphere_reduces_to_the_target_and_keeps_its_shape() {
        let sphere = uv_sphere(32);
        let (reduced, removed) = sphere.reduced(200);
        assert!(reduced.face_count() <= 200 && reduced.face_count() >= 190);
        assert_eq!(removed, 512 - reduced.face_count());
        assert!(reduced.check(1e-6).is_good(), "{:?}", reduced.check(1e-6));
        assert!(reduced.is_closed());
        // Rhino's ReduceMesh of a comparable sphere to 200 faces kept 97.5% of the volume.
        let kept = reduced.volume().unwrap() / sphere.volume().unwrap();
        assert!(kept > 0.95 && kept <= 1.0 + 1e-9, "kept {kept}");
        for p in reduced.vertices() {
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            // The quadric puts a merged point between the planes it replaces, a little outside
            // a curved surface as often as inside.
            assert!(r > 0.95 && r < 1.02, "point off the sphere: {r}");
        }
    }

    #[test]
    fn a_flat_grid_stays_flat_and_keeps_its_outline() {
        let (reduced, _) = grid(20).reduced(50);
        assert!(reduced.face_count() <= 50);
        assert!(reduced.vertices().iter().all(|p| p[2] == 0.0));
        assert_eq!(reduced.bounds(), Some(([0.0; 3], [10.0, 10.0, 0.0])));
        assert!(
            (reduced.area() - 100.0).abs() < 1e-9,
            "area {}",
            reduced.area()
        );
        assert!(reduced.check(1e-6).is_good());
    }

    #[test]
    fn a_cube_cannot_lose_its_corners() {
        // Every edge collapse would cut a corner; the costs are positive, and still the cube
        // reduces only as far as its shape allows when asked for a few faces.
        let (reduced, _) = cube().reduced(4);
        assert!(reduced.is_closed());
        assert!(reduced.check(1e-6).is_good());
        let (same, removed) = cube().reduced(12);
        assert_eq!((same.face_count(), removed), (12, 0));
        assert_eq!(same.volume(), Some(1.0));
    }

    #[test]
    fn reducing_gives_the_same_answer_every_time() {
        let sphere = uv_sphere(24);
        assert_eq!(sphere.reduced(100), sphere.reduced(100));
    }

    #[test]
    fn normals_are_worked_out_again() {
        let sphere = uv_sphere(16).with_rebuilt_normals();
        let (reduced, _) = sphere.reduced(60);
        assert!(
            matches!(reduced.normals(), Attribute::PerVertex(n) if n.len() == reduced.vertex_count())
        );
        assert!(uv_sphere(16).reduced(60).0.normals().is_none());
    }
}
