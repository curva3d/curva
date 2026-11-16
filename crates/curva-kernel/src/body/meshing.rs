//! Meshing a body for drawing: its faces as made are meshed once and shared by its copies, then
//! placed.

use super::{Body, BodyMesh, failed, is_in_place};
use crate::{KernelError, Point};
use solidcore_tess::{MeshTolerance, plan};

impl Body {
    /// Triangles and edges for drawing, fine enough to look smooth at the body's size. The
    /// faces as made are meshed, then the mesh is placed.
    pub fn mesh(&self) -> Result<BodyMesh, KernelError> {
        let made = self.unplaced_mesh()?;
        if is_in_place(&self.placement) {
            return Ok(made.clone());
        }
        let point = |p: &Point| self.placed_point(*p);
        let direction = |d: &Point| self.placed_direction(*d);
        Ok(BodyMesh {
            positions: made.positions.iter().map(point).collect(),
            normals: made.normals.iter().map(direction).collect(),
            triangles: made.triangles.clone(),
            edges: made
                .edges
                .iter()
                .map(|run| run.iter().map(point).collect())
                .collect(),
            edge_ids: made.edge_ids.clone(),
            seams: made.seams.clone(),
            triangle_faces: made.triangle_faces.clone(),
        })
    }

    /// The faces as made, without the placement, meshed the first time it is asked for, then
    /// kept: picking an edge or drawing a moved copy meshes nothing again.
    fn unplaced_mesh(&self) -> Result<&BodyMesh, KernelError> {
        if self.shared.mesh.get().is_none() {
            let (low, high) = self.unplaced_bounds()?;
            let record = self.unplaced_record()?;
            let size = (0..3)
                .map(|axis| (high[axis] - low[axis]).powi(2))
                .sum::<f64>()
                .sqrt();
            let (tess, _) = plan(record, &MeshTolerance::display_of(size), size)
                .map_err(|e| failed("meshing a body", e))?;
            let (mesh, _) = tess.evaluate();
            let seams = seam_edges(&mesh, size);
            let kept: Vec<usize> = (0..mesh.edges.len())
                .filter(|index| !mesh.seams.get(*index).copied().unwrap_or(false))
                .collect();
            let point = |v: &solidcore_record::Vec3| [v.x, v.y, v.z];
            let _ = self.shared.mesh.set(BodyMesh {
                positions: mesh.positions.iter().map(point).collect(),
                normals: mesh.normals.iter().map(point).collect(),
                triangles: mesh.triangles.clone(),
                edges: kept
                    .iter()
                    .map(|&index| mesh.edges[index].iter().map(point).collect())
                    .collect(),
                seams: kept
                    .iter()
                    .map(|&index| seams.get(index).copied().unwrap_or(false))
                    .collect(),
                edge_ids: kept,
                triangle_faces: mesh.tri_face.iter().map(|&face| face as usize).collect(),
            });
        }
        self.shared
            .mesh
            .get()
            .ok_or_else(|| KernelError("meshing a body failed".to_owned()))
    }
}

/// Which of a mesh's edges are seams: where a single face meets itself, as along a sphere's
/// meridian or a cylinder's side, so that no line should be drawn. The engine does not mark these
/// itself (it lists such an edge with one face), and the boundary of an open surface is listed the
/// same way, so the two are told apart by the triangles: along a seam, that face's triangles lie
/// on both sides of the edge; along a boundary, on one side only. Points are compared within a
/// millionth of the body's `size`, as the mesh repeats a seam's points for each side.
fn seam_edges(mesh: &solidcore_tess::Mesh, size: f64) -> Vec<bool> {
    let step = (size * 1e-6).max(1e-12);
    let key = |v: &solidcore_record::Vec3| [v.x, v.y, v.z].map(|c| (c / step).round() as i64);
    // How many triangles of each face use each side, by its two ends in either order.
    let mut sides: std::collections::HashMap<([i64; 3], [i64; 3], u32), u32> =
        std::collections::HashMap::new();
    for (triangle, face) in mesh.triangles.iter().zip(&mesh.tri_face) {
        for k in 0..3 {
            let (a, b) = (triangle[k] as usize, triangle[(k + 1) % 3] as usize);
            let (Some(a), Some(b)) = (mesh.positions.get(a), mesh.positions.get(b)) else {
                continue;
            };
            let (a, b) = (key(a), key(b));
            let ends = if a <= b { (a, b) } else { (b, a) };
            *sides.entry((ends.0, ends.1, *face)).or_default() += 1;
        }
    }
    mesh.edges
        .iter()
        .enumerate()
        .map(|(index, run)| {
            let [face] = mesh.edge_faces.get(index).map_or(&[][..], Vec::as_slice) else {
                return false;
            };
            // A seam when, for most of its pieces, the face lies on both sides.
            let both = run
                .windows(2)
                .filter(|pair| {
                    let (a, b) = (key(&pair[0]), key(&pair[1]));
                    let ends = if a <= b { (a, b) } else { (b, a) };
                    sides.get(&(ends.0, ends.1, *face)).copied().unwrap_or(0) >= 2
                })
                .count();
            run.len() > 1 && both * 2 > run.len() - 1
        })
        .collect()
}
