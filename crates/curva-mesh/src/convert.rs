//! Changing a mesh's faces without moving its points: TriangulateMesh, QuadrangulateMesh and
//! ExtractMeshFaces (spec `docs/specs/mesh-and-subd.md` §1). Normals, texture coordinates and
//! colours follow the corners they belong to.

use crate::check::{Topology, distance};
use crate::mesh::NewCorner;
use crate::triangulate::newell_normal;
use crate::{Mesh, Point};

impl Mesh {
    /// Every face of four or more corners split into triangles, as Rhino's TriangulateMesh, and
    /// how many faces were split. Quads split along their shorter diagonal and larger faces as
    /// [`Mesh::triangles`] splits them for display.
    #[must_use]
    pub fn triangulated(&self) -> (Self, usize) {
        let split = self.faces().filter(|f| f.len() > 3).count();
        if split == 0 {
            return (self.clone(), 0);
        }
        let faces: Vec<Vec<NewCorner>> = self
            .triangles()
            .corners
            .iter()
            .map(|t| {
                t.iter()
                    .map(|&c| NewCorner {
                        vertex: self.corners()[c as usize],
                        from: c as usize,
                        flip: false,
                    })
                    .collect()
            })
            .collect();
        (self.rebuilt(&self.all_vertices(), &faces), split)
    }

    /// Pairs of triangles that share an edge joined into quads, as Rhino's QuadrangulateMesh, and
    /// how many quads were made.
    ///
    /// Two triangles join when their faces lean less than `max_angle` radians apart, the quad they
    /// make is convex, and its shorter diagonal is at least `min_diagonal_ratio` of its longer one
    /// (Rhino suggests 2° and 0.875). Where a triangle could join more than one neighbour, the
    /// flattest, then squarest, quad is made first.
    // CUSTOM MESH OPERATION: triangles-to-quads
    #[must_use]
    pub fn quadrangulated(&self, max_angle: f64, min_diagonal_ratio: f64) -> (Self, usize) {
        let topology = Topology::of(self);
        let at = |v: u32| self.vertices()[v as usize];
        let mut candidates: Vec<(f64, f64, usize, usize, [usize; 4])> = Vec::new();
        for uses in topology.edges.values() {
            let [a, b] = uses[..] else { continue };
            if a.forward == b.forward
                || self.face(a.face).len() != 3
                || self.face(b.face).len() != 3
            {
                continue;
            }
            // Triangle A runs p→q→r and B runs q→p→s; without p–q they make q→r→p→s. Each
            // corner is a position in `corners`, so the quad keeps each corner's data.
            let ra = self.face_range(a.face);
            let rb = self.face_range(b.face);
            let corner = |range: &std::ops::Range<usize>, k: usize| range.start + k % 3;
            let (p, q, r) = (
                corner(&ra, a.corner),
                corner(&ra, a.corner + 1),
                corner(&ra, a.corner + 2),
            );
            let s = corner(&rb, b.corner + 2);
            let quad = [q, r, p, s];
            let points = quad.map(|c| at(self.corners()[c]));
            let place = |c: usize| topology.place[self.corners()[c] as usize];
            if place(r) == place(s) {
                continue;
            }
            let angle = angle_between(
                newell_normal(&[points[0], points[1], points[2]]),
                newell_normal(&[points[2], points[3], points[0]]),
            );
            let (d1, d2) = (
                distance(points[0], points[2]),
                distance(points[1], points[3]),
            );
            let ratio = d1.min(d2) / d1.max(d2);
            if angle <= max_angle && ratio >= min_diagonal_ratio && convex(&points) {
                candidates.push((angle, ratio, a.face, b.face, quad));
            }
        }
        candidates.sort_by(|x, y| {
            x.0.total_cmp(&y.0)
                .then(y.1.total_cmp(&x.1))
                .then((x.2, x.3).cmp(&(y.2, y.3)))
        });
        let mut taken = vec![false; self.face_count()];
        let mut quad_at: Vec<Option<[usize; 4]>> = vec![None; self.face_count()];
        let mut made = 0;
        for (_, _, fa, fb, quad) in candidates {
            if !taken[fa] && !taken[fb] {
                taken[fa] = true;
                taken[fb] = true;
                quad_at[fa.min(fb)] = Some(quad);
                made += 1;
            }
        }
        if made == 0 {
            return (self.clone(), 0);
        }
        let mut faces = Vec::with_capacity(self.face_count() - made);
        for f in 0..self.face_count() {
            let corners: Vec<usize> = match quad_at[f] {
                Some(quad) => quad.to_vec(),
                None if taken[f] => continue,
                None => self.face_range(f).collect(),
            };
            faces.push(self.corners_of(&corners, false));
        }
        (self.rebuilt(&self.all_vertices(), &faces), made)
    }

    /// The faces numbered `faces` as a mesh of their own, and the rest as another, as Rhino's
    /// ExtractMeshFaces. Each keeps only the points its faces use.
    #[must_use]
    pub fn split_off(&self, faces: &[usize]) -> (Self, Self) {
        let mut chosen = vec![false; self.face_count()];
        for &f in faces {
            if let Some(c) = chosen.get_mut(f) {
                *c = true;
            }
        }
        let (taken, _) = self.keeping_faces(|f| chosen[f]);
        let (rest, _) = self.keeping_faces(|f| !chosen[f]);
        (
            taken.without_unused_vertices().0,
            rest.without_unused_vertices().0,
        )
    }
}

/// The angle between two directions, in radians.
pub(crate) fn angle_between(a: Point, b: Point) -> f64 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    distance(cross, [0.0; 3]).atan2(dot)
}

/// Whether four points make a convex quad, every corner turning the same way about its normal.
fn convex(points: &[Point; 4]) -> bool {
    let normal = newell_normal(points);
    (0..4).all(|k| {
        let (a, b, c) = (points[k], points[(k + 1) % 4], points[(k + 2) % 4]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
        let turn = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        turn[0] * normal[0] + turn[1] * normal[1] + turn[2] * normal[2] > 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Attribute;
    use crate::measure::tests::uv_sphere;
    use crate::mesh::tests::cube;

    // Counts compared with Rhino 8 (`ConvertQuadsToTriangles`, `ConvertTrianglesToQuads`) on the
    // same meshes, 17 November 2026.

    #[test]
    fn triangulating_splits_quads_and_keeps_triangles() {
        let (cube_tris, split) = cube().triangulated();
        assert_eq!((cube_tris.face_count(), split), (12, 6));
        assert!(cube_tris.faces().all(|f| f.len() == 3));
        assert_eq!(cube_tris.volume(), Some(1.0));
        let sphere = uv_sphere(32);
        assert_eq!(sphere.triangulated().0.face_count(), 960, "as Rhino");
        let (again, split) = cube_tris.triangulated();
        assert_eq!((again, split), (cube_tris, 0));
    }

    #[test]
    fn triangulating_keeps_each_corners_texture() {
        let texture: Vec<[f64; 2]> = (0..24).map(|i| [f64::from(i), 0.0]).collect();
        let cube = cube().with_texture(Attribute::PerCorner(texture)).unwrap();
        let (tris, _) = cube.triangulated();
        let Attribute::PerCorner(t) = tris.texture() else {
            panic!("per corner in, per corner out")
        };
        // Each triangle corner carries the value of the quad corner it came from.
        for (k, &c) in tris.corners().iter().enumerate() {
            let from = (0..24).find(|&q| {
                cube.corners()[q] == c && cube.texture().at(q, c as usize) == Some(&t[k])
            });
            assert!(from.is_some(), "corner {k}");
        }
    }

    #[test]
    fn quadrangulating_puts_split_quads_back_as_rhino_does() {
        let sphere = uv_sphere(32);
        let tris = sphere.triangulated().0;
        let (quads, made) = tris.quadrangulated(2f64.to_radians(), 0.875);
        assert_eq!(made, 448);
        assert_eq!(
            quads.face_count(),
            512,
            "as Rhino: 448 quads and the 64 pole triangles"
        );
        assert!((quads.volume().unwrap() - sphere.volume().unwrap()).abs() < 1e-12);
        let (back, made) = cube()
            .triangulated()
            .0
            .quadrangulated(2f64.to_radians(), 0.875);
        assert_eq!((back.face_count(), made), (6, 6));
        assert!(back.check(1e-4).is_good());
    }

    #[test]
    fn quadrangulating_leaves_bent_or_lopsided_pairs() {
        let tris = cube().triangulated().0;
        // Across the cube's edges the faces lean 90° apart, so a 1° limit still joins only pairs
        // in one side; a ratio above 1 joins nothing.
        assert_eq!(tris.quadrangulated(1f64.to_radians(), 0.875).1, 6);
        assert_eq!(tris.quadrangulated(1f64.to_radians(), 1.01).1, 0);
        // A long rectangle still joins: its diagonals are equal, however thin it is.
        let points = vec![
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let thin = Mesh::new(points, &[[0, 1, 2], [0, 2, 3]]).unwrap();
        assert_eq!(thin.quadrangulated(2f64.to_radians(), 0.875).1, 1);
        let dart = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.5, 0.5, 0.0],
                [0.0, 2.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        assert_eq!(
            dart.quadrangulated(2f64.to_radians(), 0.0).1,
            0,
            "a dart is not convex"
        );
    }

    #[test]
    fn extracting_faces_makes_two_meshes_each_with_its_own_points() {
        let (taken, rest) = cube().split_off(&[0, 1]);
        assert_eq!((taken.face_count(), taken.vertex_count()), (2, 8));
        assert_eq!((rest.face_count(), rest.vertex_count()), (4, 8));
        let (taken, rest) = cube().split_off(&[2]);
        assert_eq!((taken.face_count(), taken.vertex_count()), (1, 4));
        assert_eq!(taken.face(0), &[0, 1, 3, 2], "points renumbered in order");
        assert_eq!(rest.face_count(), 5);
        let (none, all) = cube().split_off(&[99]);
        assert_eq!((none.face_count(), all), (0, cube()));
    }
}
