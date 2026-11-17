//! The polygon mesh: points and faces of three or more corners, with optional normals, texture
//! coordinates and colours.

use crate::Point;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Optional data a mesh carries for each vertex, or for each corner of each face.
///
/// Per-corner data lets one vertex have two values, which is how a hard edge (two normals) or a
/// texture seam (two texture coordinates) is stored, as Rhino's unwelded edges do.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Attribute<T> {
    /// Not stored.
    #[default]
    None,
    /// One value for each vertex, in vertex order.
    PerVertex(Vec<T>),
    /// One value for each face corner, in the order of [`Mesh::corners`].
    PerCorner(Vec<T>),
}

impl<T> Attribute<T> {
    /// Whether nothing is stored.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// The value at a corner, given the vertex it sits on.
    #[must_use]
    pub fn at(&self, corner: usize, vertex: usize) -> Option<&T> {
        match self {
            Self::None => None,
            Self::PerVertex(values) => values.get(vertex),
            Self::PerCorner(values) => values.get(corner),
        }
    }

    fn len_matches(&self, vertices: usize, corners: usize) -> bool {
        match self {
            Self::None => true,
            Self::PerVertex(values) => values.len() == vertices,
            Self::PerCorner(values) => values.len() == corners,
        }
    }

    fn map_values(&self, change: impl Fn(&T) -> T) -> Self {
        match self {
            Self::None => Self::None,
            Self::PerVertex(values) => Self::PerVertex(values.iter().map(change).collect()),
            Self::PerCorner(values) => Self::PerCorner(values.iter().map(change).collect()),
        }
    }
}

impl<T: Clone> Attribute<T> {
    /// The same data with each face's corners in reverse order (see [`Mesh::flipped`]).
    pub(crate) fn corners_reordered(&self, order: &[usize]) -> Self {
        match self {
            Self::PerCorner(values) => {
                Self::PerCorner(order.iter().map(|&i| values[i].clone()).collect())
            }
            other => other.clone(),
        }
    }

    pub(crate) fn mapped_with(&self, change: impl Fn(&T) -> T) -> Self {
        self.map_values(change)
    }
}

/// Why a mesh could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// A face has fewer than three corners.
    FaceTooSmall {
        /// Which face.
        face: usize,
    },
    /// A face names a vertex the mesh does not have.
    MissingVertex {
        /// Which face.
        face: usize,
        /// The vertex number it names.
        vertex: u32,
    },
    /// A face uses the same vertex at two of its corners.
    RepeatedVertex {
        /// Which face.
        face: usize,
    },
    /// A vertex position is not a finite number.
    NotFinite {
        /// Which vertex.
        vertex: usize,
    },
    /// Normals, texture coordinates or colours do not have one value per vertex or per corner.
    AttributeLength {
        /// Which data: "normals", "texture coordinates" or "colours".
        which: &'static str,
    },
}

// Messages for logs and reports to agents; the interface words its own from translation files.
impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FaceTooSmall { face } => write!(f, "face {face} has fewer than three corners"),
            Self::MissingVertex { face, vertex } => {
                write!(f, "face {face} names vertex {vertex}, which does not exist")
            }
            Self::RepeatedVertex { face } => write!(f, "face {face} uses one vertex twice"),
            Self::NotFinite { vertex } => write!(f, "vertex {vertex} is not a finite point"),
            Self::AttributeLength { which } => {
                write!(f, "the mesh's {which} do not match its vertices or corners")
            }
        }
    }
}

impl std::error::Error for MeshError {}

/// A polygon mesh: points, and faces of three or more corners between them.
///
/// Faces are stored one after another in [`Mesh::corners`], each ending where the next starts, so a
/// mesh of millions of faces is two flat lists rather than millions of small ones. Faces of more
/// than four corners (Rhino's "n-gons") are stored as they are and split into triangles only for
/// display and for the libraries that need triangles ([`Mesh::triangles`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    vertices: Vec<Point>,
    corners: Vec<u32>,
    /// Where each face's corners end in `corners`; face `i` is `face_ends[i - 1]..face_ends[i]`.
    face_ends: Vec<u32>,
    normals: Attribute<Point>,
    texture: Attribute<[f64; 2]>,
    colours: Attribute<[f32; 3]>,
}

impl Mesh {
    /// A mesh from its points and its faces, each face listing its vertices in order around it,
    /// counter-clockwise seen from the outside.
    ///
    /// # Errors
    /// When a face has fewer than three corners, names a vertex that does not exist or uses one
    /// twice, or a point is not finite.
    pub fn new<F: AsRef<[u32]>>(vertices: Vec<Point>, faces: &[F]) -> Result<Self, MeshError> {
        if let Some(vertex) = vertices
            .iter()
            .position(|p| p.iter().any(|c| !c.is_finite()))
        {
            return Err(MeshError::NotFinite { vertex });
        }
        let mut corners = Vec::with_capacity(faces.iter().map(|f| f.as_ref().len()).sum());
        let mut face_ends = Vec::with_capacity(faces.len());
        for (face, ring) in faces.iter().enumerate() {
            let ring = ring.as_ref();
            if ring.len() < 3 {
                return Err(MeshError::FaceTooSmall { face });
            }
            if let Some(&vertex) = ring.iter().find(|&&v| v as usize >= vertices.len()) {
                return Err(MeshError::MissingVertex { face, vertex });
            }
            if (1..ring.len()).any(|i| ring[..i].contains(&ring[i])) {
                return Err(MeshError::RepeatedVertex { face });
            }
            corners.extend_from_slice(ring);
            face_ends.push(index(corners.len()));
        }
        Ok(Self {
            vertices,
            corners,
            face_ends,
            ..Self::default()
        })
    }

    /// The same mesh with normals.
    ///
    /// # Errors
    /// When there is not one normal per vertex or per corner.
    pub fn with_normals(self, normals: Attribute<Point>) -> Result<Self, MeshError> {
        self.checked(&normals, "normals")?;
        Ok(Self { normals, ..self })
    }

    /// The same mesh with texture coordinates.
    ///
    /// # Errors
    /// When there is not one pair per vertex or per corner.
    pub fn with_texture(self, texture: Attribute<[f64; 2]>) -> Result<Self, MeshError> {
        self.checked(&texture, "texture coordinates")?;
        Ok(Self { texture, ..self })
    }

    /// The same mesh with colours (red, green and blue from 0 to 1).
    ///
    /// # Errors
    /// When there is not one colour per vertex or per corner.
    pub fn with_colours(self, colours: Attribute<[f32; 3]>) -> Result<Self, MeshError> {
        self.checked(&colours, "colours")?;
        Ok(Self { colours, ..self })
    }

    fn checked<T>(&self, data: &Attribute<T>, which: &'static str) -> Result<(), MeshError> {
        if data.len_matches(self.vertices.len(), self.corners.len()) {
            Ok(())
        } else {
            Err(MeshError::AttributeLength { which })
        }
    }

    /// Its points.
    #[must_use]
    pub fn vertices(&self) -> &[Point] {
        &self.vertices
    }

    /// Every face's vertices, one face after another; see [`Mesh::face`].
    #[must_use]
    pub fn corners(&self) -> &[u32] {
        &self.corners
    }

    /// How many points it has.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// How many faces it has.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.face_ends.len()
    }

    /// Where face `i`'s corners sit in [`Mesh::corners`].
    ///
    /// # Panics
    /// When there is no face `i`.
    #[must_use]
    pub fn face_range(&self, i: usize) -> std::ops::Range<usize> {
        let start = if i == 0 {
            0
        } else {
            self.face_ends[i - 1] as usize
        };
        start..self.face_ends[i] as usize
    }

    /// Face `i`'s vertices in order around it.
    ///
    /// # Panics
    /// When there is no face `i`.
    #[must_use]
    pub fn face(&self, i: usize) -> &[u32] {
        &self.corners[self.face_range(i)]
    }

    /// Every face's vertices, in order.
    pub fn faces(&self) -> impl Iterator<Item = &[u32]> {
        (0..self.face_count()).map(|i| self.face(i))
    }

    /// Its normals, if it has them.
    #[must_use]
    pub fn normals(&self) -> &Attribute<Point> {
        &self.normals
    }

    /// Its texture coordinates, if it has them.
    #[must_use]
    pub fn texture(&self) -> &Attribute<[f64; 2]> {
        &self.texture
    }

    /// Its colours, if it has them.
    #[must_use]
    pub fn colours(&self) -> &Attribute<[f32; 3]> {
        &self.colours
    }

    /// Each edge with how many faces use it, keyed by its two vertices, lower number first.
    fn edge_uses(&self) -> HashMap<(u32, u32), u32> {
        let mut uses = HashMap::with_capacity(self.corners.len());
        for face in self.faces() {
            for (k, &a) in face.iter().enumerate() {
                let b = face[(k + 1) % face.len()];
                *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        uses
    }

    /// How many edges it has (each shared edge counted once).
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edge_uses().len()
    }

    /// The edges used by only one face: the mesh's open borders.
    #[must_use]
    pub fn naked_edges(&self) -> Vec<[u32; 2]> {
        let mut naked: Vec<[u32; 2]> = self
            .edge_uses()
            .into_iter()
            .filter(|&(_, n)| n == 1)
            .map(|((a, b), _)| [a, b])
            .collect();
        naked.sort_unstable();
        naked
    }

    /// The edges used by three or more faces, which no real solid has.
    #[must_use]
    pub fn non_manifold_edges(&self) -> Vec<[u32; 2]> {
        let mut edges: Vec<[u32; 2]> = self
            .edge_uses()
            .into_iter()
            .filter(|&(_, n)| n > 2)
            .map(|((a, b), _)| [a, b])
            .collect();
        edges.sort_unstable();
        edges
    }

    /// Whether it encloses a volume: every edge shared by exactly two faces, as Rhino's "closed".
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.face_count() > 0 && self.edge_uses().values().all(|&n| n == 2)
    }

    /// The same mesh with every face turned to face the other way.
    #[must_use]
    pub fn flipped(&self) -> Self {
        let mut order = Vec::with_capacity(self.corners.len());
        for i in 0..self.face_count() {
            order.extend(self.face_range(i).rev());
        }
        let normals = self
            .normals
            .mapped_with(|n| n.map(|c| -c))
            .corners_reordered(&order);
        Self {
            vertices: self.vertices.clone(),
            corners: order.iter().map(|&i| self.corners[i]).collect(),
            face_ends: self.face_ends.clone(),
            normals,
            texture: self.texture.corners_reordered(&order),
            colours: self.colours.corners_reordered(&order),
        }
    }

    pub(crate) fn with_vertices(&self, vertices: Vec<Point>, normals: Attribute<Point>) -> Self {
        Self {
            vertices,
            normals,
            ..self.clone()
        }
    }
}

/// A list position as stored. Meshes are limited to about four billion corners, far beyond what
/// fits in memory as Curva objects.
fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A unit cube from the origin, faces counter-clockwise seen from outside.
    pub(crate) fn cube() -> Mesh {
        let v = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let faces: [[u32; 4]; 6] = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ];
        Mesh::new(v, &faces).unwrap()
    }

    #[test]
    fn a_cube_has_eight_vertices_six_faces_twelve_edges_and_is_closed() {
        let cube = cube();
        assert_eq!(
            (cube.vertex_count(), cube.face_count(), cube.edge_count()),
            (8, 6, 12)
        );
        assert!(cube.is_closed());
        assert!(cube.naked_edges().is_empty());
        assert_eq!(cube.face(1), &[4, 5, 6, 7]);
    }

    #[test]
    fn an_open_box_has_four_naked_edges() {
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(1).map(<[u32]>::to_vec).collect();
        let open = Mesh::new(cube.vertices().to_vec(), &faces).unwrap();
        assert!(!open.is_closed());
        assert_eq!(open.naked_edges(), vec![[0, 1], [0, 3], [1, 2], [2, 3]]);
    }

    #[test]
    fn three_faces_on_one_edge_are_non_manifold() {
        let v = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let mesh = Mesh::new(v, &[[0, 1, 2], [1, 0, 3], [0, 1, 4]]).unwrap();
        assert_eq!(mesh.non_manifold_edges(), vec![[0, 1]]);
    }

    #[test]
    fn bad_faces_are_refused() {
        let v = vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(
            Mesh::new(v.clone(), &[[0u32, 1]]),
            Err(MeshError::FaceTooSmall { face: 0 })
        );
        assert_eq!(
            Mesh::new(v.clone(), &[[0u32, 1, 7]]),
            Err(MeshError::MissingVertex { face: 0, vertex: 7 })
        );
        assert_eq!(
            Mesh::new(v.clone(), &[[0u32, 1, 1]]),
            Err(MeshError::RepeatedVertex { face: 0 })
        );
        let nan = vec![[0.0; 3], [f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(
            Mesh::new(nan, &[[0u32, 1, 2]]),
            Err(MeshError::NotFinite { vertex: 1 })
        );
    }

    #[test]
    fn attributes_must_match_vertices_or_corners() {
        let cube = cube();
        assert!(
            cube.clone()
                .with_colours(Attribute::PerVertex(vec![[1.0, 0.0, 0.0]; 8]))
                .is_ok()
        );
        assert!(
            cube.clone()
                .with_texture(Attribute::PerCorner(vec![[0.0, 0.0]; 24]))
                .is_ok()
        );
        assert_eq!(
            cube.with_normals(Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 7])),
            Err(MeshError::AttributeLength { which: "normals" })
        );
    }

    #[test]
    fn flipping_reverses_faces_and_their_corner_data() {
        let texture: Vec<[f64; 2]> = (0..24).map(|i| [f64::from(i), 0.0]).collect();
        let normals = Attribute::PerVertex(vec![[0.0, 0.0, 1.0]; 8]);
        let cube = cube()
            .with_texture(Attribute::PerCorner(texture))
            .unwrap()
            .with_normals(normals)
            .unwrap();
        let flipped = cube.flipped();
        assert_eq!(flipped.face(0), &[1, 2, 3, 0]);
        assert_eq!(flipped.texture().at(0, 1), Some(&[3.0, 0.0]));
        assert_eq!(flipped.normals().at(0, 0), Some(&[0.0, 0.0, -1.0]));
        assert_eq!(flipped.flipped(), cube);
    }

    #[test]
    fn a_mesh_survives_a_file_round_trip() {
        let cube = cube()
            .with_colours(Attribute::PerVertex(vec![[0.5, 0.25, 1.0]; 8]))
            .unwrap();
        let text = serde_json::to_string(&cube).unwrap();
        assert_eq!(serde_json::from_str::<Mesh>(&text).unwrap(), cube);
    }
}
