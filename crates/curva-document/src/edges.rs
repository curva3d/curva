//! Edges and faces of surfaces and solids picked for commands such as FilletEdge and Shell.
//! Which are picked is not saved and not part of undo.

use crate::{Document, Shape};

/// One edge: the object it belongs to and its number among that body's edges.
pub type EdgePick = (u64, usize);

/// One face: the object it belongs to and its number among that body's faces.
pub type FacePick = (u64, usize);

impl Document {
    /// The picked faces, leaving out those of objects that are gone or no longer bodies.
    #[must_use]
    pub fn face_selection(&self) -> Vec<FacePick> {
        self.face_selection
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

    /// Replaces the picked faces.
    pub fn select_faces(&mut self, faces: &[FacePick]) {
        self.face_selection = faces.to_vec();
    }

    /// The picked edges, leaving out those of objects that are gone or no longer bodies.
    #[must_use]
    pub fn edge_selection(&self) -> Vec<EdgePick> {
        self.edge_selection
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

    /// Replaces the picked edges.
    pub fn select_edges(&mut self, edges: &[EdgePick]) {
        self.edge_selection = edges.to_vec();
    }

    /// Replaces an object's shape, keeping its id, layer and name.
    pub fn replace_shape(&mut self, id: u64, shape: Shape) {
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == id) {
            let object = std::sync::Arc::make_mut(object);
            object.shape = shape;
            object.recipe = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn edges_of_bodies_stay_picked_and_others_drop_out() {
        let mut doc = Document::default();
        let solid = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select_edges(&[(solid, 2), (line, 0)]);
        assert_eq!(doc.edge_selection(), [(solid, 2)]);
        doc.remove(&[solid]);
        assert!(doc.edge_selection().is_empty());
    }
}
