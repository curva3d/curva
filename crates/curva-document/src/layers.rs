//! Layers: named groups of objects with a colour, shown or hidden and locked or not.

#[allow(clippy::wildcard_imports)]
use crate::*;
use serde::{Deserialize, Serialize};

/// A named group of objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// Identifies the layer.
    pub id: u64,
    /// Its name.
    pub name: String,
    /// The colour of its objects, red, green and blue from 0 to 1.
    pub colour: [f32; 3],
    /// Whether its objects are shown.
    pub visible: bool,
    /// Whether its objects can be selected.
    pub locked: bool,
    /// How thick its objects are drawn and printed, in millimetres; `None` is the default thin line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_width: Option<f64>,
    /// How its objects' lines are drawn.
    #[serde(
        default = "continuous",
        skip_serializing_if = "Linetype::is_continuous"
    )]
    pub linetype: Linetype,
    /// The layer it is a sublayer of; `None` for a top-level layer (spec `sublayers.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    /// Whether its sublayers are folded away in the Layers panel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
    /// The section style of its objects that have none of their own; `None` leaves them to the
    /// clipping plane's (spec `layouts.md` §3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<crate::section::SectionStyle>,
}

/// The layer every model starts with.
pub(crate) fn standard_layers() -> Vec<Layer> {
    vec![Layer {
        id: 0,
        name: "Default".to_owned(),
        colour: STANDARD_COLOUR,
        visible: true,
        locked: false,
        print_width: None,
        linetype: Linetype::Continuous,
        parent: None,
        collapsed: false,
        section: None,
    }]
}

pub(crate) fn first_new_layer() -> u64 {
    1
}

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
            print_width: None,
            linetype: crate::Linetype::Continuous,
            parent: None,
            collapsed: false,
            section: None,
        });
        id
    }

    /// Adds a layer under `parent` and returns its id (spec `sublayers.md`).
    pub fn add_sublayer(&mut self, parent: u64, name: String) -> u64 {
        let id = self.add_layer(name);
        if self.state.layer(parent).is_some()
            && let Some(layer) = self.layer_mut(id)
        {
            layer.parent = Some(parent);
        }
        id
    }

    /// The layer `path` names (`Make2D::Visible::Curves`), making whichever layers down it are
    /// missing. The names are matched exactly, level by level.
    pub fn layer_at_path(&mut self, path: &str) -> u64 {
        let mut parent: Option<u64> = None;
        for name in path.split("::").map(str::trim).filter(|n| !n.is_empty()) {
            let found = self
                .state
                .layers
                .iter()
                .find(|l| l.parent == parent && l.name == name)
                .map(|l| l.id);
            parent = Some(found.unwrap_or_else(|| match parent {
                Some(above) => self.add_sublayer(above, name.to_owned()),
                None => self.add_layer(name.to_owned()),
            }));
        }
        parent.unwrap_or(self.state.current_layer)
    }

    /// Puts a layer under `parent`, or at the top for `None`. Returns `false`, changing nothing,
    /// when that would put a layer under itself or one of its own sublayers, or when its new
    /// level already has a layer of its name.
    pub fn set_layer_parent(&mut self, id: u64, parent: Option<u64>) -> bool {
        let Some(name) = self.state.layer(id).map(|l| l.name.clone()) else {
            return false;
        };
        if let Some(above) = parent
            && (self.state.layer(above).is_none() || self.state.layer_tree(id).contains(&above))
        {
            return false;
        }
        let taken = self
            .state
            .layers
            .iter()
            .any(|l| l.id != id && l.parent == parent && l.name.eq_ignore_ascii_case(&name));
        if taken {
            return false;
        }
        if let Some(layer) = self.layer_mut(id) {
            layer.parent = parent;
        }
        true
    }

    /// Folds a layer's sublayers away in the Layers panel, or opens them.
    pub fn set_layer_collapsed(&mut self, id: u64, collapsed: bool) {
        if let Some(layer) = self.layer_mut(id) {
            layer.collapsed = collapsed;
        }
    }

    /// Renames a layer.
    pub fn rename_layer(&mut self, id: u64, name: String) {
        if let Some(layer) = self.layer_mut(id) {
            layer.name = name;
        }
    }

    /// Sets how thick a layer's objects are drawn, in millimetres, or `None` for the default line.
    pub fn set_layer_print_width(&mut self, id: u64, width: Option<f64>) {
        if let Some(layer) = self.layer_mut(id) {
            layer.print_width = width.map(crate::PrintWidth::clamped);
        }
    }

    /// Sets how thick some objects are drawn.
    pub fn set_print_width(&mut self, ids: &[u64], width: crate::PrintWidth) {
        let width = match width {
            crate::PrintWidth::Millimetres(mm) => {
                crate::PrintWidth::Millimetres(crate::PrintWidth::clamped(mm))
            }
            other => other,
        };
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                std::sync::Arc::make_mut(object).print_width = width;
            }
        }
    }

    /// The width in millimetres an object draws at, or `None` for the default line.
    #[must_use]
    pub fn print_width_of(&self, id: u64) -> Option<f64> {
        let object = self.state.objects.iter().find(|object| object.id == id)?;
        let layer = self
            .state
            .layers
            .iter()
            .find(|layer| layer.id == object.layer);
        object.print_width.resolved(layer)
    }

    /// Sets a layer's colour.
    pub fn set_layer_colour(&mut self, id: u64, colour: [f32; 3]) {
        if let Some(layer) = self.layer_mut(id) {
            layer.colour = colour;
        }
    }

    /// Makes a layer the one new objects go on, showing and unlocking it and its parents if
    /// need be.
    pub fn set_current_layer(&mut self, id: u64) {
        if self.state.layer(id).is_none() {
            return;
        }
        let chain: Vec<u64> = self.state.layer_chain(id).iter().map(|l| l.id).collect();
        for at in chain {
            if let Some(layer) = self.layer_mut(at) {
                layer.visible = true;
                layer.locked = false;
            }
        }
        self.state.current_layer = id;
    }

    /// Whether `id` is the current layer or one of its parents, which must stay on and
    /// unlocked.
    fn holds_current(&self, id: u64) -> bool {
        self.state
            .layer_chain(self.state.current_layer)
            .iter()
            .any(|l| l.id == id)
    }

    /// Takes the objects on a layer and its sublayers out of the selection.
    fn deselect_layer(&mut self, id: u64) {
        let tree = self.state.layer_tree(id);
        let on_it: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|object| tree.contains(&object.layer))
            .map(|object| object.id)
            .collect();
        self.selection.retain(|object| !on_it.contains(object));
    }

    /// Shows or hides a layer. The current layer and its parents cannot be hidden, and a layer
    /// under a hidden parent cannot be shown on its own (spec `sublayers.md` §1).
    pub fn set_layer_visible(&mut self, id: u64, visible: bool) {
        if !visible && self.holds_current(id) {
            return;
        }
        let parent_off = self
            .state
            .layer(id)
            .and_then(|l| l.parent)
            .is_some_and(|p| !self.state.layer_shown(p));
        if visible && parent_off {
            return;
        }
        if let Some(layer) = self.layer_mut(id) {
            layer.visible = visible;
        }
        if !visible {
            self.deselect_layer(id);
        }
    }

    /// Locks or unlocks a layer. The current layer and its parents cannot be locked, and a layer
    /// under a locked parent cannot be unlocked on its own.
    pub fn set_layer_locked(&mut self, id: u64, locked: bool) {
        if locked && self.holds_current(id) {
            return;
        }
        let parent_locked = self
            .state
            .layer(id)
            .and_then(|l| l.parent)
            .is_some_and(|p| self.state.layer_locked(p));
        if !locked && parent_locked {
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
        let moved: std::collections::HashSet<u64> = objects.iter().copied().collect();
        for object in &mut self.state.objects {
            if moved.contains(&object.id) {
                std::sync::Arc::make_mut(object).layer = layer;
            }
        }
    }

    /// Deletes a layer and its sublayers, as Rhino does. Returns `false`, changing nothing, when
    /// any of them is current or has objects on it.
    pub fn delete_layer(&mut self, id: u64) -> bool {
        if self.state.layer(id).is_none() {
            return false;
        }
        let tree = self.state.layer_tree(id);
        let in_use = self
            .state
            .objects
            .iter()
            .any(|object| tree.contains(&object.layer));
        if tree.contains(&self.state.current_layer) || in_use {
            return false;
        }
        self.state.layers.retain(|layer| !tree.contains(&layer.id));
        true
    }

    /// The objects on a layer, and with `sublayers` on the layers under it too, that can be
    /// selected (Rhino's Select Objects and Select Sublayer Objects).
    #[must_use]
    pub fn objects_on_layer(&self, id: u64, sublayers: bool) -> Vec<u64> {
        let layers = if sublayers {
            self.state.layer_tree(id)
        } else {
            vec![id]
        };
        let open = self.selectable();
        self.state
            .objects
            .iter()
            .filter(|o| layers.contains(&o.layer) && open.contains(&o.id))
            .map(|o| o.id)
            .collect()
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
    fn layers_survive_being_saved_as_text() {
        let mut doc = Document::default();
        doc.add_layer("Walls".to_owned());
        let text = serde_json::to_string(doc.state()).unwrap();
        let back: crate::State = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, doc.state());
    }

    /// Walls, with Exterior under it and Glass under that.
    fn tree() -> (Document, u64, u64, u64) {
        let mut doc = Document::default();
        let walls = doc.add_layer("Walls".to_owned());
        let exterior = doc.add_sublayer(walls, "Exterior".to_owned());
        let glass = doc.add_sublayer(exterior, "Glass".to_owned());
        (doc, walls, exterior, glass)
    }

    #[test]
    fn a_sublayer_has_a_full_name_and_can_be_found_by_it() {
        let (doc, walls, exterior, glass) = tree();
        assert_eq!(doc.state().layer_path(glass), "Walls::Exterior::Glass");
        assert_eq!(doc.state().layer_named("walls::exterior"), Some(exterior));
        assert_eq!(doc.state().layer_named("Glass"), Some(glass));
        assert_eq!(doc.state().layer_tree(walls), vec![walls, exterior, glass]);
    }

    #[test]
    fn a_parent_turned_off_or_locked_rules_its_sublayers() {
        let (mut doc, walls, exterior, glass) = tree();
        let pane = doc.add(line());
        doc.move_to_layer(&[pane], glass);
        doc.set_layer_visible(walls, false);
        assert!(!doc.state().layer_shown(glass));
        // Glass keeps its own setting, but cannot be turned on while Walls is off.
        assert!(doc.layers().iter().find(|l| l.id == glass).unwrap().visible);
        doc.set_layer_visible(exterior, false);
        doc.set_layer_visible(walls, true);
        doc.set_layer_visible(glass, true);
        assert!(!doc.state().layer_shown(glass));
        assert!(!doc.selectable().contains(&pane));
        doc.set_layer_visible(exterior, true);
        assert!(doc.selectable().contains(&pane));
        doc.set_layer_locked(walls, true);
        assert!(doc.state().layer_locked(glass));
        assert!(!doc.selectable().contains(&pane));
    }

    #[test]
    fn the_current_layers_parents_stay_on_and_unlocked() {
        let (mut doc, walls, exterior, glass) = tree();
        doc.set_layer_visible(walls, false);
        doc.set_current_layer(glass);
        // Making Glass current turned its parents back on.
        assert!(doc.state().layer_shown(glass));
        doc.set_layer_visible(exterior, false);
        doc.set_layer_locked(walls, true);
        assert!(doc.state().layer_shown(glass));
        assert!(!doc.state().layer_locked(glass));
    }

    #[test]
    fn a_layer_cannot_go_under_itself_and_names_differ_on_a_level() {
        let (mut doc, walls, exterior, glass) = tree();
        assert!(!doc.set_layer_parent(walls, Some(glass)));
        assert!(!doc.set_layer_parent(walls, Some(walls)));
        assert!(doc.set_layer_parent(glass, None));
        assert_eq!(doc.state().layer_path(glass), "Glass");
        let other = doc.add_layer("Exterior".to_owned());
        assert!(
            !doc.set_layer_parent(other, Some(walls)),
            "Walls has an Exterior"
        );
        assert!(doc.set_layer_parent(glass, Some(exterior)));
    }

    #[test]
    fn deleting_a_layer_deletes_its_sublayers_unless_one_is_used() {
        let (mut doc, walls, _, glass) = tree();
        let pane = doc.add(line());
        doc.move_to_layer(&[pane], glass);
        assert!(!doc.delete_layer(walls), "Glass has an object");
        doc.remove(&[pane]);
        assert!(doc.delete_layer(walls));
        assert_eq!(doc.layers().len(), 1);
    }

    #[test]
    fn a_path_finds_or_makes_its_layers() {
        let mut doc = Document::default();
        let curves = doc.layer_at_path("Make2D::Visible::Curves");
        assert_eq!(doc.state().layer_path(curves), "Make2D::Visible::Curves");
        let again = doc.layer_at_path("Make2D::Visible::Curves");
        assert_eq!(curves, again);
        let hidden = doc.layer_at_path("Make2D::Hidden::Curves");
        assert_ne!(hidden, curves);
        assert_eq!(doc.layers().len(), 1 + 5);
    }
}
