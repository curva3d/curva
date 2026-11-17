//! Sizes of a mesh: its extent, area and volume.

use crate::triangulate::newell_normal;
use crate::{Mesh, Point};

impl Mesh {
    /// The lowest and highest corners around it; `None` when it has no points.
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let first = *self.vertices().first()?;
        Some(self.vertices().iter().fold((first, first), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        }))
    }

    /// The area of all its faces, each measured flat in its own plane by its Newell normal, so a
    /// face that is not quite flat still counts about what it covers.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.faces()
            .map(|face| {
                let points: Vec<Point> =
                    face.iter().map(|&v| self.vertices()[v as usize]).collect();
                length(newell_normal(&points)) / 2.0
            })
            .sum()
    }

    /// The volume it encloses, when it is closed; negative when its faces point inwards.
    ///
    /// Sums the signed volumes of the tetrahedra from the origin to each triangle, by the
    /// divergence theorem (Zhang and Chen, "Efficient feature extraction for 2D/3D objects in mesh
    /// representation", ICIP 2001, eq. 6). Measured from the mesh's own centre, so a mesh far
    /// from the origin keeps its digits.
    #[must_use]
    pub fn volume(&self) -> Option<f64> {
        if !self.is_closed() {
            return None;
        }
        let (lo, hi) = self.bounds()?;
        let centre: Point = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
        let at = |v: u32| {
            let p = self.vertices()[v as usize];
            [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]
        };
        let six_times: f64 = self
            .triangles()
            .triangles
            .iter()
            .map(|&[a, b, c]| {
                let (a, b, c) = (at(a), at(b), at(c));
                a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0])
            })
            .sum();
        Some(six_times / 6.0)
    }
}

fn length(v: Point) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::cube;
    use std::f64::consts::PI;

    #[test]
    fn a_unit_cube_has_volume_one_and_area_six() {
        let cube = cube();
        assert!((cube.volume().unwrap() - 1.0).abs() < 1e-12);
        assert!((cube.area() - 6.0).abs() < 1e-12);
        assert_eq!(cube.bounds(), Some(([0.0; 3], [1.0; 3])));
    }

    #[test]
    fn an_inside_out_cube_has_negative_volume_and_an_open_one_none() {
        assert!((cube().flipped().volume().unwrap() + 1.0).abs() < 1e-12);
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(1).map(<[u32]>::to_vec).collect();
        assert_eq!(
            Mesh::new(cube.vertices().to_vec(), &faces)
                .unwrap()
                .volume(),
            None
        );
    }

    #[test]
    fn a_cube_far_from_the_origin_keeps_its_volume() {
        // At map coordinates (a million metres away, in millimetres) a sum from the origin would
        // lose most of its digits.
        let far = cube().translated([1e9, 2e9, 3e8]);
        assert!((far.volume().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_finer_sphere_approaches_its_true_volume() {
        let errors: Vec<f64> = [8, 32, 128]
            .iter()
            .map(|&n| (uv_sphere(n).volume().unwrap() - 4.0 / 3.0 * PI).abs())
            .collect();
        assert!(errors[0] > errors[1] && errors[1] > errors[2]);
        // A polyhedron inside a sphere misses a volume that shrinks with the square of its face
        // size: four times the faces around, about sixteen times less missing.
        let ratio = errors[1] / errors[2];
        assert!((15.0..17.0).contains(&ratio), "{errors:?}");
        assert!(errors[2] < 5e-3);
    }

    /// A sphere of radius 1 with `n` faces around and `n / 2` from pole to pole.
    fn uv_sphere(n: u32) -> Mesh {
        let rings = n / 2;
        let mut v = vec![[0.0, 0.0, -1.0]];
        for r in 1..rings {
            let phi = PI * f64::from(r) / f64::from(rings) - PI / 2.0;
            for k in 0..n {
                let theta = 2.0 * PI * f64::from(k) / f64::from(n);
                v.push([phi.cos() * theta.cos(), phi.cos() * theta.sin(), phi.sin()]);
            }
        }
        v.push([0.0, 0.0, 1.0]);
        let top = u32::try_from(v.len() - 1).unwrap();
        let at = |r: u32, k: u32| 1 + (r - 1) * n + k % n;
        let mut faces: Vec<Vec<u32>> = Vec::new();
        for k in 0..n {
            faces.push(vec![0, at(1, k + 1), at(1, k)]);
            faces.push(vec![top, at(rings - 1, k), at(rings - 1, k + 1)]);
        }
        for r in 1..rings - 1 {
            for k in 0..n {
                faces.push(vec![at(r, k), at(r, k + 1), at(r + 1, k + 1), at(r + 1, k)]);
            }
        }
        Mesh::new(v, &faces).unwrap()
    }
}
