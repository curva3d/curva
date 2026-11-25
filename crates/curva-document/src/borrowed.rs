//! Edges of surfaces and solids standing in for curves: a command asking for curves takes edges
//! picked with Ctrl+Shift, as Rhino lets ExtrudeCrv or Loft use the edge of another object. Each
//! edge is lent to the command as a curve object of its own, and taken back when the command
//! ends, so only what the command made stays. Which are lent is not saved and not part of undo.

use crate::{Document, Shape};

impl Document {
    /// Lends the picked edges as curve objects and lets go of them as picked; returns the new
    /// objects. They go when [`Self::take_back_lent`] is called, at the end of the command.
    pub fn lend_picked_edges(&mut self) -> Vec<u64> {
        let picked = self.edge_selection();
        let mut lent = Vec::new();
        for (id, edge) in picked {
            let curve = self
                .state
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.body()?.edge_curve(edge).ok());
            if let Some(curve) = curve {
                lent.push(self.add_shape(Shape::Curve(curve)));
            }
        }
        self.select_edges(&[]);
        self.lent.extend(&lent);
        lent
    }

    /// Takes back the curves lent for picked edges, if they are still there.
    pub fn take_back_lent(&mut self) {
        let lent = std::mem::take(&mut self.lent);
        let still: Vec<u64> = lent
            .into_iter()
            .filter(|id| self.state.objects.iter().any(|o| o.id == *id))
            .collect();
        if !still.is_empty() {
            self.remove(&still);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Document;
    use curva_kernel::Body;

    #[test]
    fn a_picked_edge_is_lent_as_a_curve_and_taken_back() {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select_edges(&[(id, 0)]);
        let lent = doc.lend_picked_edges();
        assert_eq!(lent.len(), 1);
        assert!(
            doc.state()
                .objects
                .iter()
                .any(|o| o.id == lent[0] && o.curve().is_some())
        );
        assert!(doc.edge_selection().is_empty());
        doc.take_back_lent();
        assert_eq!(doc.state().objects.len(), 1);
    }
}
