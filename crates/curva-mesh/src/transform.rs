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

    /// Faces reversed without turning normals round: after a mirror they already face outwards.
    fn flipped_keeping_normals(&self) -> Self {
        let flipped = self.flipped();
        let normals = flipped.normals().mapped_with(|n| n.map(|c| -c));
        flipped.with_vertices(flipped.vertices().to_vec(), normals)
    }
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

    #[test]
    fn a_mirror_turns_normals_with_the_shape() {
        let normals = Attribute::PerVertex(vec![[1.0, 0.0, 0.0]; 8]);
        let cube = cube().with_normals(normals).unwrap();
        let mirrored = cube.mapped(&Similarity::mirror([0.0; 3], [1.0, 0.0, 0.0]));
        let n = mirrored.normals().at(0, 0).unwrap();
        assert!((n[0] + 1.0).abs() < 1e-12);
    }
}
