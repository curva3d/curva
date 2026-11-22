//! The model: objects, selection, undo and redo, and saving.

use curva_kernel::{Body, Curve, Point, Similarity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

mod edges;
pub use edges::{EdgePick, FacePick};
mod mesh_picks;
pub use mesh_picks::{MeshPart, MeshPick};
mod grips;
mod history;
pub use grips::Grip;
pub mod blocks;
pub use blocks::{BlockDefinition, BlockEdit, Instance, PLACED_AS_IS, Placement};
mod layers;
mod live;
mod migrate;
pub use live::{LiveBoolean, LiveHow};
pub mod annotation;
pub mod clipping;
pub mod drawing;
pub mod fields;
pub mod formats;
pub mod hatch;
pub mod layouts;
pub mod meshing;
mod organise;
pub mod progress;
mod repairs;
pub mod section;
pub mod shape;
pub mod sheet;
pub mod tie;
pub mod units;
pub use shape::Shape;
mod views;
pub use views::{SavedCPlane, SavedView, views_for_viewports};

pub use history::Recipe;

mod linetype;
mod object;
mod state;
pub use layers::*;
pub use linetype::*;
pub use object::*;
pub use repairs::Repair;
pub use state::*;

/// Marks files of this early format, so a later Curva can recognise and convert them.
const FORMAT: &str = "curva-first-slice";
/// The entry inside a `.curva` package that holds the model.
const MODEL_ENTRY: &str = "model.json";

#[derive(Serialize, Deserialize)]
struct FileContents {
    format: String,
    /// The version of the format, which `migrate` brings older files up to.
    version: u64,
    state: State,
    views: Vec<SavedView>,
    cplanes: Vec<SavedCPlane>,
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

/// How much undo keeps, as Rhino's two Undo options: at least `min_steps`, and older steps are
/// dropped once the steps hold more than `max_bytes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoLimits {
    /// Steps kept whatever they hold.
    pub min_steps: usize,
    /// About how many bytes the steps may hold together beyond that.
    pub max_bytes: usize,
}

impl Default for UndoLimits {
    fn default() -> Self {
        Self {
            min_steps: 10,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// One undo or redo step: the state to go back to, the selection with it, and about how many
/// bytes it holds that the state next to it does not share.
#[derive(Debug)]
struct Step {
    state: State,
    selection: BTreeSet<u64>,
    bytes: usize,
}

/// About how many bytes a mesh holds: its points and its faces' corners.
fn mesh_bytes(mesh: &curva_mesh::Mesh) -> usize {
    std::mem::size_of_val(mesh.vertices()) + mesh.faces().map(|f| f.len() * 4 + 16).sum::<usize>()
}

/// About how many bytes `state` holds that `beside` does not share with it: the objects it
/// does not have in common (solids counted only if their faces are not shared too), and the
/// list of objects itself.
fn weigh(state: &State, beside: &State) -> usize {
    let objects: std::collections::HashSet<*const Object> =
        beside.objects.iter().map(std::sync::Arc::as_ptr).collect();
    let faces: std::collections::HashSet<usize> = beside
        .objects
        .iter()
        .filter_map(|o| o.shape.body().map(Body::faces_key))
        .collect();
    let object_bytes = |o: &Object| {
        std::mem::size_of::<Object>()
            + o.name.len()
            + o.user_text
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
            + match &o.shape {
                Shape::Curve(curve) => curve.heap_bytes(),
                Shape::Body(body) if !faces.contains(&body.faces_key()) => body.held_bytes(),
                Shape::Mesh(mesh) => mesh_bytes(mesh),
                Shape::SubD(subd) => mesh_bytes(subd.cage()),
                Shape::Body(_) | Shape::Instance(_) | Shape::Annotation(_) => 0,
            }
    };
    let own: usize = state
        .objects
        .iter()
        .filter(|o| !objects.contains(&std::sync::Arc::as_ptr(o)))
        .map(|o| object_bytes(o))
        .sum();
    let blocks: usize = state
        .blocks
        .iter()
        .flat_map(|b| &b.objects)
        .map(object_bytes)
        .sum();
    own + blocks + state.objects.len() * std::mem::size_of::<std::sync::Arc<Object>>()
}

/// An open model.
#[derive(Debug, Default)]
pub struct Document {
    state: State,
    selection: BTreeSet<u64>,
    /// Earlier states, each with the selection that came with it, newest last.
    undo: Vec<Step>,
    /// Undone states, each with its selection, newest last.
    redo: Vec<Step>,
    /// How many of the oldest undo steps were dropped to keep within `undo_limits`, so that
    /// marks from [`Self::undo_mark`] stay valid.
    undo_dropped: usize,
    /// How much undo may keep. Not saved: the app sets it from the user's settings.
    undo_limits: UndoLimits,
    path: Option<PathBuf>,
    /// The X and Y directions of the plane being drawn on. Not saved, and not part of undo.
    work_plane: Option<[Point; 2]>,
    /// The origin of that plane. Not saved, and not part of undo.
    work_origin: Point,
    /// Whether new results remember how they were made. Not saved, and not part of undo.
    recording_history: bool,
    /// Values commands keep between runs, such as the last offset distance. Not saved.
    remembered: std::collections::BTreeMap<String, f64>,
    /// Names commands keep for their next runs, like `remembered`.
    remembered_text: std::collections::BTreeMap<String, String>,
    /// The objects the last recorded change made or altered, for `SelLast`. Not saved.
    last_changed: Vec<u64>,
    /// The selection before it was last replaced, for `SelPrev`. Not saved.
    previous_selection: Vec<u64>,
    /// Views saved under a name. Saved in the file, but not part of undo.
    views: Vec<SavedView>,
    /// Construction planes saved under a name. Saved in the file, but not part of undo.
    cplanes: Vec<SavedCPlane>,
    /// Objects showing their control points. Not saved, and not part of undo.
    showing_points: BTreeSet<u64>,
    /// The picked control points. Not saved, and not part of undo.
    grip_selection: BTreeSet<Grip>,
    /// Edges of bodies picked for a command. Not saved, and not part of undo.
    edge_selection: Vec<EdgePick>,
    /// Faces of bodies picked for a command. Not saved, and not part of undo.
    face_selection: Vec<FacePick>,
    /// Parts of meshes and SubDs picked for a command (#333). Not saved, and not part of undo.
    mesh_part_selection: Vec<MeshPick>,
    /// The tie that came with the point being clicked, for a command to keep. Not saved, and not
    /// part of undo.
    pick_tie: Option<tie::Tie>,
    /// The layout being worked on, or `None` for the model. Not saved, and not part of undo.
    space: Option<u64>,
    /// Where the last-used viewport looks from, for Make2D. Not saved, and not part of undo.
    view: Option<curva_kernel::hidden::View>,
    /// The file formats the file commands offer (spec `file-formats.md`). Not saved, and not part
    /// of undo; a model that replaces this one keeps them.
    formats: std::sync::Arc<formats::Formats>,
    /// Solids read quickly, waiting to be healed (#476). Not saved, and not part of undo.
    repairs: repairs::Repairs,
    /// Whether this model has just replaced another, as Open does, and the views have not yet been
    /// fitted to it (#479). Not saved, and not part of undo.
    just_opened: bool,
    /// The views a file just opened was saved with, for the viewports to take up (#479). Not
    /// saved, and not part of undo.
    opened_views: Vec<views::SavedView>,
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
    pub fn remember(&mut self, key: &str, value: f64) {
        self.remembered.insert(key.to_owned(), value);
    }

    /// A name kept for later runs of a command, such as a chosen hatch pattern.
    #[must_use]
    pub fn remembered_text(&self, key: &str) -> Option<&str> {
        self.remembered_text.get(key).map(String::as_str)
    }

    /// Keeps a name for later runs of a command.
    pub fn remember_text(&mut self, key: &str, value: &str) {
        self.remembered_text
            .insert(key.to_owned(), value.to_owned());
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

    /// Where the last-used viewport looks from; `None` until the interface says.
    #[must_use]
    pub fn view(&self) -> Option<curva_kernel::hidden::View> {
        self.view
    }

    /// Records where the last-used viewport looks from.
    pub fn set_view(&mut self, view: curva_kernel::hidden::View) {
        self.view = Some(view);
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
        self.state.objects.push(std::sync::Arc::new(Object {
            id,
            shape,
            layer: self.state.current_layer,
            hidden: false,
            name: String::new(),
            recipe: None,
            print_width: PrintWidth::ByLayer,
            locked: false,
            group: None,
            live: None,
            input_of: None,
            colour: None,
            linetype: Linetype::ByLayer,
            user_text: std::collections::BTreeMap::new(),
            space: self.space,
            section: None,
            ties: Vec::new(),
        }));
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
            std::sync::Arc::make_mut(object).layer = layer;
        }
        id
    }

    /// Replaces an object's curve, keeping its id and layer.
    pub fn replace(&mut self, id: u64, curve: Curve) {
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == id) {
            let object = std::sync::Arc::make_mut(object);
            object.shape = Shape::Curve(curve);
            object.recipe = None;
        }
    }

    /// The objects that can be selected: not hidden, and not on a hidden or locked layer.
    #[must_use]
    pub fn selectable(&self) -> Vec<u64> {
        // Through the layer's parents (spec `sublayers.md` §1).
        let open = |layer: u64| self.state.layer_shown(layer) && !self.state.layer_locked(layer);
        self.state
            .objects
            .iter()
            .filter(|object| {
                !object.hidden && !object.locked && object.space == self.space && open(object.layer)
            })
            .map(|object| object.id)
            .collect()
    }

    /// Takes objects out of sight and out of the selection.
    pub fn hide(&mut self, ids: &[u64]) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                std::sync::Arc::make_mut(object).hidden = true;
            }
        }
        self.selection.retain(|id| !ids.contains(id));
    }

    /// Brings back every hidden object and returns how many there were.
    pub fn show_all(&mut self) -> usize {
        let mut shown = 0;
        for object in &mut self.state.objects {
            if object.hidden && object.input_of.is_none() {
                std::sync::Arc::make_mut(object).hidden = false;
                shown += 1;
            }
        }
        shown
    }

    /// Where the undo history stands, for [`Self::squash_undo`].
    #[must_use]
    pub fn undo_mark(&self) -> usize {
        self.undo_dropped + self.undo.len()
    }

    /// How much undo may keep.
    #[must_use]
    pub fn undo_limits(&self) -> UndoLimits {
        self.undo_limits
    }

    /// Sets how much undo may keep, dropping the oldest steps if it now holds too much.
    pub fn set_undo_limits(&mut self, limits: UndoLimits) {
        self.undo_limits = limits;
        self.trim_undo();
    }

    /// How many steps undo holds, and about how many bytes they hold together.
    #[must_use]
    pub fn undo_held(&self) -> (usize, usize) {
        (self.undo.len(), self.undo.iter().map(|s| s.bytes).sum())
    }

    /// Drops the oldest undo steps while there are more than the least kept and they hold more
    /// than allowed.
    fn trim_undo(&mut self) {
        let mut held: usize = self.undo.iter().map(|s| s.bytes).sum();
        let mut drop = 0;
        while self.undo.len() - drop > self.undo_limits.min_steps
            && held > self.undo_limits.max_bytes
        {
            held -= self.undo[drop].bytes;
            drop += 1;
        }
        self.undo.drain(..drop);
        self.undo_dropped += drop;
    }

    /// `before` as an undo step, weighed against the state now.
    fn step(&self, before: State, selection: BTreeSet<u64>) -> Step {
        Step {
            bytes: weigh(&before, &self.state),
            state: before,
            selection,
        }
    }

    /// Makes everything undoable since `mark` a single undo step back to `before`, so that a
    /// group of commands can be undone as one. Does nothing if the model did not change.
    pub fn squash_undo(&mut self, mark: usize, before: State) {
        let mark = mark.saturating_sub(self.undo_dropped);
        if self.state == before {
            self.undo.truncate(mark);
            return;
        }
        let selection = self
            .undo
            .get(mark)
            .map_or_else(|| self.selection.clone(), |step| step.selection.clone());
        self.undo.truncate(mark);
        let step = self.step(before, selection);
        self.undo.push(step);
        self.trim_undo();
    }

    /// Changes the model's unit. With `scale`, everything is resized about the origin to keep
    /// its real size, as Rhino offers; otherwise the numbers stay as they are.
    pub fn set_unit(&mut self, unit: Unit, scale: bool) {
        let factor = self.state.unit.millimetres() / unit.millimetres();
        if scale && (factor - 1.0).abs() > f64::EPSILON {
            let ids: Vec<u64> = self.state.objects.iter().map(|o| o.id).collect();
            self.transform(&ids, &curva_kernel::Similarity::scaling([0.0; 3], factor));
        }
        self.state.unit = unit;
    }

    /// Sets how feet and inches are shown.
    pub fn set_length_display(&mut self, display: units::LengthDisplay) {
        self.state.length_display = display;
    }

    /// A length in the model's unit, as the model shows it.
    #[must_use]
    pub fn length_text(&self, value: f64) -> String {
        units::format_length(value, self.state.unit, self.state.length_display)
    }

    /// Names objects.
    pub fn rename(&mut self, ids: &[u64], name: &str) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                name.clone_into(&mut std::sync::Arc::make_mut(object).name);
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
                let object = std::sync::Arc::make_mut(object);
                object.shape = object.shape.translated(by);
                object.recipe = None;
            }
        }
    }

    /// Turns, mirrors or resizes objects.
    pub fn transform(&mut self, ids: &[u64], change: &Similarity) {
        for object in &mut self.state.objects {
            if ids.contains(&object.id) {
                let object = std::sync::Arc::make_mut(object);
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

    /// As [`Self::record_change`], with the selection there was before the change, which an undo
    /// brings back with the model.
    pub fn record_change_with(&mut self, before: State, selection: &[u64]) {
        self.record_change(before);
        if let Some(step) = self.undo.last_mut() {
            step.selection = selection.iter().copied().collect();
        }
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
        let step = self.step(before, self.selection.clone());
        self.undo.push(step);
        self.redo.clear();
        self.trim_undo();
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
        let Some(Step {
            state, selection, ..
        }) = self.undo.pop()
        else {
            return false;
        };
        let now = std::mem::replace(&mut self.state, state);
        let selection = std::mem::replace(&mut self.selection, selection);
        let step = self.step(now, selection);
        self.redo.push(step);
        self.drop_missing_selection();
        true
    }

    /// Repeats one undone change. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(Step {
            state, selection, ..
        }) = self.redo.pop()
        else {
            return false;
        };
        let now = std::mem::replace(&mut self.state, state);
        let selection = std::mem::replace(&mut self.selection, selection);
        let step = self.step(now, selection);
        self.undo.push(step);
        self.trim_undo();
        self.drop_missing_selection();
        true
    }

    /// Writes the document to a file and remembers the location.
    pub fn save(&mut self, path: &Path) -> Result<(), FileError> {
        self.write_copy(path)?;
        self.path = Some(path.to_owned());
        Ok(())
    }

    /// Writes the document to a file without making it the document's location, as autosave
    /// does. A half-written file never replaces a whole one: it is written beside and renamed.
    pub fn write_copy(&self, path: &Path) -> Result<(), FileError> {
        let contents = FileContents {
            format: FORMAT.to_owned(),
            version: crate::migrate::CURRENT,
            state: self.state.clone(),
            views: self.views.clone(),
            cplanes: self.cplanes.clone(),
        };
        let failed = |e: &dyn std::fmt::Display| FileError(e.to_string());
        let json = serde_json::to_vec(&contents).map_err(|e| failed(&e))?;
        let mut partial = path.as_os_str().to_owned();
        partial.push(".partial");
        let partial = std::path::PathBuf::from(partial);
        // A zip package holding the model as JSON (decision 0015). Fast compression: the file is
        // about 13 times smaller than the JSON, and saving stays as quick.
        let file = std::fs::File::create(&partial).map_err(|e| failed(&e))?;
        let mut package = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(1));
        package
            .start_file(MODEL_ENTRY, options)
            .map_err(|e| failed(&e))?;
        std::io::Write::write_all(&mut package, &json).map_err(|e| failed(&e))?;
        let mut written = package.finish().map_err(|e| failed(&e))?;
        std::io::Write::flush(&mut written).map_err(|e| failed(&e))?;
        drop(written);
        std::fs::rename(&partial, path).map_err(|e| failed(&e))
    }

    /// Sets or forgets the file the document saves to, as when a recovered copy is opened on
    /// behalf of the file it was made from.
    pub fn set_path(&mut self, path: Option<std::path::PathBuf>) {
        self.path = path;
    }

    /// Reads a document from a file. The undo history starts empty.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let failed = |e: &dyn std::fmt::Display| FileError(e.to_string());
        let bytes = std::fs::read(path).map_err(|e| failed(&e))?;
        // A zip package since decision 0015; before that, the JSON text on its own.
        let json = if bytes.starts_with(b"PK\x03\x04") {
            let mut package =
                zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| failed(&e))?;
            let mut entry = package.by_name(MODEL_ENTRY).map_err(|e| failed(&e))?;
            let mut json = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut json).map_err(|e| failed(&e))?;
            json
        } else {
            bytes
        };
        let mut file: serde_json::Value = serde_json::from_slice(&json).map_err(|e| failed(&e))?;
        let format = file.get("format").and_then(serde_json::Value::as_str);
        if format != Some(FORMAT) {
            return Err(FileError(format!(
                "unknown format {}",
                format.unwrap_or("(none)")
            )));
        }
        crate::migrate::upgrade(&mut file).map_err(FileError)?;
        let contents: FileContents = serde_json::from_value(file).map_err(|e| failed(&e))?;
        Ok(Self {
            state: contents.state,
            views: contents.views,
            cplanes: contents.cplanes,
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
    fn an_undo_step_shares_the_objects_a_change_left_alone() {
        let mut doc = Document::default();
        let kept = doc.add(line(1.0));
        let moved = doc.add(line(2.0));
        let before = doc.state().clone();
        doc.translate(&[moved], [0.0, 1.0, 0.0]);
        doc.record_change(before);
        doc.undo();
        doc.redo();
        let now = &doc.state().objects;
        let earlier = &doc.undo.last().unwrap().state.objects;
        let find = |list: &[std::sync::Arc<Object>], id: u64| {
            std::sync::Arc::clone(list.iter().find(|o| o.id == id).unwrap())
        };
        assert!(std::sync::Arc::ptr_eq(
            &find(now, kept),
            &find(earlier, kept)
        ));
        assert!(!std::sync::Arc::ptr_eq(
            &find(now, moved),
            &find(earlier, moved)
        ));
    }

    #[test]
    fn undo_drops_the_oldest_steps_beyond_its_budget_but_keeps_the_least() {
        let mut doc = Document::default();
        doc.set_undo_limits(UndoLimits {
            min_steps: 3,
            max_bytes: 0,
        });
        for x in 1..=8 {
            add_undoably(&mut doc, line(f64::from(x)));
        }
        assert_eq!(doc.undo_held().0, 3);
        assert_eq!(doc.undo_mark(), 8);
        assert!(doc.undo() && doc.undo() && doc.undo());
        assert!(!doc.undo());
        assert_eq!(doc.state().objects.len(), 5);
    }

    #[test]
    fn a_roomy_budget_keeps_every_step() {
        let mut doc = Document::default();
        for x in 1..=30 {
            add_undoably(&mut doc, line(f64::from(x)));
        }
        let (steps, bytes) = doc.undo_held();
        assert_eq!(steps, 30);
        assert!(bytes > 0 && bytes < UndoLimits::default().max_bytes);
    }

    #[test]
    fn grouped_changes_still_undo_as_one_after_old_steps_are_dropped() {
        let mut doc = Document::default();
        doc.set_undo_limits(UndoLimits {
            min_steps: 2,
            max_bytes: 0,
        });
        add_undoably(&mut doc, line(1.0));
        add_undoably(&mut doc, line(2.0));
        let mark = doc.undo_mark();
        let before = doc.state().clone();
        for x in 3..=6 {
            add_undoably(&mut doc, line(f64::from(x)));
        }
        doc.squash_undo(mark, before);
        assert_eq!(doc.state().objects.len(), 6);
        assert!(doc.undo());
        assert_eq!(doc.state().objects.len(), 2);
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
    fn print_widths_follow_the_layer_unless_set_and_are_saved() {
        let mut doc = Document::default();
        let a = doc.add(line(1.0));
        let b = doc.add(line(2.0));
        let c = doc.add(line(3.0));
        assert_eq!(doc.print_width_of(a), None);
        doc.set_layer_print_width(0, Some(0.35));
        doc.set_print_width(&[b], PrintWidth::Millimetres(1.4));
        doc.set_print_width(&[c], PrintWidth::Default);
        assert_eq!(doc.print_width_of(a), Some(0.35));
        assert_eq!(doc.print_width_of(b), Some(1.4));
        assert_eq!(doc.print_width_of(c), None);
        // Kept within what can be set, to two decimals.
        doc.set_print_width(&[a], PrintWidth::Millimetres(9.123));
        assert_eq!(doc.print_width_of(a), Some(PrintWidth::MOST));
        let folder = std::env::temp_dir().join(format!("curva-widths-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("widths.curva");
        doc.save(&path).unwrap();
        assert_eq!(Document::open(&path).unwrap().state(), doc.state());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn objects_and_layers_from_before_widths_open_with_the_default_line() {
        let object: Object =
            serde_json::from_str(r#"{"id":3,"curve":{"Line":{"start":[0,0,0],"end":[1,0,0]}}}"#)
                .unwrap();
        assert_eq!(object.print_width, PrintWidth::ByLayer);
        assert!(!object.locked);
        assert_eq!(object.group, None);
        let layer: Layer = serde_json::from_str(
            r#"{"id":0,"name":"Default","colour":[1,1,1],"visible":true,"locked":false}"#,
        )
        .unwrap();
        assert_eq!(layer.print_width, None);
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
    fn files_are_zip_packages_and_older_json_files_still_open() {
        let folder = std::env::temp_dir().join(format!("curva-zip-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut doc = Document::default();
        for i in 0..200 {
            doc.add(Curve::circle([f64::from(i) * 30.0, 0.0, 0.0], 10.0).unwrap());
        }
        let package = folder.join("new.curva");
        doc.save(&package).unwrap();
        let bytes = std::fs::read(&package).unwrap();
        assert!(bytes.starts_with(b"PK\x03\x04"), "not a zip package");
        assert_eq!(Document::open(&package).unwrap().state(), doc.state());
        // The same model as the JSON text files were before.
        let old = folder.join("old.curva");
        let text = serde_json::to_string_pretty(&FileContents {
            format: FORMAT.to_owned(),
            version: 0,
            state: doc.state().clone(),
            views: Vec::new(),
            cplanes: Vec::new(),
        })
        .unwrap();
        std::fs::write(&old, &text).unwrap();
        assert_eq!(Document::open(&old).unwrap().state(), doc.state());
        assert!(
            bytes.len() * 5 < text.len(),
            "{} vs {}",
            bytes.len(),
            text.len()
        );
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_copy_is_written_whole_without_becoming_the_location() {
        let folder = std::env::temp_dir().join(format!("curva-copy-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut doc = Document::default();
        doc.add(line(5.0));
        let copy = folder.join("copy.curva");
        doc.write_copy(&copy).unwrap();
        // Written twice, the second replaces the first; no partial file is left beside it.
        doc.write_copy(&copy).unwrap();
        assert_eq!(doc.path(), None);
        assert_eq!(Document::open(&copy).unwrap().state(), doc.state());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
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

    #[test]
    fn undo_brings_back_the_selection_that_came_with_the_change() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        let before = doc.state().clone();
        doc.remove(&[line]);
        doc.record_change_with(before, &[line]);
        assert!(doc.selection().is_empty());
        assert!(doc.undo());
        assert_eq!(doc.selection(), vec![line]);
        assert!(doc.redo());
        assert!(doc.selection().is_empty());
    }

    #[test]
    fn the_unit_is_kept_in_the_file_and_scaling_keeps_real_sizes() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [1000.0, 0.0, 0.0]).unwrap());
        doc.set_unit(Unit::Metres, true);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 1.0).abs() < 1e-12);
        doc.set_unit(Unit::Millimetres, false);
        let kept = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((kept - 1.0).abs() < 1e-12);
        doc.set_unit(Unit::Metres, false);
        let folder = std::env::temp_dir().join(format!("curva-unit-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("unit.curva");
        doc.save(&file).unwrap();
        assert_eq!(Document::open(&file).unwrap().state().unit, Unit::Metres);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
