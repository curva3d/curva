//! Joining meshes into one and exploding one into its pieces, as Rhino's `Join` and `Explode` do
//! for meshes: joining keeps every point as it is (nothing is welded), and a mesh's pieces are its
//! parts that share no point, so a mesh with unwelded edges comes apart along them.

use crate::{Attribute, Mesh, MeshError};

impl Mesh {
    /// The meshes as one, their points and faces in order, nothing welded. Normals, texture
    /// coordinates and colours are kept when every mesh has them: per point when every mesh keeps
    /// them per point, per corner otherwise.
    ///
    /// # Errors
    /// When there are so many points that they cannot be numbered. No meshes give an empty one.
    // CUSTOM MESH OPERATION: join-meshes
    pub fn joined(meshes: &[&Mesh]) -> Result<Self, MeshError> {
        let mut vertices = Vec::new();
        let mut faces: Vec<Vec<u32>> = Vec::new();
        for mesh in meshes {
            let start = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
            vertices.extend_from_slice(mesh.vertices());
            faces.extend(mesh.faces().map(|f| f.iter().map(|v| v + start).collect()));
        }
        let mut joined = Self::new(vertices, &faces)?;
        if let Some(normals) = combined(meshes, Mesh::normals) {
            joined = joined.with_normals(normals)?;
        }
        if let Some(texture) = combined(meshes, Mesh::texture) {
            joined = joined.with_texture(texture)?;
        }
        if let Some(colours) = combined(meshes, Mesh::colours) {
            joined = joined.with_colours(colours)?;
        }
        Ok(joined)
    }

    /// The parts of the mesh that share no point, each a mesh of its own with only its points; the
    /// mesh itself when it is one part. Parts are in the order of their first faces.
    // CUSTOM MESH OPERATION: explode-mesh
    #[must_use]
    pub fn pieces(&self) -> Vec<Self> {
        let part = self.part_of_each_face();
        let count = part.iter().max().map_or(0, |m| m + 1);
        if count <= 1 {
            return vec![self.clone()];
        }
        (0..count)
            .map(|k| {
                self.keeping_faces(|f| part[f] == k)
                    .0
                    .without_unused_vertices()
                    .0
            })
            .collect()
    }

    /// For each face, the number of its part: faces sharing a point are in one part, numbered in
    /// the order their first faces come.
    fn part_of_each_face(&self) -> Vec<usize> {
        let mut root: Vec<usize> = (0..self.vertex_count()).collect();
        fn find(root: &mut [usize], mut v: usize) -> usize {
            while root[v] != v {
                root[v] = root[root[v]];
                v = root[v];
            }
            v
        }
        for face in self.faces() {
            let first = find(&mut root, face[0] as usize);
            for &v in &face[1..] {
                let other = find(&mut root, v as usize);
                root[other] = first;
            }
        }
        let mut number = vec![usize::MAX; self.vertex_count()];
        let mut next = 0;
        self.faces()
            .map(|face| {
                let r = find(&mut root, face[0] as usize);
                if number[r] == usize::MAX {
                    number[r] = next;
                    next += 1;
                }
                number[r]
            })
            .collect()
    }
}

/// One attribute of several meshes as one, if every mesh has it.
fn combined<T: Clone + Default>(
    meshes: &[&Mesh],
    of: impl Fn(&Mesh) -> &Attribute<T>,
) -> Option<Attribute<T>> {
    if meshes.iter().any(|m| of(m).is_none()) {
        return None;
    }
    if meshes
        .iter()
        .all(|m| matches!(of(m), Attribute::PerVertex(_)))
    {
        let values = meshes
            .iter()
            .flat_map(|m| match of(m) {
                Attribute::PerVertex(v) => v.clone(),
                _ => Vec::new(),
            })
            .collect();
        return Some(Attribute::PerVertex(values));
    }
    let values = meshes
        .iter()
        .flat_map(|m| {
            m.corners()
                .iter()
                .enumerate()
                .map(|(c, &v)| of(m).at(c, v as usize).cloned().unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .collect();
    Some(Attribute::PerCorner(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    fn cube_at(x: f64) -> Mesh {
        let frame = Frame {
            origin: [x, 0.0, 0.0],
            ..Frame::world()
        };
        // Welded, so it is one piece (a MeshBox's sides are unwelded and explode apart).
        Mesh::mesh_box(&frame, [1.0; 3], [1, 1, 1]).welded(0.0).0
    }

    #[test]
    fn joining_keeps_every_point_and_face() {
        let (a, b) = (cube_at(0.0), cube_at(5.0));
        let joined = Mesh::joined(&[&a, &b]).unwrap();
        assert_eq!(joined.vertex_count(), a.vertex_count() + b.vertex_count());
        assert_eq!(joined.face_count(), 12);
        assert!((joined.volume().unwrap() - 2.0).abs() < 1e-12);
        assert!(!joined.normals().is_none());
    }

    #[test]
    fn exploding_gives_back_the_pieces() {
        let joined = Mesh::joined(&[&cube_at(0.0), &cube_at(5.0)]).unwrap();
        let pieces = joined.pieces();
        assert_eq!(pieces.len(), 2);
        for (piece, x) in pieces.iter().zip([0.0, 5.0]) {
            assert_eq!(piece.face_count(), 6);
            assert!((piece.volume().unwrap() - 1.0).abs() < 1e-12);
            let lowest = piece
                .vertices()
                .iter()
                .map(|p| p[0])
                .fold(f64::MAX, f64::min);
            assert!((lowest - x).abs() < 1e-12);
        }
    }

    #[test]
    fn a_mesh_of_one_piece_stays_whole() {
        let cube = cube_at(0.0);
        assert_eq!(cube.pieces(), vec![cube]);
    }

    /// Rhino 8's Explode splits a mesh at its unwelded edges (measured by the testing stream, 25
    /// November 2026): a MeshBox into its 6 sides of 121 points and 100 faces, a MeshCylinder
    /// into two caps of 11 and 10 and its side of 110 and 100.
    #[test]
    fn primitives_explode_at_their_unwelded_edges_as_rhino() {
        let counts = |mesh: &Mesh| -> Vec<(usize, usize)> {
            mesh.pieces()
                .iter()
                .map(|p| (p.vertex_count(), p.face_count()))
                .collect()
        };
        let world = Frame::world();
        let box_ = Mesh::mesh_box(&world, [10.0; 3], [10, 10, 10]);
        assert_eq!(counts(&box_), vec![(121, 100); 6]);
        let cylinder = Mesh::mesh_cylinder(&world, 3.0, 10.0, (10, 10), true, false);
        let mut pieces = counts(&cylinder);
        pieces.sort_unstable();
        assert_eq!(pieces, vec![(11, 10), (11, 10), (110, 100)]);
    }

    #[test]
    fn data_kept_by_only_some_meshes_is_dropped_and_mixed_data_goes_per_corner() {
        let plain = Mesh::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[[0, 1, 2]],
        )
        .unwrap();
        let cube = cube_at(5.0);
        assert!(Mesh::joined(&[&plain, &cube]).unwrap().normals().is_none());
        let flat = plain
            .clone()
            .with_normals(Attribute::PerCorner(vec![[0.0, 0.0, 1.0]; 3]))
            .unwrap();
        let joined = Mesh::joined(&[&flat, &cube]).unwrap();
        assert!(
            matches!(joined.normals(), Attribute::PerCorner(n) if n.len() == joined.corners().len())
        );
    }

    #[test]
    fn nothing_to_join_gives_an_empty_mesh() {
        assert_eq!(Mesh::joined(&[]).unwrap().face_count(), 0);
    }
}
