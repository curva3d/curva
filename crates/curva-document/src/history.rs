//! History: curves that remember how they were made and follow the curve they came from.

use crate::Document;
use curva_kernel::{Curve, Point, Similarity};
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
}

impl Recipe {
    fn source(&self) -> u64 {
        match self {
            Self::Mirror { source, .. } | Self::Offset { source, .. } => *source,
        }
    }

    fn make(&self, source: &Curve) -> Option<Curve> {
        match self {
            Self::Mirror { change, .. } => Some(source.mapped(change)),
            Self::Offset {
                normal,
                distance,
                side,
                ..
            } => source.offset_on_side(*normal, *distance, *side).ok(),
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

    /// Remakes every object whose source has changed. An object whose source has gone forgets its
    /// recipe and stays as it is. A source that can no longer give a result leaves the object as
    /// it was. Objects are in order of making, so a chain of recipes updates in one pass.
    pub fn refresh_history(&mut self) {
        self.refresh_curve_history();
        self.refresh_live();
    }

    /// Remakes the curves made with History from the curves they follow.
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
                .and_then(|object| object.curve().cloned());
            match source {
                None => std::sync::Arc::make_mut(&mut self.state.objects[index]).recipe = None,
                Some(curve) => {
                    if let Some(made) = recipe.make(&curve) {
                        std::sync::Arc::make_mut(&mut self.state.objects[index]).shape =
                            crate::Shape::Curve(made);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
