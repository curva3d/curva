//! MeshToNURB: each mesh face as a NURBS surface, the surfaces sewn into one polysurface, through
//! Curva's kernel (spec `docs/specs/mesh-and-subd.md` §3.3).

use crate::Mesh;
use curva_kernel::{Body, Curve, KernelError};

impl Mesh {
    /// The mesh as a polysurface of one surface per face, as Rhino's MeshToNURB, sewn where faces
    /// meet; a closed mesh gives a solid.
    ///
    /// A quad gives the untrimmed bilinear surface through its corners; a triangle, or a flat face
    /// of more corners, becomes a flat trimmed surface, and a bent face of more corners an n-sided
    /// fill. Measured in Rhino 8, whose `TrimTriangularFaces` is on at first: quads untrimmed,
    /// triangles trimmed, the mesh kept. Like Rhino's, the result is heavy:
    /// a surface for every face.
    ///
    /// # Errors
    /// When the kernel cannot make a face's surface or sew them.
    pub fn to_nurbs(&self) -> Result<Body, KernelError> {
        let at = |v: u32| self.vertices()[v as usize];
        let parts = self
            .faces()
            .map(|face| {
                let points: Vec<_> = face.iter().map(|&v| at(v)).collect();
                let edges: Vec<Curve> = (0..points.len())
                    .map(|k| Curve::Line {
                        start: points[k],
                        end: points[(k + 1) % points.len()],
                    })
                    .collect();
                if points.len() == 4 {
                    Body::from_edges(&edges)
                } else {
                    Body::planar(&[Curve::Polyline {
                        points: points.clone(),
                        closed: true,
                    }])
                    .or_else(|_| Body::from_edges(&edges))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Body::assembled(&parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::tests::uv_sphere;
    use crate::mesh::tests::cube;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn a_cube_becomes_a_solid_of_six_faces_as_in_rhino() {
        let body = cube().to_nurbs().unwrap();
        assert_eq!(body.face_count().unwrap(), 6);
        assert!(body.is_solid());
        assert!(close(body.volume().unwrap(), 1.0));
    }

    #[test]
    fn an_open_box_stays_open() {
        let faces: Vec<Vec<u32>> = cube().faces().skip(1).map(<[u32]>::to_vec).collect();
        let open = Mesh::new(cube().vertices().to_vec(), &faces).unwrap();
        let body = open.to_nurbs().unwrap();
        assert_eq!(body.face_count().unwrap(), 5);
        assert!(!body.is_solid());
        assert_eq!(body.naked_edges().unwrap().len(), 4);
    }

    #[test]
    fn a_tetrahedron_becomes_a_solid_of_four_flat_faces_as_in_rhino() {
        // Measured in Rhino 8: four trimmed three-sided surfaces, a solid of volume 1/6.
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let faces = [vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![0, 3, 2]];
        let body = Mesh::new(points, &faces).unwrap().to_nurbs().unwrap();
        assert_eq!(body.face_count().unwrap(), 4);
        assert!(body.is_solid());
        assert!(close(body.volume().unwrap(), 1.0 / 6.0));
    }

    #[test]
    fn triangles_and_bent_quads_become_surfaces_through_their_corners() {
        let sphere = uv_sphere(12);
        let body = sphere.to_nurbs().unwrap();
        assert_eq!(body.face_count().unwrap(), sphere.face_count());
        assert!(body.is_solid());
        // A UV sphere's quads are flat, so the surfaces enclose what the mesh does, as Rhino's
        // MeshToNURB of the same sphere does (4.121942 both, at 32 segments).
        assert!(close(body.volume().unwrap(), sphere.volume().unwrap()));
    }

    #[test]
    fn a_prism_with_five_sided_ends_becomes_a_solid() {
        let ring: Vec<[f64; 3]> = (0..5)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 5.0;
                [a.cos(), a.sin(), 0.0]
            })
            .collect();
        let mut points = ring.clone();
        points.extend(ring.iter().map(|p| [p[0], p[1], 2.0]));
        let mut faces: Vec<Vec<u32>> = vec![vec![4, 3, 2, 1, 0], vec![5, 6, 7, 8, 9]];
        for k in 0..5u32 {
            faces.push(vec![k, (k + 1) % 5, (k + 1) % 5 + 5, k + 5]);
        }
        let prism = Mesh::new(points, &faces).unwrap();
        let body = prism.to_nurbs().unwrap();
        assert!(body.is_solid());
        assert_eq!(body.face_count().unwrap(), 7);
        assert!(close(body.volume().unwrap(), prism.volume().unwrap()));
    }
}
