//! The model: objects, selection, undo and redo, and saving.

use curva_kernel::{Body, Curve, Point, Similarity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

mod grips;
mod history;
pub use grips::Grip;
mod layers;
mod shape;
pub use shape::Shape;
mod views;
pub use views::SavedView;

pub use history::Recipe;

/// Marks files of this early format, so a later Curva can recognise and convert them.
const FORMAT: &str = "curva-first-slice";

/// One object in the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredObject", into = "StoredObject")]
pub struct Object {
    /// Identifies the object for as long as the document lasts.
    pub id: u64,
    /// Its geometry.
    pub shape: Shape,
    /// The layer it is on. Files from before layers put everything on the first.
    #[serde(default)]
    pub layer: u64,
    /// Whether it is out of sight until `Show`.
    #[serde(default)]
    pub hidden: bool,
    /// Its name, empty if it has none.
    #[serde(default)]
    pub name: String,
    /// How it was made from another object, if it follows one.
    #[serde(default)]
    pub recipe: Option<Recipe>,
}

impl Object {
    /// The curve, if this object is one.
    #[must_use]
    pub fn curve(&self) -> Option<&Curve> {
        self.shape.curve()
    }
}

/// An object as a file holds it: a curve or a body under its own name, so that files from
/// before bodies, which have only `curve`, still open.
#[derive(Serialize, Deserialize)]
struct StoredObject {
    id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    curve: Option<Curve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<Body>,
    #[serde(default)]
    layer: u64,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    name: String,
    #[serde(default)]
    recipe: Option<Recipe>,
}

impl TryFrom<StoredObject> for Object {
    type Error = String;

    fn try_from(stored: StoredObject) -> Result<Self, String> {
        let shape = match (stored.curve, stored.body) {
            (Some(curve), None) => Shape::Curve(curve),
            (None, Some(body)) => Shape::Body(body),
            _ => return Err(format!("object {} needs a curve or a body", stored.id)),
        };
        Ok(Self {
            id: stored.id,
            shape,
            layer: stored.layer,
            hidden: stored.hidden,
            name: stored.name,
            recipe: stored.recipe,
        })
    }
}

impl From<Object> for StoredObject {
    fn from(object: Object) -> Self {
        let (curve, body) = match object.shape {
            Shape::Curve(curve) => (Some(curve), None),
            Shape::Body(body) => (None, Some(body)),
        };
        Self {
            id: object.id,
            curve,
            body,
            layer: object.layer,
            hidden: object.hidden,
            name: object.name,
            recipe: object.recipe,
        }
    }
}

/// The colour of a new layer, red, green and blue from 0 to 1.
pub const STANDARD_COLOUR: [f32; 3] = [0.92, 0.92, 0.95];

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
}

/// The layer every model starts with. Files from before layers put everything on it.
fn standard_layers() -> Vec<Layer> {
    vec![Layer {
        id: 0,
        name: "Default".to_owned(),
        colour: STANDARD_COLOUR,
        visible: true,
        locked: false,
    }]
}

fn first_new_layer() -> u64 {
    1
}

/// Everything undo restores and a file stores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The objects, oldest first.
    pub objects: Vec<Object>,
    /// The id the next object receives. Kept so ids are not reused after an undo.
    pub next_id: u64,
    /// The layers, oldest first.
    #[serde(default = "standard_layers")]
    pub layers: Vec<Layer>,
    /// The layer new objects go on.
    #[serde(default)]
    pub current_layer: u64,
    /// The id the next layer receives.
    #[serde(default = "first_new_layer")]
    pub next_layer: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_id: 0,
            layers: standard_layers(),
            current_layer: 0,
            next_layer: first_new_layer(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct FileContents {
    format: String,
    state: State,
    #[serde(default)]
    views: Vec<SavedView>,
}

/// Why a file could not be saved or opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError(pub String);

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FileError {}

/// An open model.
#[derive(Debug, Default)]
pub struct Document {
    state: State,
    selection: BTreeSet<u64>,
    undo: Vec<State>,
    redo: Vec<State>,
    path: Option<PathBuf>,
    /// The X and Y directions of the plane being drawn on. Not saved, and not part of undo.
    work_plane: Option<[Point; 2]>,
    /// The origin of that plane. Not saved, and not part of undo.
    work_origin: Point,
    /// Whether new results remember how they were made. Not saved, and not part of undo.
    recording_history: bool,
    /// Values commands keep between runs, such as the last offset distance. Not saved.
    remembered: std::collections::BTreeMap<&'static str, f64>,
    /// The objects the last recorded change made or altered, for `SelLast`. Not saved.
    last_changed: Vec<u64>,
    /// The selection before it was last replaced, for `SelPrev`. Not saved.
    previous_selection: Vec<u64>,
    /// Views saved under a name. Saved in the file, but not part of undo.
    views: Vec<SavedView>,
    /// Objects showing their control points. Not saved, and not part of undo.
    showing_points: BTreeSet<u64>,
    /// The picked control points. Not saved, and not part of undo.
    grip_selection: BTreeSet<Grip>,
}

impl Document {
    /// The X and Y directions of the plane new flat objects go on, the ground unless set.
    #[must_use]
    pub fn work_plane(&self) -> [Point; 2] {
        self.work_plane
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
    }

    /// A value a command kept from an earlier run.
    #[must_use]
    pub fn remembered(&self, key: &str) -> Option<f64> {
        self.remembered.get(key).copied()
    }

    /// Keeps a value for later runs of a command.
    pub fn remember(&mut self, key: &'static str, value: f64) {
        self.remembered.insert(key, value);
    }

    /// The origin and the X and Y directions that typed coordinates are measured on.
    #[must_use]
    pub fn work_frame(&self) -> (Point, [Point; 2]) {
        (self.work_origin, self.work_plane())
    }

    /// Sets the origin typed coordinates are measured from.
    pub fn set_work_origin(&mut self, origin: Point) {
        self.work_origin = origin;
    }

    /// Sets the plane new flat objects go on.
    pub fn set_work_plane(&mut self, axes: [Point; 2]) {
        self.work_plane = Some(axes);
    }

    /// The objects and counters, for reading.
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Where the document was last saved or opened from.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Adds a curve and returns its id.
    pub fn add(&mut self, curve: Curve) -> u64 {
        self.add_shape(Shape::Curve(curve))
    }

    /// Adds a surface or solid and returns its id.
    pub fn add_body(&mut self, body: Body) -> u64 {
        self.add_shape(Shape::Body(body))
    }

    /// Adds an object on the current layer and returns its id.
    pub fn add_shape(&mut self, shape: Shape) -> u64 {
        let id = self.state.next_id;
        self.state.next_id += 1;
        self.state.objects.push(Object {
            id,
            shape,
            layer: self.state.current_layer,
            hidden: false,
            name: String::new(),
            recipe: None,
        });
        id
    }

    /// Adds a curve on a given layer, or the current one if `None`, and returns its id.
    pub fn add_on(&mut self, curve: Curve, layer: Option<u64>) -> u64 {
        self.add_shape_on(Shape::Curve(curve), layer)
    }

    /// Adds an object on a given layer, or the current one if `None`, and returns its id.
    pub fn add_shape_on(&mut self, shape: Shape, layer: Option<u64>) -> u64 {
        let id = self.add_shape(shape);
        if let (Some(layer), Some(object)) = (layer, self.state.objects.last_mut()) {
            object.layer = layer;
        }
        id
    }

    /// Replaces an object's curve, keeping its id and layer.
    pub fn replace(&mut self, id: u64, curve: Curve) {
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == id) {
            object.shape = Shape::Curve(curve);
            object.recipe = None;
        }
    }

    /// The objects that can be selected: not hidden, and not on a hidden or locked layer.
    #[must_use]
    pub fn selectable(&self) -> Vec<u64> {
        let open = |layer: u64| {
            self.state
                .layers
                .iter()
                .find(|l| l.id == layer)
                .is_none_or(|l| l.visible && !l.locked)
        };
        self.state
            .objects
            .iter()
            .filter(|object| !object.hidden && open(object.layer))
            .map(|object| object.id)
            .collect()
    }

    /// Takes objects out of sight and out of the selection.
    pub fn hide(&mut self, ids: &[u64]) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                object.hidden = true;
            }
        }
        self.selection.retain(|id| !ids.contains(id));
    }

    /// Brings back every hidden object and returns how many there were.
    pub fn show_all(&mut self) -> usize {
        let mut shown = 0;
        for object in &mut self.state.objects {
            if object.hidden {
                object.hidden = false;
                shown += 1;
            }
        }
        shown
    }

    /// Where the undo history stands, for [`Self::squash_undo`].
    #[must_use]
    pub fn undo_mark(&self) -> usize {
        self.undo.len()
    }

    /// Makes everything undoable since `mark` a single undo step back to `before`, so that a
    /// group of commands can be undone as one. Does nothing if the model did not change.
    pub fn squash_undo(&mut self, mark: usize, before: State) {
        if self.state == before {
            self.undo.truncate(mark);
            return;
        }
        self.undo.truncate(mark);
        self.undo.push(before);
    }

    /// Names objects.
    pub fn rename(&mut self, ids: &[u64], name: &str) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                name.clone_into(&mut object.name);
            }
        }
    }

    /// Removes objects by id.
    pub fn remove(&mut self, ids: &[u64]) {
        self.state
            .objects
            .retain(|object| !ids.contains(&object.id));
        self.selection.retain(|id| !ids.contains(id));
    }

    /// Moves objects by a vector.
    pub fn translate(&mut self, ids: &[u64], by: Point) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                object.shape = object.shape.translated(by);
                object.recipe = None;
            }
        }
    }

    /// Turns, mirrors or resizes objects.
    pub fn transform(&mut self, ids: &[u64], change: &Similarity) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                object.shape = object.shape.mapped(change);
                object.recipe = None;
            }
        }
    }

    /// Adds turned, mirrored or resized copies of objects and returns the new ids.
    pub fn copy_transformed(&mut self, ids: &[u64], change: &Similarity) -> Vec<u64> {
        let copies: Vec<(Shape, u64)> = self
            .state
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .map(|object| (object.shape.mapped(change), object.layer))
            .collect();
        copies
            .into_iter()
            .map(|(shape, layer)| self.add_shape_on(shape, Some(layer)))
            .collect()
    }

    /// Adds moved copies of objects and returns the new ids.
    pub fn copy(&mut self, ids: &[u64], by: Point) -> Vec<u64> {
        let copies: Vec<(Shape, u64)> = self
            .state
            .objects
            .iter()
            .filter(|object| ids.contains(&object.id))
            .map(|object| (object.shape.translated(by), object.layer))
            .collect();
        copies
            .into_iter()
            .map(|(shape, layer)| self.add_shape_on(shape, Some(layer)))
            .collect()
    }

    /// The selected ids, in order.
    #[must_use]
    pub fn selection(&self) -> Vec<u64> {
        self.selection.iter().copied().collect()
    }

    /// Replaces the selection, ignoring ids that do not exist.
    pub fn select(&mut self, ids: &[u64]) {
        let old = std::mem::take(&mut self.selection);
        self.selection = ids
            .iter()
            .copied()
            .filter(|id| self.state.objects.iter().any(|object| object.id == *id))
            .collect();
        if !old.is_empty() && old != self.selection {
            self.previous_selection = old.into_iter().collect();
        }
    }

    /// The objects the last recorded change made or altered that still exist.
    #[must_use]
    pub fn last_changed(&self) -> Vec<u64> {
        self.existing(&self.last_changed)
    }

    /// The selection before it was last replaced, leaving out objects that are gone.
    #[must_use]
    pub fn previous_selection(&self) -> Vec<u64> {
        self.existing(&self.previous_selection)
    }

    fn existing(&self, ids: &[u64]) -> Vec<u64> {
        ids.iter()
            .copied()
            .filter(|id| self.state.objects.iter().any(|object| object.id == *id))
            .collect()
    }

    /// Records `before` as one undo step, and clears anything that could be redone.
    pub fn record_change(&mut self, before: State) {
        let earlier: std::collections::BTreeMap<u64, &Shape> =
            before.objects.iter().map(|o| (o.id, &o.shape)).collect();
        self.last_changed = self
            .state
            .objects
            .iter()
            .filter(|o| earlier.get(&o.id) != Some(&&o.shape))
            .map(|o| o.id)
            .collect();
        self.undo.push(before);
        self.redo.clear();
    }

    /// Puts the state back to `state` without touching undo, as when a command is cancelled.
    pub fn restore(&mut self, state: State) {
        self.state = state;
        self.drop_missing_selection();
    }

    fn drop_missing_selection(&mut self) {
        let existing: BTreeSet<u64> = self.state.objects.iter().map(|o| o.id).collect();
        self.selection.retain(|id| existing.contains(id));
    }

    /// Steps back one change. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.state, previous));
        self.drop_missing_selection();
        true
    }

    /// Repeats one undone change. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.state, next));
        self.drop_missing_selection();
        true
    }

    /// Writes the document to a file and remembers the location.
    pub fn save(&mut self, path: &Path) -> Result<(), FileError> {
        let contents = FileContents {
            format: FORMAT.to_owned(),
            state: self.state.clone(),
            views: self.views.clone(),
        };
        let text = serde_json::to_string_pretty(&contents).map_err(|e| FileError(e.to_string()))?;
        std::fs::write(path, text).map_err(|e| FileError(e.to_string()))?;
        self.path = Some(path.to_owned());
        Ok(())
    }

    /// Reads a document from a file. The undo history starts empty.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let text = std::fs::read_to_string(path).map_err(|e| FileError(e.to_string()))?;
        let contents: FileContents =
            serde_json::from_str(&text).map_err(|e| FileError(e.to_string()))?;
        if contents.format != FORMAT {
            return Err(FileError(format!("unknown format {}", contents.format)));
        }
        Ok(Self {
            state: contents.state,
            views: contents.views,
            path: Some(path.to_owned()),
            ..Self::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: f64) -> Curve {
        Curve::line([0.0; 3], [x, 0.0, 0.0]).unwrap()
    }

    /// Adds an object as one undoable change, the way the command layer does.
    fn add_undoably(doc: &mut Document, curve: Curve) -> u64 {
        let before = doc.state().clone();
        let id = doc.add(curve);
        doc.record_change(before);
        id
    }

    #[test]
    fn undo_restores_exactly_and_redo_repeats() {
        let mut doc = Document::default();
        add_undoably(&mut doc, line(1.0));
        let one = doc.state().clone();
        add_undoably(&mut doc, line(2.0));
        let two = doc.state().clone();
        assert!(doc.undo());
        assert_eq!(doc.state(), &one);
        assert!(doc.redo());
        assert_eq!(doc.state(), &two);
    }

    #[test]
    fn nothing_to_undo_or_redo_says_so() {
        let mut doc = Document::default();
        assert!(!doc.undo());
        assert!(!doc.redo());
    }

    #[test]
    fn a_new_change_clears_redo() {
        let mut doc = Document::default();
        add_undoably(&mut doc, line(1.0));
        doc.undo();
        add_undoably(&mut doc, line(2.0));
        assert!(!doc.redo());
    }

    #[test]
    fn ids_are_not_reused_after_a_removal() {
        let mut doc = Document::default();
        let first = doc.add(line(1.0));
        doc.remove(&[first]);
        assert_ne!(doc.add(line(2.0)), first);
    }

    #[test]
    fn selection_loses_objects_that_undo_removes() {
        let mut doc = Document::default();
        let id = add_undoably(&mut doc, line(1.0));
        doc.select(&[id]);
        doc.undo();
        assert!(doc.selection().is_empty());
    }

    #[test]
    fn saved_files_reopen_with_the_same_objects() {
        let folder = std::env::temp_dir().join(format!("curva-doc-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("model.curva");
        let mut doc = Document::default();
        doc.add(line(7.0));
        doc.add(Curve::circle([1.0, 2.0, 3.0], 4.0).unwrap());
        doc.save(&path).unwrap();
        let opened = Document::open(&path).unwrap();
        assert_eq!(opened.state(), doc.state());
        assert_eq!(opened.path(), Some(path.as_path()));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn solids_save_and_reopen_exactly() {
        let folder = std::env::temp_dir().join(format!("curva-body-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("solids.curva");
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]).unwrap());
        doc.transform(&[id], &Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], 0.3));
        doc.add(line(2.0));
        doc.save(&path).unwrap();
        let opened = Document::open(&path).unwrap();
        assert_eq!(opened.state(), doc.state());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn files_from_before_solids_still_open() {
        let folder = std::env::temp_dir().join(format!("curva-old-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("old.curva");
        std::fs::write(
            &path,
            r#"{"format":"curva-first-slice","state":{"objects":[{"id":0,"curve":{"Line":{"start":[0,0,0],"end":[3,0,0]}}}],"next_id":1}}"#,
        )
        .unwrap();
        let opened = Document::open(&path).unwrap();
        assert_eq!(opened.state().objects[0].curve(), Some(&line(3.0)));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_file_in_another_format_is_refused() {
        let folder = std::env::temp_dir().join(format!("curva-bad-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("other.curva");
        std::fs::write(
            &path,
            r#"{"format":"other","state":{"objects":[],"next_id":0}}"#,
        )
        .unwrap();
        assert!(Document::open(&path).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
