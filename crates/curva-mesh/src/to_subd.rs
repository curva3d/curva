//! ToSubD for meshes, as Rhino 8 makes a SubD from a mesh (spec `docs/specs/mesh-and-subd.md` §1,
//! Stage B), with its options `UseMesh`, `MeshCreases` and `MeshCorners`.

use crate::check::Topology;
use crate::{Mesh, Point, SmoothError, SubD};

/// Rhino's ToSubD options for a mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToSubdOptions {
    /// `UseMesh=Location`: the cage is placed so the smooth surface passes through the mesh's
    /// points; otherwise (`ControlPoints`) the mesh's points are the cage.
    pub through_points: bool,
    /// `MeshCreases`: unwelded edges (hard edges, where the faces on either side keep different
    /// normals) become creases.
    pub creases: bool,
    /// `MeshCorners`: a border point used by only one face becomes a corner.
    pub corners: bool,
}

impl Default for ToSubdOptions {
    /// The mesh's points as the cage, with its hard edges creased and its border corners kept.
    fn default() -> Self {
        Self {
            through_points: false,
            creases: true,
            corners: true,
        }
    }
}

impl SubD {
    /// A SubD from a mesh, as Rhino's ToSubD. Faces on edges shared by more than two faces are
    /// left out, as Rhino leaves them out; points at one place are joined.
    ///
    /// # Errors
    /// When nothing is left, or the cage cannot be fitted through the points.
    pub fn from_mesh(mesh: &Mesh, options: ToSubdOptions) -> Result<Self, SmoothError> {
        let topology = Topology::of(mesh);
        // Faces touching an edge with three or more faces go.
        let mut drop = vec![false; mesh.face_count()];
        for uses in topology.edges.values() {
            if uses.len() > 2 {
                for u in uses {
                    drop[u.face] = true;
                }
            }
        }
        // Hard edges: where the two faces see different normals at either end, or meet at
        // different points of one place (an unwelded edge).
        let mut hard: Vec<[Point; 2]> = Vec::new();
        if options.creases {
            for uses in topology.edges.values() {
                let [a, b] = uses[..] else { continue };
                let corner = |u: crate::check::EdgeUse, step: usize| {
                    let range = mesh.face_range(u.face);
                    range.start + (u.corner + step) % range.len()
                };
                // The same place at each end, seen from each face (edges run opposite ways).
                let (a0, a1) = (corner(a, 0), corner(a, 1));
                let (b0, b1) = (corner(b, 1), corner(b, 0));
                let differ = |c: usize, d: usize| {
                    let (vc, vd) = (mesh.corners()[c], mesh.corners()[d]);
                    vc != vd
                        || mesh.normals().at(c, vc as usize) != mesh.normals().at(d, vd as usize)
                };
                if !mesh.normals().is_none() && (differ(a0, b0) || differ(a1, b1))
                    || mesh.corners()[a0] != mesh.corners()[b0]
                    || mesh.corners()[a1] != mesh.corners()[b1]
                {
                    let at = |c: usize| mesh.vertices()[mesh.corners()[c] as usize];
                    hard.push([at(a0), at(a1)]);
                }
            }
        }
        let rings: Vec<Vec<u32>> = (0..mesh.face_count())
            .filter(|&f| !drop[f])
            .map(|f| mesh.face(f).to_vec())
            .collect();
        if rings.is_empty() {
            return Err(SmoothError(
                "the mesh has no faces to make a SubD of".to_owned(),
            ));
        }
        let cage = Mesh::new(mesh.vertices().to_vec(), &rings)
            .map_err(|e| SmoothError(e.to_string()))?
            .without_unused_vertices()
            .0;
        let mut subd = Self::new(cage);
        // Find the welded cage's points for the hard edges' places.
        let place_of = |p: Point, subd: &SubD| {
            subd.cage()
                .vertices()
                .iter()
                .position(|q| *q == p)
                .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        };
        let creases: Vec<[u32; 2]> = hard
            .iter()
            .filter_map(|[p, q]| Some([place_of(*p, &subd)?, place_of(*q, &subd)?]))
            .collect();
        subd = subd.with_creases(&creases);
        if options.corners {
            let mut faces_at = vec![0u32; subd.cage().vertex_count()];
            for &v in subd.cage().corners() {
                faces_at[v as usize] += 1;
            }
            let lone: Vec<u32> = (0..faces_at.len())
                .filter(|&v| faces_at[v] == 1)
                .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
                .collect();
            subd = subd.with_corners(&lone);
        }
        if options.through_points {
            let targets = subd.cage().vertices().to_vec();
            subd = subd.fitted(&targets)?;
        }
        Ok(subd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame, VertexTag};

    #[test]
    fn a_box_with_hard_edges_becomes_a_creased_subd_that_stays_a_box() {
        // The mesh box keeps a normal per corner: every edge is hard.
        let mesh = Mesh::mesh_box(&Frame::world(), [2.0; 3], [1, 1, 1]);
        let subd = SubD::from_mesh(&mesh, ToSubdOptions::default()).unwrap();
        assert_eq!(subd.creases().count(), 12);
        assert!((0..8).all(|v| subd.vertex_tag(v) == VertexTag::Corner));
        let volume = subd.surface(2).unwrap().volume().unwrap();
        assert!((volume - 8.0).abs() < 1e-5, "{volume}");
        let smooth = SubD::from_mesh(
            &mesh,
            ToSubdOptions {
                creases: false,
                ..ToSubdOptions::default()
            },
        )
        .unwrap();
        assert_eq!(smooth.creases().count(), 0);
    }

    #[test]
    fn a_smooth_sphere_mesh_has_no_creases() {
        let mesh = Mesh::mesh_sphere(
            &Frame::world(),
            1.0,
            crate::SphereStyle::Quads { subdivisions: 1 },
        );
        let subd = SubD::from_mesh(&mesh, ToSubdOptions::default()).unwrap();
        assert_eq!(subd.creases().count(), 0);
    }

    #[test]
    fn through_points_puts_the_surface_on_the_mesh_points() {
        let mesh = Mesh::mesh_sphere(
            &Frame::world(),
            1.0,
            crate::SphereStyle::Quads { subdivisions: 1 },
        );
        let control = SubD::from_mesh(&mesh, ToSubdOptions::default()).unwrap();
        let through = SubD::from_mesh(
            &mesh,
            ToSubdOptions {
                through_points: true,
                ..ToSubdOptions::default()
            },
        )
        .unwrap();
        let (_, small) = control.surface(5).unwrap().bounds().unwrap();
        let (_, whole) = through.surface(5).unwrap().bounds().unwrap();
        assert!(
            small[0] < 0.95,
            "control points: the surface shrinks inside ({})",
            small[0]
        );
        assert!(
            (whole[0] - 1.0).abs() < 1e-6,
            "through the points: it reaches them ({})",
            whole[0]
        );
    }

    #[test]
    fn border_corners_follow_the_option_and_non_manifold_faces_go() {
        let plane = Mesh::mesh_plane(&Frame::world(), [3.0, 3.0], [3, 3]);
        let with = SubD::from_mesh(&plane, ToSubdOptions::default()).unwrap();
        let corners = (0..16)
            .filter(|&v| with.vertex_tag(v) == VertexTag::Corner)
            .count();
        assert_eq!(corners, 4);
        let without = SubD::from_mesh(
            &plane,
            ToSubdOptions {
                corners: false,
                ..ToSubdOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            (0..16)
                .filter(|&v| without.vertex_tag(v) == VertexTag::Corner)
                .count(),
            0
        );
        // A third face on one of the plane's edges: the faces on that edge go.
        let mut points = plane.vertices().to_vec();
        points.push([0.5, 0.0, 1.0]);
        let mut faces: Vec<Vec<u32>> = plane.faces().map(<[u32]>::to_vec).collect();
        faces.push(vec![0, 1, 16]);
        faces.push(vec![1, 0, 5]);
        let messy = Mesh::new(points, &faces).unwrap();
        let cleaned = SubD::from_mesh(&messy, ToSubdOptions::default()).unwrap();
        assert!(cleaned.cage().non_manifold_edges().is_empty());
    }
}
