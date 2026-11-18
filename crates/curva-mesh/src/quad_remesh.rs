//! QuadRemesh: a mesh or a SubD's smooth surface turned into an even grid of quads, on quadrs
//! (decision 0033, spec `docs/specs/mesh-and-subd.md` §3.5). This file is the only one that calls
//! it.
//!
//! quadrs follows Instant Meshes (Jakob, Tarini, Panozzo and Sorkine-Hornung, "Instant
//! field-aligned meshes", SIGGRAPH Asia 2015): it lays a field of directions over the surface and
//! grows quads along it. Given the same mesh and the same seed it makes the same quads every
//! time. Points go in measured from the mesh's centre, so a mesh far from the origin keeps its
//! digits.

use crate::{Mesh, Point, SubD};
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
}

impl QuadRemeshOptions {
    /// About `count` quads, with Curva's fixed seed. Rhino's own default is 2000.
    #[must_use]
    pub fn count(count: usize) -> Self {
        Self {
            target: QuadTarget::Count(count),
            seed: 1,
        }
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

impl Mesh {
    /// The mesh remeshed into quads, as Rhino's QuadRemesh does with its output set to a mesh.
    /// The input is taken as triangles; normals, texture coordinates and colours are not carried.
    ///
    /// # Errors
    /// When the mesh has no faces, or quadrs cannot remesh it.
    pub fn quad_remeshed(&self, options: &QuadRemeshOptions) -> Result<Self, QuadRemeshError> {
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
        let target = match options.target {
            QuadTarget::Count(n) => quadrs::RemeshTarget::FaceCount(n.max(1)),
            QuadTarget::EdgeLength(l) => quadrs::RemeshTarget::EdgeLength(l),
        };
        let mut settings = quadrs::RemeshOptions::new(target);
        settings.seed = Some(options.seed);
        let result =
            quadrs::remesh(&input, &settings).map_err(|e| QuadRemeshError(format!("{e:?}")))?;
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
                let face: Vec<u32> = f.iter().map(|&v| u32::try_from(v).unwrap_or(u32::MAX)).collect();
                let distinct = (1..face.len()).all(|i| !face[..i].contains(&face[i]));
                (face.len() >= 3 && distinct && face.iter().all(|&v| (v as usize) < points.len()))
                    .then_some(face)
            })
            .collect();
        let mesh = Mesh::new(points, &faces).map_err(|e| QuadRemeshError(e.to_string()))?;
        Ok(mesh.without_unused_vertices().0)
    }
}

impl SubD {
    /// A mesh remeshed into quads as a SubD, Rhino's QuadRemesh default output: the quads become
    /// the cage.
    ///
    /// # Errors
    /// When the mesh cannot be remeshed.
    pub fn quad_remeshed_from(mesh: &Mesh, options: &QuadRemeshOptions) -> Result<Self, QuadRemeshError> {
        Ok(Self::new(mesh.quad_remeshed(options)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame, SphereStyle};

    fn sphere() -> Mesh {
        Mesh::mesh_sphere(&Frame::world(), 10.0, SphereStyle::Uv { vertical: 40, around: 60 })
    }

    fn quads(m: &Mesh) -> usize {
        m.faces().filter(|f| f.len() == 4).count()
    }

    #[test]
    fn a_sphere_becomes_mostly_quads_near_the_count_asked_for() {
        let out = sphere().quad_remeshed(&QuadRemeshOptions::count(400)).unwrap();
        let n = out.face_count();
        assert!((280..=520).contains(&n), "{n} faces for 400 asked");
        assert!(quads(&out) * 10 >= n * 9, "{} of {n} are quads", quads(&out));
        assert!(out.is_closed());
        let ball = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
        let kept = out.volume().unwrap().abs() / ball;
        assert!(kept > 0.95 && kept < 1.02, "kept {kept}");
    }

    #[test]
    fn the_same_seed_gives_the_same_quads() {
        let options = QuadRemeshOptions::count(200);
        assert_eq!(sphere().quad_remeshed(&options), sphere().quad_remeshed(&options));
    }

    #[test]
    fn far_from_the_origin_it_remeshes_the_same() {
        let near = sphere().quad_remeshed(&QuadRemeshOptions::count(200)).unwrap();
        let far = sphere()
            .translated([1.0e6, -2.0e6, 3.0e5])
            .quad_remeshed(&QuadRemeshOptions::count(200))
            .unwrap();
        assert_eq!(near.face_count(), far.face_count());
        assert!((near.volume().unwrap() - far.volume().unwrap()).abs() < 1e-6 * near.volume().unwrap().abs());
    }

    #[test]
    fn a_subd_comes_out_with_the_quads_as_its_cage() {
        let subd = SubD::quad_remeshed_from(&sphere(), &QuadRemeshOptions::count(200)).unwrap();
        assert!(subd.cage().face_count() > 100);
        assert_eq!(subd.creases().count(), 0, "a closed cage has no border");
    }

    #[test]
    fn an_empty_mesh_is_refused() {
        assert!(Mesh::default().quad_remeshed(&QuadRemeshOptions::count(10)).is_err());
    }
}
