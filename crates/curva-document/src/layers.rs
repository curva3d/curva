//! Layers: named groups of objects with a colour, shown or hidden and locked or not.

use crate::{Document, Layer};

impl Document {
    /// The layers, oldest first. There is always at least one.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.state.layers
    }

    /// The layer new objects go on.
    #[must_use]
    pub fn current_layer(&self) -> u64 {
        self.state.current_layer
    }

    /// The layer an object is on.
    #[must_use]
    pub fn layer_of(&self, object: u64) -> Option<u64> {
        self.state
            .objects
            .iter()
            .find(|o| o.id == object)
            .map(|o| o.layer)
    }

    fn layer_mut(&mut self, id: u64) -> Option<&mut Layer> {
        self.state.layers.iter_mut().find(|layer| layer.id == id)
    }

    /// Adds a layer and returns its id.
    pub fn add_layer(&mut self, name: String) -> u64 {
        let id = self.state.next_layer;
        self.state.next_layer += 1;
        self.state.layers.push(Layer {
            id,
            name,
            colour: crate::STANDARD_COLOUR,
            visible: true,
            locked: false,
        });
        id
    }

    /// Renames a layer.
    pub fn rename_layer(&mut self, id: u64, name: String) {
        if let Some(layer) = self.layer_mut(id) {
            layer.name = name;
        }
    }

    /// Sets a layer's colour.
    pub fn set_layer_colour(&mut self, id: u64, colour: [f32; 3]) {
        if let Some(layer) = self.layer_mut(id) {
            layer.colour = colour;
        }
    }

    /// Makes a layer the one new objects go on, showing and unlocking it if need be.
    pub fn set_current_layer(&mut self, id: u64) {
        if let Some(layer) = self.layer_mut(id) {
            layer.visible = true;
            layer.locked = false;
            self.state.current_layer = id;
        }
    }

    fn deselect_layer(&mut self, id: u64) {
        let on_it: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|object| object.layer == id)
            .map(|object| object.id)
            .collect();
        self.selection.retain(|object| !on_it.contains(object));
    }

    /// Shows or hides a layer. The current layer cannot be hidden.
    pub fn set_layer_visible(&mut self, id: u64, visible: bool) {
        if !visible && id == self.state.current_layer {
            return;
        }
        if let Some(layer) = self.layer_mut(id) {
            layer.visible = visible;
        }
        if !visible {
            self.deselect_layer(id);
        }
    }

    /// Locks or unlocks a layer. The current layer cannot be locked.
    pub fn set_layer_locked(&mut self, id: u64, locked: bool) {
        if locked && id == self.state.current_layer {
            return;
        }
        if let Some(layer) = self.layer_mut(id) {
            layer.locked = locked;
        }
        if locked {
            self.deselect_layer(id);
        }
    }

    /// Moves objects onto a layer.
    pub fn move_to_layer(&mut self, objects: &[u64], layer: u64) {
        if self.state.layers.iter().all(|l| l.id != layer) {
            return;
        }
        for object in &mut self.state.objects {
            if objects.contains(&object.id) {
                object.layer = layer;
            }
        }
    }

    /// Deletes a layer. Returns `false`, changing nothing, for the current layer or one in use.
    pub fn delete_layer(&mut self, id: u64) -> bool {
        let in_use = self.state.objects.iter().any(|object| object.layer == id);
        if id == self.state.current_layer || in_use {
            return false;
        }
        let before = self.state.layers.len();
        self.state.layers.retain(|layer| layer.id != id);
        self.state.layers.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    fn line() -> Curve {
        Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap()
    }

    #[test]
    fn a_new_model_has_one_layer_and_objects_go_on_it() {
        let mut doc = Document::default();
        assert_eq!(doc.layers().len(), 1);
        let id = doc.add(line());
        assert_eq!(doc.layer_of(id), Some(doc.current_layer()));
    }

    #[test]
    fn new_objects_go_on_the_current_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Walls".to_owned());
        doc.set_current_layer(layer);
        let id = doc.add(line());
        assert_eq!(doc.layer_of(id), Some(layer));
    }

    #[test]
    fn the_current_layer_cannot_be_hidden_or_locked() {
        let mut doc = Document::default();
        let current = doc.current_layer();
        doc.set_layer_visible(current, false);
        doc.set_layer_locked(current, true);
        assert!(doc.layers()[0].visible);
        assert!(!doc.layers()[0].locked);
    }

    #[test]
    fn making_a_hidden_layer_current_shows_it() {
        let mut doc = Document::default();
        let layer = doc.add_layer("A".to_owned());
        doc.set_layer_visible(layer, false);
        doc.set_current_layer(layer);
        assert!(doc.layers()[1].visible);
    }

    #[test]
    fn hiding_or_locking_a_layer_deselects_its_objects() {
        let mut doc = Document::default();
        let layer = doc.add_layer("A".to_owned());
        let id = doc.add(line());
        doc.move_to_layer(&[id], layer);
        doc.select(&[id]);
        doc.set_layer_visible(layer, false);
        assert!(doc.selection().is_empty());
        doc.set_layer_visible(layer, true);
        doc.select(&[id]);
        doc.set_layer_locked(layer, true);
        assert!(doc.selection().is_empty());
    }

    #[test]
    fn a_layer_in_use_or_current_cannot_be_deleted() {
        let mut doc = Document::default();
        let layer = doc.add_layer("A".to_owned());
        let id = doc.add(line());
        doc.move_to_layer(&[id], layer);
        assert!(!doc.delete_layer(layer));
        assert!(!doc.delete_layer(doc.current_layer()));
        doc.remove(&[id]);
        assert!(doc.delete_layer(layer));
        assert_eq!(doc.layers().len(), 1);
    }

    #[test]
    fn moving_to_a_layer_that_does_not_exist_changes_nothing() {
        let mut doc = Document::default();
        let id = doc.add(line());
        let before = doc.layer_of(id);
        doc.move_to_layer(&[id], 99);
        assert_eq!(doc.layer_of(id), before);
    }

    #[test]
    fn a_file_from_before_layers_opens_on_the_default_layer() {
        let text = r#"{"objects":[{"id":3,"curve":{"Line":{"start":[0.0,0.0,0.0],"end":[1.0,0.0,0.0]}}}],"next_id":4}"#;
        let state: crate::State = serde_json::from_str(text).unwrap();
        assert_eq!(state.layers.len(), 1);
        assert_eq!(state.objects[0].layer, state.current_layer);
    }

    #[test]
    fn layers_survive_being_saved_as_text() {
        let mut doc = Document::default();
        doc.add_layer("Walls".to_owned());
        let text = serde_json::to_string(doc.state()).unwrap();
        let back: crate::State = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, doc.state());
    }
}
