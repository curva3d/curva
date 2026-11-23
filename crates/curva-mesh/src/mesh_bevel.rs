//! Rhino 8's Bevel for mesh edges. Measured (23 November 2026, RhinoCommon picks of topology
//! edges, `-Bevel 0.5` with its starting options Segments=3, OffsetMode=Absolute,
//! Straightness=0, WeldAngle=30, RetainShape=No) on a welded 4 m mesh box of 2 × 2 × 2 faces:
//!
//! - the shape is the SubD Bevel's (32 places, Rhino's rows, spaced along their curves), each
//!   five-sided cap a fan of triangles from its far point: 27 quads and 6 triangles;
//! - the changed faces' edges sharper than WeldAngle are unwelded, each copy with the normal of
//!   its side as UnweldEdge gives (at the end of a run along the box's bottom-left edge:
//!   (−0.894, 0, −0.447) and (−0.447, 0, −0.894)).
//!
//! Rhino left one end of such a run welded (46 vertices where this gives 47, at (204, 2, 0)); why
//! is not known. Rhino also groups the faces of each changed face into an n-gon, which Curva's
//! meshes do not keep. RetainShape is not here.

use crate::subd_bevel::bevel_work;
use crate::triangulate::newell_normal;
use crate::{BevelAmount, Mesh, MeshError, Point};
use std::collections::BTreeMap;

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// The angle between two face normals, in degrees.
fn degrees_between(a: Point, b: Point) -> f64 {
    let dot: f64 = (0..3).map(|k| a[k] * b[k]).sum();
    let len = |p: Point| (0..3).map(|k| p[k] * p[k]).sum::<f64>().sqrt();
    (dot / (len(a) * len(b)).max(f64::MIN_POSITIVE))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

impl Mesh {
    /// A run of edges bevelled, as Rhino's Bevel bevels a mesh (see the module notes): `run`
    /// lists its points in order (a loop repeats its first at the end), as for
    /// [`crate::SubD::bevelled_run`]; vertices at one place count as one. Afterwards the changed
    /// faces' edges sharper than `weld_angle` degrees are unwelded.
    ///
    /// # Errors
    /// When a point is not an inner one where four edges meet, or the result is not a valid mesh.
    pub fn bevelled_run(
        &self,
        run: &[u32],
        amount: BevelAmount,
        segments: u32,
        straightness: f64,
        weld_angle: f64,
    ) -> Result<Self, MeshError> {
        let (welded, _) = self.welded(0.0);
        let at = |v: u32| {
            let p = self.vertices()[v as usize];
            welded
                .vertices()
                .iter()
                .position(|&q| q == p)
                .map_or(u32::MAX, index)
        };
        let run: Vec<u32> = run.iter().map(|&v| at(v)).collect();
        let mut uses: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
        for (f, ring) in welded.faces().enumerate() {
            for (k, &a) in ring.iter().enumerate() {
                let b = ring[(k + 1) % ring.len()];
                uses.entry((a.min(b), a.max(b))).or_default().push(f);
            }
        }
        let (work, added, gone) = bevel_work(&welded, &uses, &run, amount, segments, straightness)?;
        let mut changed: Vec<bool> = work
            .rings
            .iter()
            .zip(welded.faces())
            .map(|(now, was)| now.as_slice() != was)
            .collect();
        let mut rings = work.rings;
        for face in added {
            if face.len() > 4 {
                // A cap: a fan of triangles from its far point, its last.
                let far = face[face.len() - 1];
                rings.extend((0..face.len() - 2).map(|k| vec![face[k], face[k + 1], far]));
                changed.extend(std::iter::repeat_n(true, face.len() - 2));
            } else {
                rings.push(face);
                changed.push(true);
            }
        }
        let mut number: Vec<Option<u32>> = vec![None; work.points.len()];
        let mut kept: Vec<Point> = Vec::new();
        for (v, p) in work.points.iter().enumerate() {
            if !gone.contains(&index(v)) {
                number[v] = Some(index(kept.len()));
                kept.push(*p);
            }
        }
        let rings: Vec<Vec<u32>> = rings
            .iter()
            .map(|ring| ring.iter().filter_map(|&p| number[p as usize]).collect())
            .collect();
        let bevelled = Mesh::new(kept, &rings)?.with_rebuilt_normals();
        let sharp = sharp_edges(&bevelled, &changed, weld_angle);
        if sharp.is_empty() {
            return Ok(bevelled);
        }
        bevelled.with_edges_unwelded(&sharp, true)
    }
}

/// The edges of the `changed` faces whose two faces meet at more than `weld_angle` degrees.
fn sharp_edges(mesh: &Mesh, changed: &[bool], weld_angle: f64) -> Vec<[u32; 2]> {
    let normals: Vec<Point> = mesh
        .faces()
        .map(|ring| {
            let points: Vec<Point> = ring.iter().map(|&v| mesh.vertices()[v as usize]).collect();
            newell_normal(&points)
        })
        .collect();
    let mut faces_of: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (f, ring) in mesh.faces().enumerate() {
        for (k, &a) in ring.iter().enumerate() {
            let b = ring[(k + 1) % ring.len()];
            faces_of.entry((a.min(b), a.max(b))).or_default().push(f);
        }
    }
    faces_of
        .into_iter()
        .filter(|(_, faces)| {
            faces.len() == 2
                && (changed[faces[0]] || changed[faces[1]])
                && degrees_between(normals[faces[0]], normals[faces[1]]) > weld_angle
        })
        .map(|((a, b), _)| [a, b])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Attribute;

    fn mesh_box() -> Mesh {
        let frame = crate::Frame {
            origin: [200.0, 0.0, 0.0],
            ..crate::Frame::world()
        };
        crate::SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])
            .cage()
            .clone()
            .with_rebuilt_normals()
    }

    fn normals_at(mesh: &Mesh, p: Point) -> Vec<[f64; 3]> {
        let Attribute::PerVertex(n) = mesh.normals() else {
            panic!("per-vertex normals")
        };
        let mut out: Vec<[f64; 3]> = (0..mesh.vertex_count())
            .filter(|&v| mesh.vertices()[v] == p)
            .map(|v| n[v].map(|c| (c * 1e4).round() / 1e4 + 0.0))
            .collect();
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }

    #[test]
    #[allow(clippy::approx_constant)] // normals rounded to 4 places, as compared
    fn a_front_edge_bevels_as_rhino_bevels_a_mesh() {
        let mesh = mesh_box();
        let at = |p: Point| index(mesh.vertices().iter().position(|&q| q == p).unwrap());
        let edge = [at([202.0, 0.0, 0.0]), at([202.0, 0.0, 2.0])];
        let out = mesh
            .bevelled_run(&edge, BevelAmount::Absolute(0.5), 3, 0.0, 30.0)
            .unwrap();
        let quads = out.faces().filter(|f| f.len() == 4).count();
        let triangles = out.faces().filter(|f| f.len() == 3).count();
        assert_eq!((quads, triangles), (27, 6), "Rhino's 33 faces");
        assert_eq!(out.welded(0.0).0.vertex_count(), 32, "Rhino's 32 places");
        assert_eq!(
            normals_at(&out, [200.0, 2.0, 0.0]),
            vec![[-0.8944, 0.0, -0.4472], [-0.4472, 0.0, -0.8944]]
        );
        assert_eq!(
            normals_at(&out, [200.0, 0.0, 4.0]),
            vec![[-0.7071, 0.0, 0.7071], [0.0, -1.0, 0.0]]
        );
        assert_eq!(normals_at(&out, [200.0, 0.0, 0.0]).len(), 3);
    }
}
