//! Parts of meshes and SubDs picked for commands such as Crease, InsertEdge and DeleteMeshFaces,
//! and for moving a SubD's cage points with the gumball (#333): points, edges and faces, numbered
//! as `curva_mesh::Mesh` and `SubD` number them (a SubD's by its cage). Which are picked is not
//! saved and not part of undo.

use crate::{Document, Shape};

/// A part of a mesh or a SubD's cage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeshPart {
    /// A point, by its number.
    Point(u32),
    /// An edge, by the numbers of its two points, the lower first.
    Edge(u32, u32),
    /// A face, by its number.
    Face(u32),
}

impl MeshPart {
    /// The edge between points `a` and `b`, either way round.
    #[must_use]
    pub fn edge(a: u32, b: u32) -> Self {
        Self::Edge(a.min(b), a.max(b))
    }
}

/// One picked part: the object it belongs to, and the part.
pub type MeshPick = (u64, MeshPart);

/// The mesh a shape is picked on: the mesh itself, or a SubD's cage.
fn picked_on(shape: &Shape) -> Option<&curva_mesh::Mesh> {
    match shape {
        Shape::Mesh(mesh) => Some(mesh),
        Shape::SubD(subd) => Some(subd.cage()),
        _ => None,
    }
}

/// Whether `part` is one `mesh` has.
fn has(mesh: &curva_mesh::Mesh, part: MeshPart) -> bool {
    let points = mesh.vertices().len();
    match part {
        MeshPart::Point(p) => (p as usize) < points,
        MeshPart::Face(f) => (f as usize) < mesh.faces().count(),
        MeshPart::Edge(a, b) => {
            a < b
                && (b as usize) < points
                && mesh.faces().any(|face| {
                    (0..face.len()).any(|k| {
                        let (p, q) = (face[k], face[(k + 1) % face.len()]);
                        (p.min(q), p.max(q)) == (a, b)
                    })
                })
        }
    }
}

impl Document {
    /// The picked parts of meshes and SubDs, leaving out those of objects that are gone, are no
    /// longer meshes or SubDs, or no longer have that part.
    #[must_use]
    pub fn mesh_part_selection(&self) -> Vec<MeshPick> {
        self.mesh_part_selection
            .iter()
            .copied()
            .filter(|(id, part)| {
                self.state
                    .objects
                    .iter()
                    .find(|o| o.id == *id)
                    .and_then(|o| picked_on(&o.shape))
                    .is_some_and(|mesh| has(mesh, *part))
            })
            .collect()
    }

    /// Replaces the picked parts of meshes and SubDs, each once, in order.
    pub fn select_mesh_parts(&mut self, parts: &[MeshPick]) {
        let mut seen = std::collections::BTreeSet::new();
        self.mesh_part_selection = parts.iter().copied().filter(|p| seen.insert(*p)).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_mesh::{Frame, Mesh, SubD};

    #[test]
    fn picked_parts_are_kept_only_while_their_object_has_them() {
        let mut doc = Document::default();
        // A welded cube of 8 points (a MeshBox keeps its edges unwelded, as Rhino's).
        let mesh = doc.add_shape(Shape::Mesh(
            Mesh::mesh_box(&Frame::world(), [1.0; 3], [1, 1, 1])
                .welded(0.0)
                .0,
        ));
        let subd = doc.add_shape(Shape::SubD(SubD::subd_box(
            &Frame::world(),
            [1.0; 3],
            [1, 1, 1],
        )));
        let curve = doc.add(curva_kernel::Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let first_edge = {
            let face: Vec<u32> = doc.state().objects[0]
                .shape
                .mesh()
                .unwrap()
                .faces()
                .next()
                .unwrap()
                .to_vec();
            MeshPart::edge(face[1], face[0])
        };
        doc.select_mesh_parts(&[
            (mesh, MeshPart::Point(0)),
            (mesh, MeshPart::Point(0)),
            (mesh, first_edge),
            (mesh, MeshPart::Face(5)),
            (subd, MeshPart::Point(7)),
            // Not there: no ninth point, no seventh face, an edge across a face, a curve.
            (mesh, MeshPart::Point(8)),
            (mesh, MeshPart::Face(6)),
            (mesh, MeshPart::edge(0, 6)),
            (curve, MeshPart::Point(0)),
        ]);
        assert_eq!(
            doc.mesh_part_selection(),
            [
                (mesh, MeshPart::Point(0)),
                (mesh, first_edge),
                (mesh, MeshPart::Face(5)),
                (subd, MeshPart::Point(7)),
            ]
        );
        // An edge is the same either way round.
        assert_eq!(MeshPart::edge(4, 2), MeshPart::Edge(2, 4));
        // Once the mesh is gone, so are its parts.
        doc.remove(&[mesh]);
        assert_eq!(doc.mesh_part_selection(), [(subd, MeshPart::Point(7))]);
    }
}
