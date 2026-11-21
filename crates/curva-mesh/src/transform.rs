//! Moving, turning, mirroring and resizing meshes.

use crate::{Mesh, Point};
use curva_kernel::Similarity;

impl Mesh {
    /// The mesh moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        let vertices = self
            .vertices()
            .iter()
            .map(|p| [p[0] + by[0], p[1] + by[1], p[2] + by[2]])
            .collect();
        self.with_vertices(vertices, self.normals().clone())
    }

    /// The mesh turned, mirrored or resized. A mirror turns faces inside out, so their corner
    /// order is reversed to keep them facing outwards, as Rhino does.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let origin = change.point([0.0; 3]);
        let axis = |a: Point| {
            let p = change.point(a);
            [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]]
        };
        let (x, y, z) = (
            axis([1.0, 0.0, 0.0]),
            axis([0.0, 1.0, 0.0]),
            axis([0.0, 0.0, 1.0]),
        );
        let scale = change.factor();
        // Normals turn with the shape but keep length 1; a similarity needs no inverse transpose.
        let turn = |n: &Point| -> Point {
            std::array::from_fn(|i| (x[i] * n[0] + y[i] * n[1] + z[i] * n[2]) / scale)
        };
        let vertices = self.vertices().iter().map(|&p| change.point(p)).collect();
        let moved = self.with_vertices(vertices, self.normals().mapped_with(turn));
        let mirrored = determinant(x, y, z) < 0.0;
        if mirrored {
            moved.flipped_keeping_normals()
        } else {
            moved
        }
    }

    /// The mesh changed by any affine map, as a 4 × 4 matrix for points written as rows,
    /// `p' = p·A + t` (rows 0 to 2 are where the X, Y and Z axes go, row 3 the move), as
    /// [`curva_kernel::Body::placement`]: stretching in one direction, shearing, mirroring. Normals
    /// follow by the inverse transpose, so they stay square to the faces; a map that mirrors
    /// reverses the faces' corner order to keep them facing outwards. A flat map (one that squashes
    /// the mesh into a plane or line) has no inverse, so the normals are worked out again.
    #[must_use]
    pub fn transformed(&self, map: [[f64; 4]; 4]) -> Self {
        let a: [Point; 3] = std::array::from_fn(|r| [map[r][0], map[r][1], map[r][2]]);
        let t = [map[3][0], map[3][1], map[3][2]];
        let point = |p: Point| -> Point {
            std::array::from_fn(|j| p[0] * a[0][j] + p[1] * a[1][j] + p[2] * a[2][j] + t[j])
        };
        let vertices: Vec<Point> = self.vertices().iter().map(|&p| point(p)).collect();
        let det = determinant(a[0], a[1], a[2]);
        let moved = if det.abs() > f64::EPSILON * scale_cubed(&a) {
            // A normal as a row times the inverse transpose of A (for rows) is A⁻¹ times it as a
            // column: n'_j = Σ_i inv[j][i] n_i.
            let inv = inverse(&a, det);
            let turn = |n: &Point| -> Point {
                let m: Point = std::array::from_fn(|j| (0..3).map(|i| inv[j][i] * n[i]).sum());
                let length = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
                if length > 0.0 {
                    m.map(|c| c / length)
                } else {
                    *n
                }
            };
            self.with_vertices(vertices, self.normals().mapped_with(turn))
        } else {
            let flat = self.with_vertices(vertices, self.normals().clone());
            if flat.normals().is_none() {
                flat
            } else {
                flat.with_rebuilt_normals()
            }
        };
        if det < 0.0 {
            moved.flipped_keeping_normals()
        } else {
            moved
        }
    }

    /// Each point moved by `map`, as Rhino deforms a mesh (Bend, Twist, Taper, CageEdit and the
    /// others move its points); normals, where the mesh has them, are worked out again.
    #[must_use]
    pub fn deformed(&self, map: impl Fn(Point) -> Point) -> Self {
        let vertices = self.vertices().iter().map(|&p| map(p)).collect();
        let moved = self.with_vertices(vertices, self.normals().clone());
        if moved.normals().is_none() {
            moved
        } else {
            moved.with_rebuilt_normals()
        }
    }

    /// The mesh with some points moved, by number, to new places, keeping its faces, normals,
    /// texture and colours: what dragging picked points does. Numbers the mesh does not have are
    /// left out; a point named twice takes its last place.
    #[must_use]
    pub fn with_points_moved(&self, moves: &[(u32, Point)]) -> Self {
        let mut vertices = self.vertices().to_vec();
        for &(v, p) in moves {
            if let Some(slot) = vertices.get_mut(v as usize) {
                *slot = p;
            }
        }
        self.with_vertices(vertices, self.normals().clone())
    }

    /// Faces reversed without turning normals round: after a mirror they already face outwards.
    fn flipped_keeping_normals(&self) -> Self {
        let flipped = self.flipped();
        let normals = flipped.normals().mapped_with(|n| n.map(|c| -c));
        flipped.with_vertices(flipped.vertices().to_vec(), normals)
    }
}

/// The largest row length cubed: the size a determinant is measured against.
fn scale_cubed(a: &[Point; 3]) -> f64 {
    let longest = a
        .iter()
        .map(|r| (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt())
        .fold(0.0, f64::max);
    longest.powi(3).max(f64::MIN_POSITIVE)
}

/// The inverse of the 3 × 3 matrix with rows `a`, given its determinant.
fn inverse(a: &[Point; 3], det: f64) -> [Point; 3] {
    let m = |r: usize, c: usize| a[r % 3][c % 3];
    // The adjugate: each entry the cofactor of its transposed position.
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            (m(c + 1, r + 1) * m(c + 2, r + 2) - m(c + 1, r + 2) * m(c + 2, r + 1)) / det
        })
    })
}

fn determinant(x: Point, y: Point, z: Point) -> f64 {
    x[0] * (y[1] * z[2] - y[2] * z[1]) - x[1] * (y[0] * z[2] - y[2] * z[0])
        + x[2] * (y[0] * z[1] - y[1] * z[0])
}

#[cfg(test)]
mod tests {
    use crate::Attribute;
    use crate::mesh::tests::cube;
    use curva_kernel::Similarity;

    #[test]
    fn moving_keeps_shape_and_shifts_bounds() {
        let moved = cube().translated([10.0, 0.0, -1.0]);
        assert_eq!(moved.bounds(), Some(([10.0, 0.0, -1.0], [11.0, 1.0, 0.0])));
        assert_eq!(moved.face(3), cube().face(3));
    }

    #[test]
    fn a_mirrored_cube_still_faces_outwards() {
        let mirrored = cube().mapped(&Similarity::mirror([0.0; 3], [1.0, 0.0, 0.0]));
        assert!((mirrored.volume().unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(mirrored.bounds(), Some(([-1.0, 0.0, 0.0], [0.0, 1.0, 1.0])));
    }

    #[test]
    fn scaling_by_two_multiplies_volume_by_eight_and_keeps_normals_unit() {
        let normals = Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        let big = cube.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert!((big.volume().unwrap() - 8.0).abs() < 1e-12);
        let n = big.normals().at(0, 0).unwrap();
        assert!((n[2] - 1.0).abs() < 1e-12);
    }

    fn stretch(x: f64, y: f64, z: f64) -> [[f64; 4]; 4] {
        [
            [x, 0.0, 0.0, 0.0],
            [0.0, y, 0.0, 0.0],
            [0.0, 0.0, z, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn a_stretch_in_one_direction_resizes_volume_and_bounds() {
        let long = cube().transformed(stretch(3.0, 1.0, 1.0));
        assert!((long.volume().unwrap() - 3.0).abs() < 1e-12);
        assert_eq!(long.bounds(), Some(([0.0; 3], [3.0, 1.0, 1.0])));
        let moved = cube().transformed([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [5.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(moved, cube().translated([5.0, 0.0, 0.0]));
    }

    #[test]
    fn sheared_normals_stay_square_to_the_faces() {
        // A slanted face's normal must stay square to it after a shear, not just turn with it.
        let normals = Attribute::PerVertex(vec![[1.0, 0.0, 0.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        // x' = x + y: the plane x = 0 becomes x' = y', whose normal is (1, -1, 0) / √2.
        let shear = [
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let sheared = cube.transformed(shear);
        let n = sheared.normals().at(0, 0).unwrap();
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            (n[0] - h).abs() < 1e-12 && (n[1] + h).abs() < 1e-12,
            "{n:?}"
        );
        assert!(
            (sheared.volume().unwrap() - 1.0).abs() < 1e-12,
            "a shear keeps volume"
        );
    }

    #[test]
    fn a_mirroring_map_keeps_faces_outward() {
        let flipped = cube().transformed(stretch(-2.0, 1.0, 1.0));
        assert!((flipped.volume().unwrap() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_flat_map_works_the_normals_out_again() {
        let normals = Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        let flat = cube.transformed(stretch(1.0, 1.0, 0.0));
        assert_eq!(flat.bounds(), Some(([0.0; 3], [1.0, 1.0, 0.0])));
        assert!(!flat.normals().is_none());
    }

    #[test]
    fn a_deformation_moves_each_point() {
        let normals = Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        // Taller the further along x: a taper.
        let tapered = cube.deformed(|p| [p[0], p[1], p[2] * (1.0 + p[0])]);
        assert_eq!(tapered.bounds(), Some(([0.0; 3], [1.0, 1.0, 2.0])));
        assert!((tapered.volume().unwrap() - 1.5).abs() < 1e-12);
        let n = tapered.normals().at(0, 0).unwrap();
        assert!((n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_mirror_turns_normals_with_the_shape() {
        let normals = Attribute::PerVertex(vec![[1.0, 0.0, 0.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        let mirrored = cube.mapped(&Similarity::mirror([0.0; 3], [1.0, 0.0, 0.0]));
        let n = mirrored.normals().at(0, 0).unwrap();
        assert!((n[0] + 1.0).abs() < 1e-12);
    }
}
