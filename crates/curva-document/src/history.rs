//! History: objects that remember how they were made and follow the object they came from: a
//! curve from a curve (mirror, offset), a mesh from a surface or solid (Rhino's `Mesh`, #335).

use crate::{Document, Shape};
use curva_kernel::{Point, Similarity};
use serde::{Deserialize, Serialize};

/// How an object was made from another.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Recipe {
    /// A mirror image of `source`.
    Mirror {
        /// The object it follows.
        source: u64,
        /// The mirror.
        change: Similarity,
    },
    /// An offset of `source`.
    Offset {
        /// The object it follows.
        source: u64,
        /// The direction straight out of the plane it was offset on.
        normal: Point,
        /// How far away it is.
        distance: f64,
        /// Which side it is on: `1.0` for the left looking along the source.
        side: f64,
    },
    /// A mesh of the surface or solid `source` (Rhino's `Mesh`).
    MeshOf {
        /// The object it follows.
        source: u64,
        /// How fine, from 0 (coarse) to 1 (fine), as Rhino's slider
        /// (`curva_mesh::density_tolerances`).
        density: f64,
    },
}

impl Recipe {
    fn source(&self) -> u64 {
        match self {
            Self::Mirror { source, .. }
            | Self::Offset { source, .. }
            | Self::MeshOf { source, .. } => *source,
        }
    }

    /// Whether it can be made from `source`'s kind of shape: a curve for a mirror or offset, a
    /// surface or solid for a mesh.
    fn takes(&self, source: &Shape) -> bool {
        match self {
            Self::Mirror { .. } | Self::Offset { .. } => source.curve().is_some(),
            Self::MeshOf { .. } => source.body().is_some(),
        }
    }

    /// The object made from `source`, if it can be made.
    fn make(&self, source: &Shape) -> Option<Shape> {
        match self {
            Self::Mirror { change, .. } => Some(Shape::Curve(source.curve()?.mapped(change))),
            Self::Offset {
                normal,
                distance,
                side,
                ..
            } => source
                .curve()?
                .offset_on_side(*normal, *distance, *side)
                .ok()
                .map(Shape::Curve),
            Self::MeshOf { density, .. } => {
                let body = source.body()?;
                let (low, high) = body.bounds().ok()?;
                let size = (0..3)
                    .map(|k| (high[k] - low[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                let (chord, angle) = curva_mesh::density_tolerances(*density, size);
                let made = body.mesh_with(chord, Some(angle)).ok()?;
                Some(Shape::Mesh(curva_mesh::Mesh::from_body_mesh(&made)))
            }
        }
    }
}

impl Document {
    /// Whether new results should remember how they were made.
    #[must_use]
    pub fn recording_history(&self) -> bool {
        self.recording_history
    }

    /// Turns history recording on or off.
    pub fn set_recording_history(&mut self, on: bool) {
        self.recording_history = on;
    }

    /// Makes an object remember how it was made, if recording is on.
    pub fn record_recipe(&mut self, id: u64, recipe: Recipe) {
        if !self.recording_history {
            return;
        }
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == id) {
            std::sync::Arc::make_mut(object).recipe = Some(recipe);
        }
    }

    /// The recipe an object remembers, if any.
    #[must_use]
    pub fn recipe_of(&self, id: u64) -> Option<Recipe> {
        self.state
            .objects
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| o.recipe)
    }

    /// Remakes every object whose source has changed, then moves dimensions tied to objects that
    /// changed (`tie`). An object whose source has gone forgets its recipe and stays as it is. A
    /// source that can no longer give a result leaves the object as it was. Objects are in order
    /// of making, so a chain of recipes updates in one pass.
    pub fn refresh_history(&mut self) {
        self.refresh_curve_history();
        self.refresh_live();
        self.refresh_ties();
    }

    /// Remakes the objects made with History from the objects they follow.
    fn refresh_curve_history(&mut self) {
        for index in 0..self.state.objects.len() {
            let Some(recipe) = self.state.objects[index].recipe else {
                continue;
            };
            let source = self
                .state
                .objects
                .iter()
                .find(|object| object.id == recipe.source())
                .map(|object| object.shape.clone())
                .filter(|shape| recipe.takes(shape));
            match source {
                None => std::sync::Arc::make_mut(&mut self.state.objects[index]).recipe = None,
                Some(shape) => {
                    if let Some(made) = recipe.make(&shape) {
                        std::sync::Arc::make_mut(&mut self.state.objects[index]).shape = made;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::{Body, Curve};

    const X: Point = [1.0, 0.0, 0.0];

    fn line() -> Curve {
        Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap()
    }

    fn mirror() -> Similarity {
        Similarity::mirror([0.0; 3], X)
    }

    /// A line and its mirror image, with history on.
    fn with_child() -> (Document, u64, u64) {
        let mut doc = Document::default();
        doc.set_recording_history(true);
        let source = doc.add(line());
        let child = doc.copy_transformed(&[source], &mirror())[0];
        doc.record_recipe(
            child,
            Recipe::Mirror {
                source,
                change: mirror(),
            },
        );
        (doc, source, child)
    }

    fn start_of(doc: &Document, id: u64) -> Point {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.curve().unwrap().sample().unwrap()[0]
    }

    #[test]
    fn a_child_follows_its_source() {
        let (mut doc, source, child) = with_child();
        doc.translate(&[source], [3.0, 1.0, 0.0]);
        doc.refresh_history();
        assert_eq!(start_of(&doc, child), [-4.0, 1.0, 0.0]);
    }

    #[test]
    fn nothing_is_remembered_with_recording_off() {
        let mut doc = Document::default();
        let source = doc.add(line());
        let child = doc.copy_transformed(&[source], &mirror())[0];
        doc.record_recipe(
            child,
            Recipe::Mirror {
                source,
                change: mirror(),
            },
        );
        assert_eq!(doc.recipe_of(child), None);
    }

    #[test]
    fn deleting_the_source_leaves_the_child_and_forgets_the_recipe() {
        let (mut doc, source, child) = with_child();
        doc.remove(&[source]);
        doc.refresh_history();
        assert_eq!(doc.recipe_of(child), None);
        assert_eq!(start_of(&doc, child), [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn editing_the_child_breaks_its_history() {
        let (mut doc, source, child) = with_child();
        doc.translate(&[child], [0.0, 5.0, 0.0]);
        assert_eq!(doc.recipe_of(child), None);
        doc.translate(&[source], [9.0, 0.0, 0.0]);
        doc.refresh_history();
        assert_eq!(start_of(&doc, child), [-1.0, 5.0, 0.0]);
    }

    #[test]
    fn moving_source_and_child_together_keeps_the_child_where_it_was_moved() {
        let (mut doc, source, child) = with_child();
        let both = [source, child];
        doc.translate(&both, [0.0, 5.0, 0.0]);
        doc.refresh_history();
        assert_eq!(start_of(&doc, source), [1.0, 5.0, 0.0]);
        assert_eq!(start_of(&doc, child), [-1.0, 5.0, 0.0]);
    }

    #[test]
    fn a_chain_of_children_updates_in_one_pass() {
        let (mut doc, source, child) = with_child();
        let shift = Similarity::mirror([10.0, 0.0, 0.0], X);
        let grandchild = doc.copy_transformed(&[child], &shift)[0];
        doc.record_recipe(
            grandchild,
            Recipe::Mirror {
                source: child,
                change: shift,
            },
        );
        doc.translate(&[source], [1.0, 0.0, 0.0]);
        doc.refresh_history();
        assert_eq!(start_of(&doc, grandchild), [22.0, 0.0, 0.0]);
    }

    #[test]
    fn recipes_are_kept_in_files() {
        let (doc, _, child) = with_child();
        let text = serde_json::to_string(doc.state()).unwrap();
        let back: crate::State = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, doc.state());
        assert!(
            back.objects
                .iter()
                .any(|o| o.id == child && o.recipe.is_some())
        );
    }

    #[test]
    fn a_mesh_made_with_history_follows_its_solid() {
        let mut doc = Document::default();
        doc.set_recording_history(true);
        let source = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        let recipe = Recipe::MeshOf {
            source,
            density: 0.5,
        };
        let made = recipe.make(&doc.state().objects[0].shape.clone()).unwrap();
        let mesh = doc.add_shape(made);
        doc.record_recipe(mesh, recipe);
        let bounds = |doc: &Document| {
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == mesh)
                .and_then(|o| o.shape.mesh())
                .and_then(curva_mesh::Mesh::bounds)
                .unwrap()
        };
        assert_eq!(bounds(&doc), ([0.0; 3], [10.0, 10.0, 10.0]));
        // The solid grows; the mesh follows it.
        doc.replace_shape(
            source,
            Shape::Body(Body::cuboid([0.0; 3], [20.0, 10.0, 5.0]).unwrap()),
        );
        doc.refresh_history();
        assert_eq!(bounds(&doc), ([0.0; 3], [20.0, 10.0, 5.0]));
        assert!(doc.recipe_of(mesh).is_some());
        // A curve in the solid's place cannot be meshed: the mesh forgets its recipe and stays.
        doc.replace_shape(source, Shape::Curve(line()));
        doc.refresh_history();
        assert!(doc.recipe_of(mesh).is_none());
        assert_eq!(bounds(&doc), ([0.0; 3], [20.0, 10.0, 5.0]));
    }
}
