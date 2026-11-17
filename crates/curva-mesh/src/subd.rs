//! The SubD object: the cage the user edits, from which the smooth surface is worked out.
//!
//! Only the cage's points and faces for now, so the document can carry SubD objects (issue #266).
//! Edge and vertex tags, creases and the smooth surface from subdiv-kernels come in Stage B of
//! `docs/specs/mesh-and-subd.md`.

use crate::{Mesh, Point};
use curva_kernel::Similarity;
use serde::{Deserialize, Serialize};

/// A SubD: its cage, a polygon mesh whose faces may have any number of corners.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubD {
    cage: Mesh,
}

impl SubD {
    /// A SubD with this cage.
    #[must_use]
    pub fn new(cage: Mesh) -> Self {
        Self { cage }
    }

    /// The cage.
    #[must_use]
    pub fn cage(&self) -> &Mesh {
        &self.cage
    }

    /// The lowest and highest corners around its cage, which contains the smooth surface.
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        self.cage.bounds()
    }

    /// The SubD moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        Self {
            cage: self.cage.translated(by),
        }
    }

    /// The SubD turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        Self {
            cage: self.cage.mapped(change),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    #[test]
    fn a_subd_moves_with_its_cage() {
        let subd = SubD::new(cube()).translated([0.0, 0.0, 5.0]);
        assert_eq!(subd.bounds(), Some(([0.0, 0.0, 5.0], [1.0, 1.0, 6.0])));
        assert_eq!(subd.cage().face_count(), 6);
    }
}
