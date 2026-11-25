//! Moving what is picked below the object, as Rhino moves sub-objects: picked control points,
//! parts of meshes and SubDs, and the corners, edges and faces of surfaces and solids, all as
//! one set of points that the gumball, `Move`, `Rotate` and `Scale` change. Which corners are
//! picked is not saved and not part of undo.

use crate::{Document, MeshPart, Shape};
use curva_kernel::{KernelError, Point};

/// One corner of a surface or solid: the object it belongs to and its number among that body's
/// corners (as [`curva_kernel::Body::corner_points`] numbers them).
pub type CornerPick = (u64, usize);

/// The points of `mesh` that `parts` move: a point itself, an edge's two ends, a face's corners.
fn mesh_points(mesh: &curva_mesh::Mesh, parts: &[MeshPart]) -> Vec<u32> {
    let faces: Vec<&[u32]> = mesh.faces().collect();
    let mut points = Vec::new();
    for part in parts {
        match *part {
            MeshPart::Point(p) => points.push(p),
            MeshPart::Edge(a, b) => points.extend([a, b]),
            MeshPart::Face(f) => {
                points.extend(faces.get(f as usize).into_iter().flat_map(|f| f.iter()))
            }
        }
    }
    points.sort_unstable();
    points.dedup();
    points
}

impl Document {
    /// The picked corners of surfaces and solids, leaving out those of objects that are gone or
    /// no longer bodies.
    #[must_use]
    pub fn corner_selection(&self) -> Vec<CornerPick> {
        self.corner_selection
            .iter()
            .copied()
            .filter(|(id, _)| {
                self.state
                    .objects
                    .iter()
                    .any(|o| o.id == *id && o.shape.body().is_some())
            })
            .collect()
    }

    /// Replaces the picked corners.
    pub fn select_corners(&mut self, corners: &[CornerPick]) {
        self.corner_selection = corners.to_vec();
    }

    /// Whether anything below the object is picked: control points, parts of meshes and SubDs,
    /// or corners, edges and faces of bodies. Then the gumball and the transform commands change
    /// those rather than whole objects, as in Rhino.
    #[must_use]
    pub fn has_picked_parts(&self) -> bool {
        !self.grip_selection().is_empty()
            || !self.mesh_part_selection().is_empty()
            || !self.corner_selection().is_empty()
            || !self.edge_selection().is_empty()
            || !self.face_selection().is_empty()
    }

    /// The objects that picked control points and parts belong to, each once, in order.
    #[must_use]
    pub fn picked_part_owners(&self) -> Vec<u64> {
        let mut owners: Vec<u64> = self
            .grip_selection()
            .iter()
            .map(|(id, _)| *id)
            .chain(self.mesh_part_selection().iter().map(|(id, _)| *id))
            .chain(self.corner_selection().iter().map(|(id, _)| *id))
            .chain(self.edge_selection().iter().map(|(id, _)| *id))
            .chain(self.face_selection().iter().map(|(id, _)| *id))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }

    /// Lets go of everything picked below the object.
    pub fn clear_picked_parts(&mut self) {
        self.select_grips(&[]);
        self.select_mesh_parts(&[]);
        self.select_corners(&[]);
        self.select_edges(&[]);
        self.select_faces(&[]);
    }

    /// The objects with picked parts, each with the numbers of the points those parts move:
    /// a mesh's or SubD cage's points, or a body's corners.
    fn moving_points(&self) -> Result<Vec<(u64, Vec<usize>)>, KernelError> {
        let mut owners: Vec<u64> = self
            .mesh_part_selection()
            .iter()
            .map(|(id, _)| *id)
            .chain(self.corner_selection().iter().map(|(id, _)| *id))
            .chain(self.edge_selection().iter().map(|(id, _)| *id))
            .chain(self.face_selection().iter().map(|(id, _)| *id))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        let mut moving = Vec::new();
        for id in owners {
            let Some(object) = self.state.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            let of = |list: Vec<(u64, usize)>| -> Vec<usize> {
                list.into_iter()
                    .filter(|(o, _)| *o == id)
                    .map(|(_, k)| k)
                    .collect()
            };
            let points = match &object.shape {
                Shape::Body(body) => {
                    let mut corners = of(self.corner_selection());
                    corners.extend(body.corners_of_edges(&of(self.edge_selection()))?);
                    corners.extend(body.corners_of_faces(&of(self.face_selection()))?);
                    corners.sort_unstable();
                    corners.dedup();
                    corners
                }
                Shape::Mesh(_) | Shape::SubD(_) => {
                    let parts: Vec<MeshPart> = self
                        .mesh_part_selection()
                        .into_iter()
                        .filter(|(o, _)| *o == id)
                        .map(|(_, part)| part)
                        .collect();
                    let mesh = match &object.shape {
                        Shape::Mesh(mesh) => mesh,
                        Shape::SubD(subd) => subd.cage(),
                        _ => unreachable!(),
                    };
                    mesh_points(mesh, &parts)
                        .into_iter()
                        .map(|p| p as usize)
                        .collect()
                }
                _ => continue,
            };
            moving.push((id, points));
        }
        Ok(moving)
    }

    /// Where every picked point is: control points, and the points that picked parts move.
    /// The gumball sits at their middle.
    pub fn picked_part_points(&self) -> Result<Vec<Point>, KernelError> {
        let grips = self.grip_selection();
        let mut points: Vec<Point> = self
            .shown_grips()
            .into_iter()
            .filter(|(grip, _)| grips.contains(grip))
            .map(|(_, at)| at)
            .collect();
        for (id, moving) in self.moving_points()? {
            let Some(object) = self.state.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            let all: Vec<Point> = match &object.shape {
                Shape::Body(body) => body.corner_points()?,
                Shape::Mesh(mesh) => mesh.vertices().to_vec(),
                Shape::SubD(subd) => subd.cage().vertices().to_vec(),
                _ => continue,
            };
            points.extend(moving.iter().filter_map(|k| all.get(*k).copied()));
        }
        Ok(points)
    }

    /// Each object with picked points, reshaped with those points moved by `change`: curves by
    /// their control points, meshes and SubDs by their points, surfaces and solids by their
    /// corners with their faces following (Rhino's sub-object editing).
    pub fn picked_parts_reshaped(
        &self,
        change: impl Fn(Point) -> Point,
    ) -> Result<Vec<(u64, Shape)>, KernelError> {
        let mut made = self.grips_reshaped(&self.grip_selection(), &change)?;
        for (id, moving) in self.moving_points()? {
            let Some(object) = self.state.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            let moved = |all: &[Point]| -> Vec<(u32, Point)> {
                moving
                    .iter()
                    .filter_map(|&k| Some((u32::try_from(k).ok()?, change(*all.get(k)?))))
                    .collect()
            };
            let shape = match &object.shape {
                Shape::Body(body) => Shape::Body(body.with_corners_moved(&moving, &change)?),
                Shape::Mesh(mesh) => Shape::Mesh(mesh.with_points_moved(&moved(mesh.vertices()))),
                Shape::SubD(subd) => {
                    Shape::SubD(subd.with_cage_points_moved(&moved(subd.cage().vertices())))
                }
                _ => continue,
            };
            made.retain(|(held, _)| *held != id);
            made.push((id, shape));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Document;
    use curva_kernel::Body;

    #[test]
    fn a_picked_face_moves_and_the_solid_follows() {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let body = doc.state().objects[0].shape.body().unwrap().clone();
        let corners = body.corner_points().unwrap();
        let top = (0..6)
            .find(|f| {
                body.corners_of_faces(&[*f])
                    .unwrap()
                    .iter()
                    .all(|c| corners[*c][2] > 0.5)
            })
            .unwrap();
        doc.select_faces(&[(id, top)]);
        assert!(doc.has_picked_parts());
        assert_eq!(doc.picked_part_points().unwrap().len(), 4);
        let made = doc
            .picked_parts_reshaped(|p| [p[0], p[1], p[2] + 2.0])
            .unwrap();
        let taller = made[0].1.body().unwrap();
        assert!((taller.volume().unwrap() - 3.0).abs() < 1e-6);
        doc.clear_picked_parts();
        assert!(!doc.has_picked_parts());
    }

    #[test]
    fn a_picked_corner_is_kept_only_while_its_object_is_a_body() {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select_corners(&[(id, 3)]);
        assert_eq!(doc.corner_selection(), [(id, 3)]);
        doc.remove(&[id]);
        assert!(doc.corner_selection().is_empty());
    }
}
