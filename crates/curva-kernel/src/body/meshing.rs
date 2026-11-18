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
        Ok(self.placed_mesh(made))
    }

    /// Triangles for a mesh object (Rhino's `Mesh` command): facets within `chord` of the
    /// surface, in the model's units, none turning more than `angle` radians (`None`: the chord
    /// alone). Made afresh each time; the display mesh ([`Body::mesh`]) is left as it is.
    ///
    /// # Errors
    ///
    /// When the engine cannot describe or mesh the faces.
    pub fn mesh_with(&self, chord: f64, angle: Option<f64>) -> Result<BodyMesh, KernelError> {
        // Meshed as made, where lengths are the placement's scale smaller.
        let tolerance = MeshTolerance {
            chord: chord / self.scale(),
            angle,
        };
        let made = self.meshed_as_made(&tolerance)?;
        if is_in_place(&self.placement) {
            return Ok(made);
        }
        Ok(self.placed_mesh(&made))
    }

    /// A mesh of the faces as made, placed where the body is.
    fn placed_mesh(&self, made: &BodyMesh) -> BodyMesh {
        let point = |p: &Point| self.placed_point(*p);
        let direction = |d: &Point| self.placed_direction(*d);
        BodyMesh {
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
        }
    }

    /// The faces as made, without the placement, meshed the first time it is asked for, then
    /// kept: picking an edge or drawing a moved copy meshes nothing again.
    fn unplaced_mesh(&self) -> Result<&BodyMesh, KernelError> {
        if self.shared.mesh.get().is_none() {
            let size = self.size_as_made()?;
            let made = self.meshed_as_made(&MeshTolerance::display_of(size))?;
            let _ = self.shared.mesh.set(Arc::new(made));
        }
        self.shared
            .mesh
            .get()
            .map(Arc::as_ref)
            .ok_or_else(|| KernelError("meshing a body failed".to_owned()))
    }
}

impl Body {
    /// The length of the diagonal of the box around the faces as made.
    fn size_as_made(&self) -> Result<f64, KernelError> {
        let (low, high) = self.unplaced_bounds()?;
        Ok((0..3)
            .map(|axis| (high[axis] - low[axis]).powi(2))
            .sum::<f64>()
            .sqrt())
    }

    /// The faces as made, meshed within `tolerance`.
    fn meshed_as_made(&self, tolerance: &MeshTolerance) -> Result<BodyMesh, KernelError> {
        let size = self.size_as_made()?;
        let record = self.unplaced_record()?;
        let (tess, _) = plan(record, tolerance, size).map_err(|e| failed("meshing a body", e))?;
        let (mesh, _) = tess.evaluate();
        let seams = seam_edges(&mesh, size);
        let kept: Vec<usize> = (0..mesh.edges.len())
            .filter(|index| !mesh.seams.get(*index).copied().unwrap_or(false))
            .collect();
        let point = |v: &solidcore_record::Vec3| [v.x, v.y, v.z];
        Ok(BodyMesh {
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
        })
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

    /// A block instance's map that turns, scales evenly, mirrors and moves shares the mesh and
    /// gives the same solid as the engine makes; a shear is remade by the engine.
    #[test]
    fn an_even_map_shares_the_mesh_and_a_shear_does_not() {
        let cube = Body::cuboid([1.0, 2.0, 3.0], [2.0, 3.0, 4.0]).unwrap();
        let (c, s) = (0.6 * 2.0, 0.8 * 2.0);
        // Turned about Z, doubled, mirrored in Z, moved.
        let even = [
            [c, s, 0.0, 0.0],
            [-s, c, 0.0, 0.0],
            [0.0, 0.0, -2.0, 0.0],
            [5.0, 6.0, 7.0, 1.0],
        ];
        let placed = cube.transformed(even).unwrap();
        assert!(Arc::ptr_eq(
            &cube.shared_mesh().unwrap(),
            &placed.shared_mesh().unwrap()
        ));
        assert!((placed.volume().unwrap() - 8.0 * 24.0).abs() < 1e-6);
        // The box's eight corners, mapped by hand, bound the placed solid.
        let (mut p, mut q) = ([f64::MAX; 3], [f64::MIN; 3]);
        for corner in 0..8 {
            let v = [
                [1.0, 3.0][corner & 1],
                [2.0, 5.0][(corner >> 1) & 1],
                [3.0, 7.0][corner >> 2],
            ];
            for j in 0..3 {
                let mapped = (0..3).map(|k| v[k] * even[k][j]).sum::<f64>() + even[3][j];
                p[j] = p[j].min(mapped);
                q[j] = q[j].max(mapped);
            }
        }
        let (a, b) = placed.bounds().unwrap();
        for axis in 0..3 {
            assert!((a[axis] - p[axis]).abs() < 1e-6 && (b[axis] - q[axis]).abs() < 1e-6);
        }
        let shear = [
            [1.0, 0.0, 0.0, 0.0],
            [0.5, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let sheared = cube.transformed(shear).unwrap();
        assert!(!Arc::ptr_eq(
            &cube.shared_mesh().unwrap(),
            &sheared.shared_mesh().unwrap()
        ));
        assert!((sheared.volume().unwrap() - 24.0).abs() < 1e-6);
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

    #[test]
    fn a_mesh_at_a_chosen_tolerance_keeps_within_it_and_leaves_the_display_mesh() {
        let sphere = Body::sphere([0.0; 3], 10.0).unwrap();
        let display = sphere.mesh().unwrap();
        let farthest = |mesh: &crate::BodyMesh, centre: crate::Point, radius: f64| {
            mesh.positions
                .iter()
                .map(|p| (crate::gap(*p, centre) - radius).abs())
                .fold(0.0, f64::max)
        };
        // Facets sag inwards from the surface; their corners lie on it.
        let fine = sphere.mesh_with(0.01, None).unwrap();
        let coarse = sphere.mesh_with(1.0, None).unwrap();
        assert!(fine.triangles.len() > 4 * coarse.triangles.len());
        assert!(farthest(&fine, [0.0; 3], 10.0) < 1e-6);
        // A doubled, moved copy is meshed to the same tolerance in the model's units.
        let doubled = sphere
            .mapped(&Similarity::scaling([0.0; 3], 2.0))
            .translated([100.0, 0.0, 0.0]);
        let copy = doubled.mesh_with(0.01, None).unwrap();
        assert!(farthest(&copy, [100.0, 0.0, 0.0], 20.0) < 1e-6);
        assert!(copy.triangles.len() > fine.triangles.len());
        // The display mesh is as it was.
        assert_eq!(sphere.mesh().unwrap(), display);
    }
}
