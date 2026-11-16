//! Meshing a body for drawing: its faces as made are meshed once and shared by its copies, then
//! placed.

use super::{Body, BodyMesh, failed, is_in_place};
use crate::{KernelError, Point};
use solidcore_tess::{MeshTolerance, plan};
use std::sync::Arc;

impl Body {
    /// The faces as made, meshed, without the placement: one mesh shared by every copy of the
    /// body (moved, turned, scaled or mirrored), so a drawing engine can hold it once and draw
    /// each copy at its [`Body::placement`]. [`Body::mesh`] is this mesh, placed.
    pub fn shared_mesh(&self) -> Result<Arc<BodyMesh>, KernelError> {
        self.unplaced_mesh()?;
        self.shared
            .mesh
            .get()
            .cloned()
            .ok_or_else(|| KernelError("meshing a body failed".to_owned()))
    }

    /// Where this copy sits: the change from the faces as made (and [`Body::shared_mesh`]) to
    /// the body, as a 4 × 4 matrix for points written as rows, `p' = p·A + t`: rows 0 to 2 are
    /// where the X, Y and Z axes go, row 3 the move, and the last column `[0, 0, 0, 1]`.
    #[must_use]
    pub fn placement(&self) -> [[f64; 4]; 4] {
        self.placement
    }

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
            let _ = self.shared.mesh.set(Arc::new(BodyMesh {
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
            }));
        }
        self.shared
            .mesh
            .get()
            .map(Arc::as_ref)
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

#[cfg(test)]
mod tests {
    use super::Body;
    use crate::Similarity;
    use std::sync::Arc;

    /// Copies of a body, moved and turned, hand out the very same mesh, meshed once.
    #[test]
    fn copies_share_one_mesh() {
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let moved = cube.translated([10.0, 0.0, 0.0]);
        let turned = moved.mapped(&Similarity::rotation([1.0, 2.0, 0.0], [0.0, 0.0, 1.0], 0.7));
        let shared = cube.shared_mesh().unwrap();
        assert!(Arc::ptr_eq(&shared, &moved.shared_mesh().unwrap()));
        assert!(Arc::ptr_eq(&shared, &turned.shared_mesh().unwrap()));
    }

    /// The shared mesh, each point placed by `placement` as its rows say, is the body's own mesh.
    #[test]
    fn the_shared_mesh_placed_is_the_mesh() {
        let cube = Body::cuboid([1.0, 2.0, 3.0], [2.0, 3.0, 4.0]).unwrap();
        let turn = Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], 1.1);
        let copy = cube.translated([5.0, -7.0, 2.0]).mapped(&turn);
        let (shared, m) = (copy.shared_mesh().unwrap(), copy.placement());
        assert_eq!(m.map(|row| row[3]), [0.0, 0.0, 0.0, 1.0]);
        let placed = copy.mesh().unwrap();
        assert_eq!(shared.positions.len(), placed.positions.len());
        for (p, q) in shared.positions.iter().zip(&placed.positions) {
            let moved: [f64; 3] =
                std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j]);
            for axis in 0..3 {
                assert!(
                    (moved[axis] - q[axis]).abs() < 1e-9,
                    "{moved:?} against {q:?}"
                );
            }
        }
    }

    /// A body in place has the identity as its placement, and its mesh is the shared one.
    #[test]
    fn a_body_as_made_is_in_place() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        assert_eq!(cube.placement(), super::super::IN_PLACE);
        assert_eq!(*cube.shared_mesh().unwrap(), cube.mesh().unwrap());
    }
}
