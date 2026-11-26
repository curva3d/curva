//! Rhino's `Mesh` of an untrimmed face: a grid of quads in the face's (u, v) (#733).
//!
//! Rhino 8 meshes a face whose edges are its surface's own sides (a sphere, a plane rectangle, a
//! cylinder's side) as a grid: its spans halved, each way on its own, until every grid line stays
//! within the density's distance and angle. Quads that meet a pole, where a side shrinks to a
//! point, are triangles with a point of their own there. A sphere of radius 5 at the middle of the
//! slider is 64 quads round and 32 along, as in Rhino: 1920 quads, 128 triangles and 2143 points.

use crate::{Attribute, Mesh, Point};
use curva_kernel::NurbsSurface;

/// The most grid spans either way, whatever the tolerances ask for.
const MOST_SPANS: usize = 4096;

/// The parameter lines of the grid, along u and along v.
#[must_use]
pub fn grid_lines(surface: &NurbsSurface, chord: f64, angle: f64) -> (Vec<f64>, Vec<f64>) {
    let [[u0, u1], [v0, v1]] = surface.domain();
    let mut u = spans(&surface.u_knots, u0, u1);
    let mut v = spans(&surface.v_knots, v0, v1);
    // Each way is halved until it is fine enough across every line of the other; halving one
    // way adds lines the other must then meet, so both are checked again until neither moves.
    loop {
        let finer_u = !fits(surface, &u, &v, false, chord, angle) && u.len() <= MOST_SPANS;
        if finer_u {
            u = halved(&u);
        }
        let finer_v = !fits(surface, &v, &u, true, chord, angle) && v.len() <= MOST_SPANS;
        if finer_v {
            v = halved(&v);
        }
        if !finer_u && !finer_v {
            return (u, v);
        }
    }
}

/// The distinct knots inside `[a, b]`, with both ends: the surface's own spans.
fn spans(knots: &[f64], a: f64, b: f64) -> Vec<f64> {
    let gap = 1e-9 * (b - a).abs().max(1.0);
    let mut lines = vec![a];
    for &k in knots {
        if k > a + gap && k < b - gap && k - lines[lines.len() - 1] > gap {
            lines.push(k);
        }
    }
    lines.push(b);
    lines
}

/// Every span split in two.
fn halved(lines: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(lines.len() * 2);
    for pair in lines.windows(2) {
        out.extend([pair[0], (pair[0] + pair[1]) / 2.0]);
    }
    out.push(lines[lines.len() - 1]);
    out
}

/// The point at a grid place, `along` running u unless `across_v`.
fn at(surface: &NurbsSurface, along: f64, other: f64, across_v: bool) -> Point {
    if across_v {
        surface.point(other, along)
    } else {
        surface.point(along, other)
    }
}

/// Whether every span of `lines` stands within `chord` of the surface at its middle, and turns
/// by no more than `angle`, along each line of `others` and halfway between them.
fn fits(
    surface: &NurbsSurface,
    lines: &[f64],
    others: &[f64],
    across_v: bool,
    chord: f64,
    angle: f64,
) -> bool {
    let mut through: Vec<f64> = others.to_vec();
    through.extend(others.windows(2).map(|p| (p[0] + p[1]) / 2.0));
    through.iter().all(|&o| {
        lines.windows(2).all(|p| {
            let a = at(surface, p[0], o, across_v);
            let b = at(surface, p[1], o, across_v);
            let m = at(surface, (p[0] + p[1]) / 2.0, o, across_v);
            let mid = std::array::from_fn(|k| (a[k] + b[k]) / 2.0);
            distance(m, mid) <= chord && turn(a, m, b) <= angle
        })
    })
}

fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}

/// How far the path a → m → b turns at m, in radians: twice that is about how much the span turns.
fn turn(a: Point, m: Point, b: Point) -> f64 {
    let d1: Point = std::array::from_fn(|k| m[k] - a[k]);
    let d2: Point = std::array::from_fn(|k| b[k] - m[k]);
    let (l1, l2) = (distance(m, a), distance(b, m));
    if l1 < 1e-12 || l2 < 1e-12 {
        return 0.0;
    }
    let cos = (0..3).map(|k| d1[k] * d2[k]).sum::<f64>() / (l1 * l2);
    2.0 * cos.clamp(-1.0, 1.0).acos()
}

impl Mesh {
    /// An untrimmed face's surface as Rhino's `Mesh` makes it: a grid of quads at the distance
    /// `chord` and `angle` (radians), triangles at any pole, normals from the surface (turned
    /// round with `flip`, for a face facing against its surface).
    #[must_use]
    pub fn surface_grid(surface: &NurbsSurface, chord: f64, angle: f64, flip: bool) -> Self {
        let (u, v) = grid_lines(surface, chord, angle);
        let (nu, nv) = (u.len(), v.len());
        let point = |i: usize, j: usize| surface.point(u[i], v[j]);
        let sign = if flip { -1.0 } else { 1.0 };
        let centre = [(u[0] + u[nu - 1]) / 2.0, (v[0] + v[nv - 1]) / 2.0];
        let positions: Vec<Point> = (0..nu * nv).map(|k| point(k / nv, k % nv)).collect();
        let mut normals: Vec<Point> = (0..nu * nv)
            .map(|k| normal_near(surface, [u[k / nv], v[k % nv]], centre).map(|c| c * sign))
            .collect();
        let mut vertices = positions.clone();
        // Points closer than this are one: a pole's points differ only by rounding.
        let same = 1e-9
            * positions.iter().fold(1.0, |m: f64, p| {
                m.max(p.iter().fold(0.0, |a: f64, c| a.max(c.abs())))
            });
        let mut push = |p: Point, uv: [f64; 2], toward: [f64; 2]| -> u32 {
            vertices.push(p);
            normals.push(normal_near(surface, uv, toward).map(|c| c * sign));
            (vertices.len() - 1) as u32
        };
        let mut faces: Vec<Vec<u32>> = Vec::with_capacity((nu - 1) * (nv - 1));
        for i in 0..nu - 1 {
            for j in 0..nv - 1 {
                let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let mut face: Vec<u32> = Vec::with_capacity(4);
                let mut fresh: Vec<(usize, Point)> = Vec::new();
                for (k, &(a, b)) in corners.iter().enumerate() {
                    let index = (a * nv + b) as u32;
                    let p = positions[index as usize];
                    let next = corners[(k + 1) % 4];
                    let q = positions[next.0 * nv + next.1];
                    if distance(p, q) <= same {
                        // The side to the next corner has shrunk to a pole: the next corner
                        // stands for both.
                        continue;
                    }
                    let previous = corners[(k + 3) % 4];
                    if distance(p, positions[previous.0 * nv + previous.1]) <= same {
                        // The triangle's point at the pole, made its own as Rhino does.
                        fresh.push((face.len(), p));
                        face.push(u32::MAX);
                        continue;
                    }
                    face.push(index);
                }
                if face.len() < 3 {
                    continue;
                }
                let middle = [(u[i] + u[i + 1]) / 2.0, (v[j] + v[j + 1]) / 2.0];
                for (slot, p) in fresh {
                    let uv = surface_uv_of(&u, &v, i, j, p, &point, same);
                    face[slot] = push(p, uv, middle);
                }
                if flip {
                    face.reverse();
                }
                faces.push(face);
            }
        }
        let Ok(mesh) = Mesh::new(vertices, &faces) else {
            return Mesh::default();
        };
        // The grid points at a pole that only the triangles' own points replaced go.
        mesh.clone()
            .with_normals(Attribute::PerVertex(normals))
            .unwrap_or(mesh)
            .without_unused_vertices()
            .0
    }
}

/// The (u, v) of the cell (i, j)'s corner at `p`.
fn surface_uv_of(
    u: &[f64],
    v: &[f64],
    i: usize,
    j: usize,
    p: Point,
    point: &impl Fn(usize, usize) -> Point,
    same: f64,
) -> [f64; 2] {
    [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
        .into_iter()
        .find(|&(a, b)| distance(point(a, b), p) <= same)
        .map_or([u[i], v[j]], |(a, b)| [u[a], v[b]])
}

/// The surface's unit normal at `uv`, or, where it has none (a pole), a little way towards
/// `toward`.
fn normal_near(surface: &NurbsSurface, uv: [f64; 2], toward: [f64; 2]) -> Point {
    for step in [0.0, 1e-6, 1e-4, 1e-2] {
        let at = [0, 1].map(|k| uv[k] + (toward[k] - uv[k]) * step);
        let du = surface.iso_v(at[1]).derivative(at[0]);
        let dv = surface.iso_u(at[0]).derivative(at[1]);
        if let Some(n) = curva_kernel::unit(curva_kernel::cross(du, dv)) {
            return n;
        }
    }
    [0.0, 0.0, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rhino's NURBS sphere: degree 2, a circle of four quarter spans round, a half circle of two
    /// along, weights 1 and √½.
    fn sphere(r: f64) -> NurbsSurface {
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let ring = [
            ([1.0, 0.0], 1.0),
            ([1.0, 1.0], h),
            ([0.0, 1.0], 1.0),
            ([-1.0, 1.0], h),
            ([-1.0, 0.0], 1.0),
            ([-1.0, -1.0], h),
            ([0.0, -1.0], 1.0),
            ([1.0, -1.0], h),
            ([1.0, 0.0], 1.0),
        ];
        let arc = [
            ([0.0, -1.0], 1.0),
            ([1.0, -1.0], h),
            ([1.0, 0.0], 1.0),
            ([1.0, 1.0], h),
            ([0.0, 1.0], 1.0),
        ];
        let net: Vec<Vec<Point>> = ring
            .iter()
            .map(|&([x, y], _)| {
                arc.iter()
                    .map(|&([s, z], _)| [r * x * s, r * y * s, r * z])
                    .collect()
            })
            .collect();
        let weights: Vec<Vec<f64>> = ring
            .iter()
            .map(|&(_, a)| arc.iter().map(|&(_, b)| a * b).collect())
            .collect();
        let quarter = std::f64::consts::FRAC_PI_2;
        let u = [0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0].map(|k| k * quarter);
        let v = [-1.0, -1.0, -1.0, 0.0, 0.0, 1.0, 1.0, 1.0].map(|k| k * quarter);
        NurbsSurface::new([2, 2], u.to_vec(), v.to_vec(), &net, Some(&weights)).unwrap()
    }

    fn quads(mesh: &Mesh) -> usize {
        (0..mesh.face_count())
            .filter(|&f| mesh.face(f).len() == 4)
            .count()
    }

    #[test]
    fn a_sphere_is_gridded_as_rhinos() {
        // The testing stream's Rhino 8 sphere r5 at PolygonDensity 50 (#733): 2143 points, 2048
        // faces, 1920 of them quads and 128 triangles at the poles.
        let s = sphere(5.0);
        let (chord, angle) = crate::density_tolerances(0.5, 10.0 * 3f64.sqrt());
        let mesh = Mesh::surface_grid(&s, chord, angle, false);
        assert_eq!(
            (mesh.vertex_count(), mesh.face_count(), quads(&mesh)),
            (2143, 2048, 1920)
        );
        assert!((mesh.welded(1e-9).0.volume().unwrap() - 521.4958).abs() < 0.05);
    }

    #[test]
    fn a_flat_rectangle_is_one_quad() {
        let flat = NurbsSurface::new(
            [1, 1],
            vec![0.0, 0.0, 10.0, 10.0],
            vec![0.0, 0.0, 10.0, 10.0],
            &[
                vec![[0.0; 3], [0.0, 10.0, 0.0]],
                vec![[10.0, 0.0, 0.0], [10.0, 10.0, 0.0]],
            ],
            None,
        )
        .unwrap();
        let mesh = Mesh::surface_grid(&flat, 0.01, 0.35, false);
        assert_eq!(
            (mesh.vertex_count(), mesh.face_count(), quads(&mesh)),
            (4, 1, 1)
        );
    }

    #[test]
    fn a_flipped_grid_faces_the_other_way() {
        let s = sphere(5.0);
        let out = Mesh::surface_grid(&s, 0.05, 0.35, false);
        let inward = Mesh::surface_grid(&s, 0.05, 0.35, true);
        let (a, b) = (
            out.welded(1e-9).0.volume().unwrap(),
            inward.welded(1e-9).0.volume().unwrap(),
        );
        assert!((a + b).abs() < 1e-6 * a.abs(), "{a} {b}");
    }
}
