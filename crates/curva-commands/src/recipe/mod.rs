//! The command framework (spec `docs/specs/command-framework.md`). A command written as a
//! [`Recipe`] states only what is its own: its options, the steps it asks for given the answers so
//! far, and what the answers make. Everything else is done here once for every command: asking,
//! options in Rhino's style and remembered between runs, typed input, the live preview (which is
//! always exactly the result), and adding the result to the model.
//!
//! Typed distances and angles for points, and sizes that follow the mouse, come from the
//! processor and [`Request::along`], so every recipe step gets them without asking.

use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Curve, Point};

mod options;
mod run;
mod steps;
pub use options::*;
pub use run::*;
pub use steps::*;

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
pub const ON_CURVE: f64 = 1e-3;

/// An answer to a step.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A position.
    Point(Point),
    /// A number.
    Number(f64),
    /// Chosen objects.
    Objects(Vec<u64>),
    /// Picked edges of surfaces and solids.
    Edges(Vec<curva_document::EdgePick>),
    /// Picked faces of surfaces and solids.
    Faces(Vec<curva_document::FacePick>),
    /// Enter on an optional step.
    Empty,
}

impl Value {
    /// The position, if it is one.
    #[must_use]
    pub fn point(&self) -> Option<Point> {
        match self {
            Self::Point(p) => Some(*p),
            _ => None,
        }
    }

    /// The number, if it is one.
    #[must_use]
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }
}

/// What a recipe's answers make, for the model or for the preview.
#[derive(Clone, Debug, Default)]
pub struct Made {
    /// New objects.
    pub add: Vec<Shape>,
    /// Objects changed in place, keeping their ids and layers.
    pub replace: Vec<(u64, Shape)>,
    /// Objects taken away.
    pub remove: Vec<u64>,
    /// New objects made from existing ones, which go on their source's layer.
    pub derived: Vec<Derived>,
    /// Lines that only guide while the command runs, such as from a centre to the pointer.
    pub guides: Vec<Curve>,
    /// What to report when done.
    pub message: Option<Message>,
    /// A live boolean: the one result made keeps these inputs, hidden, and follows them.
    pub live: Option<(curva_document::LiveHow, Vec<u64>, Vec<u64>)>,
    /// Changes to objects' state rather than their shapes, made after everything else.
    pub edits: Vec<Edit>,
    /// A change of view to ask the interface for when done, as Plan asks.
    pub view: Option<crate::ViewRequest>,
}

impl Made {
    /// Everything it adds or changes, for the preview.
    fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.add
            .iter()
            .chain(self.replace.iter().map(|(_, s)| s))
            .chain(self.derived.iter().map(|d| &d.shape))
    }
}

/// A change to objects' state rather than their shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Locks these objects.
    Lock(Vec<u64>),
    /// Unlocks every locked object.
    UnlockAll,
    /// Puts these objects in one new group.
    Group(Vec<u64>),
    /// Breaks up the groups of these objects.
    Ungroup(Vec<u64>),
    /// Selects these objects, and only these.
    Select(Vec<u64>),
}

/// A new object made from an existing one, such as an offset.
#[derive(Clone, Debug)]
pub struct Derived {
    /// The new object.
    pub shape: Shape,
    /// The object it was made from, whose layer it goes on.
    pub from: u64,
    /// How it follows its source when that changes, if it does (`History`).
    pub history: Option<curva_document::Recipe>,
}

/// The steps of a run of points ended by Enter, as Polyline and Curve take them: the first point,
/// then one more each time, which Enter may answer once `enough` points are placed. Each point is
/// measured from the one before, so typed distances and angles work. A run already ended by Enter
/// asks nothing more.
#[must_use]
pub fn point_run(
    cx: &Context<'_>,
    first: &'static str,
    next: &'static str,
    enough: usize,
) -> Vec<StepDef> {
    let points: Vec<Point> = cx.values.iter().filter_map(Value::point).collect();
    let mut steps: Vec<StepDef> = cx
        .values
        .iter()
        .enumerate()
        .map(|(i, _)| StepDef::point(if i == 0 { first } else { next }))
        .collect();
    if cx.values.last() == Some(&Value::Empty) {
        return steps;
    }
    let mut step = StepDef::point(if points.is_empty() { first } else { next });
    if let Some(last) = points.last() {
        step = step.from(*last);
    }
    if points.len() >= enough {
        step = step.optional();
    }
    steps.push(step);
    steps
}

/// The points of a run made by [`point_run`].
#[must_use]
pub fn run_points(cx: &Context<'_>) -> Vec<Point> {
    cx.values.iter().filter_map(Value::point).collect()
}

/// What a recipe sees: the model, the answers so far, the mode chosen, and its options' values.
pub struct Context<'a> {
    /// The model.
    pub doc: &'a Document,
    /// The answers so far, first step first.
    pub values: &'a [Value],
    /// The way of making chosen for this run, if any.
    pub mode: Option<&'static str>,
    command: &'static str,
    options: &'static [OptionDef],
    /// The construction plane when the first answer came, which the command keeps.
    plane: Option<[Point; 2]>,
    /// For the preview: a number being typed for an option, standing in for its value.
    held: Option<(&'static str, f64)>,
    /// Whether this is for the preview rather than the result, so a recipe can show less of a
    /// heavy change.
    pub previewing: bool,
    /// How many of `values` are real answers; for the preview, the rest stand in for the step
    /// being asked.
    pub answered: usize,
}

impl Context<'_> {
    /// A yes-or-no option's value.
    #[must_use]
    pub fn toggle(&self, name: &str) -> bool {
        match self.options.iter().find(|o| o.name == name).map(|o| o.kind) {
            Some(OptionKind::Toggle(default)) => self
                .doc
                .remembered(&key(self.command, name))
                .map_or(default, |v| v != 0.0),
            _ => false,
        }
    }

    /// A number option's value.
    #[must_use]
    pub fn number(&self, name: &str) -> f64 {
        if let Some((held, value)) = self.held
            && held == name
        {
            return value;
        }
        match self.options.iter().find(|o| o.name == name).map(|o| o.kind) {
            Some(OptionKind::Number(default)) => self
                .doc
                .remembered(&key(self.command, name))
                .unwrap_or(default),
            Some(OptionKind::Whole { default, .. }) => self
                .doc
                .remembered(&key(self.command, name))
                .unwrap_or(f64::from(default)),
            Some(OptionKind::Distance { default, .. }) => self
                .doc
                .remembered(&key(self.command, name))
                .unwrap_or(default),
            _ => 0.0,
        }
    }

    /// The construction plane's X and Y directions: the one in use when the first answer came,
    /// kept for the rest of the command, as in Rhino, even if another viewport is used later.
    #[must_use]
    pub fn plane(&self) -> [Point; 2] {
        self.plane.unwrap_or_else(|| self.doc.work_plane())
    }
}

/// Where an option's value is remembered.
fn key(command: &str, option: &str) -> String {
    format!("{command}.{option}").to_lowercase()
}

/// A command stated as its options, its steps and what it makes.
pub trait Recipe: Default + 'static {
    /// Its name, which also keys its remembered options.
    const NAME: &'static str;
    /// Its options; none by default.
    const OPTIONS: &'static [OptionDef] = &[];
    /// Whether what the answers make goes into the model after each answer, as Rhino's Copy
    /// places each copy at once, so later answers can snap to it. Off by default.
    const PLACES_AS_IT_GOES: bool = false;
    /// Whether what each answer makes goes into the model for good as soon as it is given, as
    /// Rhino's Trim cuts at each click. `make` then makes only what the last answer changes, from
    /// the model as it now is; the run stays one undo step and Escape keeps what was done. Off by
    /// default.
    const APPLIES_EACH_ANSWER: bool = false;

    /// The steps to ask, given the answers and mode so far. The list may change as they come in,
    /// and the command finishes once every step in it is answered.
    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef>;

    /// Checks the answers so far, after each one, so a command stops as soon as it cannot go on
    /// (a flat base, points in a line). Nothing to check by default.
    ///
    /// # Errors
    /// A message saying why the command cannot go on.
    fn check(&self, _cx: &Context<'_>) -> Result<(), Message> {
        Ok(())
    }

    /// Whether the answer just given can be taken. If not, it is dropped and the step asked
    /// again, as Revolve asks again for an angle over a whole turn. Every answer is taken by
    /// default.
    fn accepts(&self, _cx: &Context<'_>) -> bool {
        true
    }

    /// What the answers make. For the preview, `cx.values` may stop short, with the pointer or
    /// typed number standing in for the step being asked; return what can be shown so far.
    ///
    /// # Errors
    /// A message saying why nothing can be made from these answers.
    fn make(&self, cx: &Context<'_>) -> Result<Made, Message>;
}
