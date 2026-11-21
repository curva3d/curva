//! MeshToNURB: each mesh face as a NURBS surface, the surfaces sewn into one polysurface, through
//! Curva's kernel (spec `docs/specs/mesh-and-subd.md` §3.3).

use crate::{Mesh, Point};
use curva_kernel::{Body, Curve, KernelError, NurbsSurface};

/// How `MeshToNURB` treats triangles and faces of more than four corners: Rhino 8's
/// `TrimTriangularFaces` and `UseNgons`, both on at first (measured).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshToNurbOptions {
    /// A triangle as a flat trimmed surface; off, as an untrimmed bilinear surface with one side
    /// shrunk to its first corner.
    pub trim_triangles: bool,
    /// A face of more than four corners as one surface; off, as its triangles.
    pub ngons: bool,
}

impl Default for MeshToNurbOptions {
    fn default() -> Self {
        Self {
            trim_triangles: true,
            ngons: true,
        }
    }
}

/// One face's surface: a quad's bilinear surface, a triangle's (trimmed or shrunk), or a larger
/// face's flat trimmed surface or n-sided fill.
fn face_body(points: &[Point], trim_triangles: bool) -> Result<Body, KernelError> {
    let edges: Vec<Curve> = (0..points.len())
        .map(|k| Curve::Line {
            start: points[k],
            end: points[(k + 1) % points.len()],
        })
        .collect();
    match points.len() {
        4 => Body::from_edges(&edges),
        3 if !trim_triangles => {
            // Rhino 8 (RhinoCommon, 21 November 2026): a triangle A, B, C becomes the bilinear
            // surface on [[A, A], [B, C]].
            let [a, b, c] = [points[0], points[1], points[2]];
            let knots = vec![0.0, 0.0, 1.0, 1.0];
            NurbsSurface::new(
                [1, 1],
                knots.clone(),
                knots,
                &[vec![a, a], vec![b, c]],
                None,
            )?
            .to_body()
        }
        _ => Body::planar(&[Curve::Polyline {
            points: points.to_vec(),
            closed: true,
        }])
        .or_else(|_| Body::from_edges(&edges)),
    }
}

impl Mesh {
    /// The mesh as a polysurface with Rhino's `MeshToNURB` options (see [`MeshToNurbOptions`]);
    /// [`Mesh::to_nurbs`] uses their defaults.
    ///
    /// # Errors
    /// When the kernel cannot make a face's surface or sew them.
    pub fn to_nurbs_with(&self, options: MeshToNurbOptions) -> Result<Body, KernelError> {
        let at = |v: u32| self.vertices()[v as usize];
        let mut parts = Vec::new();
        for face in self.faces() {
            let points: Vec<Point> = face.iter().map(|&v| at(v)).collect();
            if points.len() > 4 && !options.ngons {
                // Its triangles, split as TriangulateMesh splits faces.
                let one = Mesh::new(
                    points.clone(),
                    &[(0..face.len() as u32).collect::<Vec<_>>()],
                )
                .map_err(|e| KernelError(e.to_string()))?;
                let (split, _) = one.triangulated();
                for triangle in split.faces() {
                    let corners: Vec<Point> = triangle
                        .iter()
                        .map(|&v| split.vertices()[v as usize])
                        .collect();
                    parts.push(face_body(&corners, options.trim_triangles)?);
                }
            } else {
                parts.push(face_body(&points, options.trim_triangles)?);
            }
        }
        Body::assembled(&parts)
    }

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
        self.to_nurbs_with(MeshToNurbOptions::default())
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

    fn tetrahedron() -> Mesh {
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let faces = [vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![0, 3, 2]];
        Mesh::new(points, &faces).unwrap()
    }

    #[test]
    fn untrimmed_triangles_shrink_one_side_as_in_rhino() {
        // Rhino 8 with TrimTriangularFaces off (RhinoCommon, 21 November 2026): the tetrahedron
        // is still a solid of four faces and volume 1/6, each an untrimmed surface on
        // [[A, A], [B, C]].
        let options = MeshToNurbOptions {
            trim_triangles: false,
            ngons: true,
        };
        let body = tetrahedron().to_nurbs_with(options).unwrap();
        assert_eq!(body.face_count().unwrap(), 4);
        assert!(body.is_solid());
        assert!(close(body.volume().unwrap(), 1.0 / 6.0));
    }

    #[test]
    fn a_face_of_six_corners_is_one_surface_unless_ngons_are_off() {
        // Rhino 8's UseNgons, on at first (measured): a flat hexagon of area 6 gives one trimmed
        // surface; off, its triangles.
        let points = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let hexagon = Mesh::new(points, &[vec![0, 1, 2, 3, 4, 5]]).unwrap();
        let one = hexagon.to_nurbs().unwrap();
        assert_eq!(one.face_count().unwrap(), 1);
        assert!(close(one.area().unwrap(), 6.0));
        let options = MeshToNurbOptions {
            trim_triangles: true,
            ngons: false,
        };
        let split = hexagon.to_nurbs_with(options).unwrap();
        assert_eq!(split.face_count().unwrap(), 4);
        assert!(close(split.area().unwrap(), 6.0));
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
