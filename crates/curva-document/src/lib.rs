//! The model: objects, selection, undo and redo, and saving.

use curva_kernel::{Body, Curve, Point, Similarity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

mod edges;
pub use edges::{EdgePick, FacePick};
mod grips;
mod history;
pub use grips::Grip;
pub mod blocks;
pub use blocks::{BlockDefinition, BlockEdit, Instance, PLACED_AS_IS, Placement};
mod layers;
mod live;
mod migrate;
pub use live::{LiveBoolean, LiveHow};
mod organise;
mod shape;
pub use shape::Shape;
mod views;
pub use views::{SavedCPlane, SavedView};

pub use history::Recipe;

/// Marks files of this early format, so a later Curva can recognise and convert them.
const FORMAT: &str = "curva-first-slice";
/// The entry inside a `.curva` package that holds the model.
const MODEL_ENTRY: &str = "model.json";

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
    /// How thick its lines are drawn and printed (spec `line-weights.md`).
    pub print_width: PrintWidth,
    /// Whether it shows but cannot be picked or changed, until `Unlock`.
    pub locked: bool,
    /// The group it belongs to, if any: picking one member picks them all.
    pub group: Option<u64>,
    /// For a live boolean's result: how it is made and from what.
    pub live: Option<crate::live::LiveBoolean>,
    /// For an input kept by a live boolean: the result it belongs to.
    pub input_of: Option<u64>,
    /// How its lines are drawn: continuous, dashed and so on, or as its layer says.
    pub linetype: Linetype,
    /// Key–value text kept with it (`SetUserText`).
    pub user_text: std::collections::BTreeMap<String, String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance: Option<crate::blocks::Instance>,
    #[serde(default)]
    layer: u64,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    name: String,
    #[serde(default)]
    recipe: Option<Recipe>,
    #[serde(default, skip_serializing_if = "PrintWidth::is_by_layer")]
    print_width: PrintWidth,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live: Option<crate::live::LiveBoolean>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_of: Option<u64>,
    #[serde(default, skip_serializing_if = "Linetype::is_by_layer")]
    linetype: Linetype,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    user_text: std::collections::BTreeMap<String, String>,
}

impl TryFrom<StoredObject> for Object {
    type Error = String;

    fn try_from(stored: StoredObject) -> Result<Self, String> {
        let shape = match (stored.curve, stored.body, stored.instance) {
            (Some(curve), None, None) => Shape::Curve(curve),
            (None, Some(body), None) => Shape::Body(body),
            (None, None, Some(instance)) => Shape::Instance(instance),
            _ => return Err(format!("object {} needs a curve or a body", stored.id)),
        };
        Ok(Self {
            id: stored.id,
            shape,
            layer: stored.layer,
            hidden: stored.hidden,
            name: stored.name,
            recipe: stored.recipe,
            print_width: stored.print_width,
            locked: stored.locked,
            group: stored.group,
            live: stored.live,
            input_of: stored.input_of,
            linetype: stored.linetype,
            user_text: stored.user_text,
        })
    }
}

impl From<Object> for StoredObject {
    fn from(object: Object) -> Self {
        let (curve, body, instance) = match object.shape {
            Shape::Curve(curve) => (Some(curve), None, None),
            Shape::Body(body) => (None, Some(body), None),
            Shape::Instance(instance) => (None, None, Some(instance)),
        };
        Self {
            id: object.id,
            curve,
            body,
            instance,
            layer: object.layer,
            hidden: object.hidden,
            name: object.name,
            recipe: object.recipe,
            print_width: object.print_width,
            locked: object.locked,
            group: object.group,
            live: object.live,
            input_of: object.input_of,
            linetype: object.linetype,
            user_text: object.user_text,
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
    /// How thick its objects are drawn and printed, in millimetres; `None` is the default thin line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_width: Option<f64>,
    /// How its objects' lines are drawn.
    #[serde(
        default = "continuous",
        skip_serializing_if = "Linetype::is_continuous"
    )]
    pub linetype: Linetype,
}

/// How thick an object's lines are, as Rhino's Print Width (spec `line-weights.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PrintWidth {
    /// As its layer says.
    #[default]
    ByLayer,
    /// The default thin line, whatever the layer says.
    Default,
    /// This many millimetres.
    Millimetres(f64),
}

impl PrintWidth {
    /// The widths Rhino offers in its list, in millimetres.
    pub const COMMON: [f64; 9] = [0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];
    /// The thickest width that can be set, in millimetres.
    pub const MOST: f64 = 5.0;

    /// Whether it follows the layer, which files leave unwritten.
    #[must_use]
    pub fn is_by_layer(&self) -> bool {
        matches!(self, Self::ByLayer)
    }

    /// The width in millimetres an object on `layer` draws at, or `None` for the default line.
    #[must_use]
    pub fn resolved(self, layer: Option<&Layer>) -> Option<f64> {
        match self {
            Self::ByLayer => layer.and_then(|layer| layer.print_width),
            Self::Default => None,
            Self::Millimetres(mm) => Some(mm),
        }
    }

    /// A width kept to what can be set: 0 to [`Self::MOST`] millimetres, two decimals.
    #[must_use]
    pub fn clamped(mm: f64) -> f64 {
        (mm.clamp(0.0, Self::MOST) * 100.0).round() / 100.0
    }
}

/// How a line is drawn, as Rhino's linetypes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Linetype {
    /// As its layer says (objects only).
    #[default]
    ByLayer,
    /// A solid line.
    Continuous,
    /// Long dashes.
    Dashed,
    /// Dots.
    Dotted,
    /// Dashes and dots.
    DashDot,
    /// Short dashes, for hidden lines.
    Hidden,
    /// Long and short dashes, for centre lines.
    Center,
    /// Two dashes and a dot.
    Border,
}

impl Linetype {
    /// The linetypes offered, after `ByLayer`.
    pub const ALL: [Self; 7] = [
        Self::Continuous,
        Self::Dashed,
        Self::Dotted,
        Self::DashDot,
        Self::Hidden,
        Self::Center,
        Self::Border,
    ];

    /// Whether it follows the layer, which files leave unwritten.
    #[must_use]
    pub fn is_by_layer(&self) -> bool {
        matches!(self, Self::ByLayer)
    }

    /// Whether it is a solid line, which files leave unwritten for layers.
    #[must_use]
    pub fn is_continuous(&self) -> bool {
        matches!(self, Self::Continuous)
    }

    /// Its name, as Rhino names it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ByLayer => "ByLayer",
            Self::Continuous => "Continuous",
            Self::Dashed => "Dashed",
            Self::Dotted => "Dots",
            Self::DashDot => "DashDot",
            Self::Hidden => "Hidden",
            Self::Center => "Center",
            Self::Border => "Border",
        }
    }

    /// The linetype with this name, ignoring case.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        std::iter::once(Self::ByLayer).chain(Self::ALL).find(|t| {
            t.name().eq_ignore_ascii_case(name)
                || (name.eq_ignore_ascii_case("dotted") && *t == Self::Dotted)
        })
    }

    /// Its pattern in model units: lengths drawn and left out in turn; empty for a solid line.
    #[must_use]
    pub fn pattern(self) -> &'static [f64] {
        match self {
            Self::ByLayer | Self::Continuous => &[],
            Self::Dashed => &[3.0, 1.5],
            Self::Dotted => &[0.25, 1.0],
            Self::DashDot => &[3.0, 1.0, 0.25, 1.0],
            Self::Hidden => &[1.5, 1.0],
            Self::Center => &[6.0, 1.0, 1.5, 1.0],
            Self::Border => &[3.0, 1.0, 3.0, 1.0, 0.25, 1.0],
        }
    }

    /// The linetype an object on `layer` draws with.
    #[must_use]
    pub fn resolved(self, layer: Option<&Layer>) -> Self {
        match self {
            Self::ByLayer => layer.map_or(Self::Continuous, |l| l.linetype),
            other => other,
        }
    }
}

/// A layer's linetype when its file names none.
fn continuous() -> Linetype {
    Linetype::Continuous
}

/// The layer every model starts with.
fn standard_layers() -> Vec<Layer> {
    vec![Layer {
        id: 0,
        name: "Default".to_owned(),
        colour: STANDARD_COLOUR,
        visible: true,
        locked: false,
        print_width: None,
        linetype: Linetype::Continuous,
    }]
}

fn first_new_layer() -> u64 {
    1
}

/// Everything undo restores and a file stores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The objects, oldest first.
    pub objects: Vec<std::sync::Arc<Object>>,
    /// The id the next object receives. Kept so ids are not reused after an undo.
    pub next_id: u64,
    /// The layers, oldest first.
    pub layers: Vec<Layer>,
    /// The layer new objects go on.
    pub current_layer: u64,
    /// The id the next layer receives.
    pub next_layer: u64,
    /// The model's unit.
    pub unit: Unit,
    /// The id the next group receives.
    pub next_group: u64,
    /// The block definitions.
    pub blocks: Vec<crate::blocks::BlockDefinition>,
    /// The id the next block definition receives.
    pub next_block: u64,
    /// A block definition being edited in place.
    pub block_edit: Option<crate::blocks::BlockEdit>,
}

impl State {
    /// Whether `other` is this state, told quickly: objects are the same when they are the
    /// same shared copy. Any change to an object makes a new copy of it (`Arc::make_mut`)
    /// while an earlier state still holds the old one, so this never misses a change; it may
    /// call two equal objects different, which only costs drawing them again.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        // Listed field by field, so that a new field cannot be forgotten here.
        let Self {
            objects,
            next_id,
            layers,
            current_layer,
            next_layer,
            unit,
            next_group,
            blocks,
            next_block,
            block_edit,
        } = self;
        objects.len() == other.objects.len()
            && objects
                .iter()
                .zip(&other.objects)
                .all(|(a, b)| std::sync::Arc::ptr_eq(a, b))
            && *next_id == other.next_id
            && *layers == other.layers
            && *current_layer == other.current_layer
            && *next_layer == other.next_layer
            && *unit == other.unit
            && *next_group == other.next_group
            && *blocks == other.blocks
            && *next_block == other.next_block
            && *block_edit == other.block_edit
    }
}

/// The unit a model is drawn in (spec `docs/specs/ui-look-and-layout.md` §7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Millimetres, the unit new models start in.
    #[default]
    Millimetres,
    /// Metres.
    Metres,
}

impl Unit {
    /// Its symbol, shown after numbers.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Metres => "m",
        }
    }

    /// How many millimetres one of it is.
    #[must_use]
    pub fn millimetres(self) -> f64 {
        match self {
            Self::Millimetres => 1.0,
            Self::Metres => 1000.0,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_id: 0,
            layers: standard_layers(),
            current_layer: 0,
            next_layer: first_new_layer(),
            unit: Unit::Millimetres,
            next_group: 0,
            blocks: Vec::new(),
            next_block: 0,
            block_edit: None,
        }
    }
}

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
                Shape::Body(_) | Shape::Instance(_) => 0,
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
            linetype: Linetype::ByLayer,
            user_text: std::collections::BTreeMap::new(),
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
            .filter(|object| !object.hidden && !object.locked && open(object.layer))
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
