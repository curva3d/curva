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

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
pub const ON_CURVE: f64 = 1e-3;

/// What an option holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionKind {
    /// Yes or no, flipped by choosing it; remembered between runs.
    Toggle(bool),
    /// A number, typed after choosing it; remembered between runs.
    Number(f64),
    /// A whole number from `low` to `high`, typed after choosing it, such as a curve's degree;
    /// remembered between runs.
    Whole {
        /// Its value until another is typed.
        default: u32,
        /// The least it may be.
        low: u32,
        /// The most it may be.
        high: u32,
    },
    /// A length, typed after choosing it: more than zero, or zero too when `zero` is set;
    /// remembered between runs.
    Distance {
        /// Its value until another is typed.
        default: f64,
        /// Whether zero is allowed, as for a fillet's radius.
        zero: bool,
    },
    /// A way of making the result, such as Arc's `StartPoint`: choosing it switches to it for this
    /// run, and the steps can change with it.
    Mode,
    /// Finishes the command at once in this way, such as `Close` on a polyline; the recipe sees
    /// it as the mode.
    Finish,
    /// Takes back the last answer, as Rhino's `Undo` option does while drawing.
    Undo,
}

/// One option of a command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionDef {
    /// Its name, as typed and shown.
    pub name: &'static str,
    /// What it holds.
    pub kind: OptionKind,
}

impl OptionDef {
    /// A yes-or-no option.
    #[must_use]
    pub const fn toggle(name: &'static str, default: bool) -> Self {
        Self {
            name,
            kind: OptionKind::Toggle(default),
        }
    }

    /// A number option.
    #[must_use]
    pub const fn number(name: &'static str, default: f64) -> Self {
        Self {
            name,
            kind: OptionKind::Number(default),
        }
    }

    /// A length option: more than zero, or zero too when `zero` is set.
    #[must_use]
    pub const fn distance(name: &'static str, default: f64, zero: bool) -> Self {
        Self {
            name,
            kind: OptionKind::Distance { default, zero },
        }
    }

    /// Why `value` cannot be this option's value, if it cannot.
    fn refuses(&self, value: f64) -> Option<Message> {
        let name = self.name.to_owned();
        match self.kind {
            OptionKind::Whole { low, high, .. }
                if value.fract() != 0.0 || value < f64::from(low) || value > f64::from(high) =>
            {
                Some(Message::with(
                    "error.option_whole",
                    vec![name, low.to_string(), high.to_string()],
                ))
            }
            OptionKind::Distance { zero: true, .. } if value < 0.0 => {
                Some(Message::with("error.option_negative", vec![name]))
            }
            OptionKind::Distance { zero: false, .. } if value <= 0.0 => {
                Some(Message::with("error.option_positive", vec![name]))
            }
            _ if !value.is_finite() => Some(Message::with("error.bad_number", vec![name])),
            _ => None,
        }
    }

    /// A whole-number option, from `low` to `high`.
    #[must_use]
    pub const fn whole(name: &'static str, default: u32, low: u32, high: u32) -> Self {
        Self {
            name,
            kind: OptionKind::Whole { default, low, high },
        }
    }

    /// A way of making the result.
    #[must_use]
    pub const fn mode(name: &'static str) -> Self {
        Self {
            name,
            kind: OptionKind::Mode,
        }
    }

    /// An option that finishes the command in its own way.
    #[must_use]
    pub const fn finish(name: &'static str) -> Self {
        Self {
            name,
            kind: OptionKind::Finish,
        }
    }

    /// Rhino's `Undo` while drawing: takes back the last answer.
    #[must_use]
    pub const fn undo() -> Self {
        Self {
            name: "Undo",
            kind: OptionKind::Undo,
        }
    }
}

/// One step of a recipe: what it asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct StepDef {
    /// The prompt's key in the translation file.
    pub prompt: &'static str,
    /// The sort of answer.
    pub kind: Kind,
    /// Where it measures from: typed distances and angles, and the cursor tag, start here.
    pub from: Option<Point>,
    /// For a size along a direction: where it starts and the direction, of length 1.
    pub along: Option<(Point, Point)>,
    /// Whether Enter with nothing typed answers it.
    pub optional: bool,
    /// Which of the recipe's options this step offers: all of them unless a list is given.
    pub options: Option<Vec<&'static str>>,
    /// The number option a number typed at this step sets, as Rhino's Offset takes a distance at
    /// its side prompt; the step is then asked again.
    pub sets: Option<&'static str>,
    /// A number step whose answer is remembered under this name: Enter then gives the last one
    /// (or the starting value), shown in the prompt, as Rhino offers its defaults.
    pub remembered: Option<(&'static str, f64)>,
    /// Whether it picks a rectangle's far corner, so the tag shows width × depth.
    pub corner: bool,
    /// For objects: which sort it takes.
    pub only: Only,
    /// For objects: how many it needs; with fewer it asks again.
    pub least: usize,
    /// For objects: how many it takes at most; with more it asks again.
    pub most: usize,
    /// For objects: whether objects chosen at an earlier step are left out.
    pub apart: bool,
}

/// Which sort of objects a step takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Only {
    /// Any object.
    #[default]
    Any,
    /// Curves.
    Curves,
    /// Surfaces and solids.
    Bodies,
}

impl StepDef {
    fn new(prompt: &'static str, kind: Kind) -> Self {
        Self {
            prompt,
            kind,
            from: None,
            along: None,
            optional: false,
            options: None,
            sets: None,
            remembered: None,
            corner: false,
            only: Only::Any,
            least: 1,
            most: usize::MAX,
            apart: false,
        }
    }

    /// Asks for a spot on a curve, clicked; the answer is the exact point on it.
    #[must_use]
    pub fn pick(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::PickPart)
    }

    /// Asks for a position.
    #[must_use]
    pub fn point(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Point)
    }

    /// Asks for a position, or a typed number instead, such as Arc's end point or angle.
    #[must_use]
    pub fn point_or_number(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::PointOrNumber)
    }

    /// Asks for a number, or a click whose distance from [`Self::from`] gives it.
    #[must_use]
    pub fn number(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Number)
    }

    /// Asks for a size along `direction` from `origin`, following the snapped mouse.
    #[must_use]
    pub fn along(prompt: &'static str, origin: Point, direction: Point) -> Self {
        Self {
            from: Some(origin),
            along: curva_kernel::unit(direction).map(|d| (origin, d)),
            ..Self::new(prompt, Kind::Number)
        }
    }

    /// Asks for edges of surfaces and solids, picked in the viewport.
    #[must_use]
    pub fn edges(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Edges)
    }

    /// Asks for faces of surfaces and solids, picked in the viewport.
    #[must_use]
    pub fn faces(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Faces)
    }

    /// Asks for objects; a selection made beforehand answers it.
    #[must_use]
    pub fn objects(prompt: &'static str) -> Self {
        Self::new(prompt, Kind::Objects)
    }

    /// Measures from `base`.
    #[must_use]
    pub fn from(mut self, base: Point) -> Self {
        self.from = Some(base);
        self
    }

    /// Enter with nothing typed answers it.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Offers only these of the recipe's options at this step.
    #[must_use]
    pub fn offering(mut self, options: &[&'static str]) -> Self {
        self.options = Some(options.to_vec());
        self
    }

    /// Remembers the number given under `name`; Enter gives it again, `starting` at first.
    #[must_use]
    pub fn remembered(mut self, name: &'static str, starting: f64) -> Self {
        self.remembered = Some((name, starting));
        self.optional = true;
        self
    }

    /// Picks a rectangle's far corner: the pointer's tag shows width × depth.
    #[must_use]
    pub fn corner(mut self) -> Self {
        self.corner = true;
        self
    }

    /// Takes only curves.
    #[must_use]
    pub fn curves(mut self) -> Self {
        self.only = Only::Curves;
        self
    }

    /// Takes only surfaces and solids.
    #[must_use]
    pub fn bodies(mut self) -> Self {
        self.only = Only::Bodies;
        self
    }

    /// Needs at least `count` objects.
    #[must_use]
    pub fn at_least(mut self, count: usize) -> Self {
        self.least = count;
        self
    }

    /// Takes exactly one object, as Sweep1's rail.
    #[must_use]
    pub fn one(mut self) -> Self {
        self.least = 1;
        self.most = 1;
        self
    }

    /// Leaves out objects chosen at an earlier step, as Rhino's Boolean commands do.
    #[must_use]
    pub fn apart(mut self) -> Self {
        self.apart = true;
        self
    }

    /// A number typed at this step sets the number option `option`, and the step is asked again.
    #[must_use]
    pub fn number_sets(mut self, option: &'static str) -> Self {
        self.sets = Some(option);
        self
    }
}

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

/// A preview made, with the answers and mode it was made from.
type Shown = std::cell::RefCell<Option<(Vec<Value>, Option<&'static str>, Vec<f64>, Made)>>;

/// Runs a [`Recipe`] as a [`Command`].
#[derive(Default)]
pub struct RecipeCommand<R: Recipe> {
    recipe: R,
    values: Vec<Value>,
    mode: Option<&'static str>,
    /// Waiting for a number option's new value.
    typing_option: Option<&'static str>,
    /// The last preview made and the answers it was made from, so the lines and the ghosted
    /// solids of one frame come from one `make`.
    shown: Shown,
    /// The construction plane when the first answer came.
    plane: Option<[Point; 2]>,
    /// What a recipe that places as it goes has put in the model so far.
    placed: Vec<u64>,
}

impl<R: Recipe> RecipeCommand<R> {
    fn context<'a>(&self, doc: &'a Document, values: &'a [Value]) -> Context<'a> {
        Context {
            doc,
            values,
            mode: self.mode,
            command: R::NAME,
            options: R::OPTIONS,
            plane: self.plane,
            held: None,
            previewing: false,
            answered: values.len(),
        }
    }

    fn steps(&self, doc: &Document) -> Vec<StepDef> {
        self.recipe.steps(&self.context(doc, &self.values))
    }

    /// The options a step offers, and how each shows in the prompt.
    fn offered(&self, doc: &Document, step: &StepDef) -> (Vec<&'static str>, Vec<String>) {
        let cx = self.context(doc, &self.values);
        R::OPTIONS
            .iter()
            .filter(|o| {
                step.options
                    .as_ref()
                    .is_none_or(|only| only.contains(&o.name))
            })
            .map(|o| {
                let label = match o.kind {
                    OptionKind::Toggle(_) => {
                        format!(
                            "{}={}",
                            o.name,
                            if cx.toggle(o.name) { "Yes" } else { "No" }
                        )
                    }
                    OptionKind::Number(_)
                    | OptionKind::Whole { .. }
                    | OptionKind::Distance { .. } => {
                        format!("{}={}", o.name, cx.number(o.name))
                    }
                    OptionKind::Mode | OptionKind::Finish | OptionKind::Undo => o.name.to_owned(),
                };
                (o.name, label)
            })
            .unzip()
    }

    fn ask(&mut self, doc: &mut Document) -> Step {
        let steps = self.steps(doc);
        let Some(step) = steps.get(self.values.len()).cloned() else {
            return self.finish(doc);
        };
        if step.kind == Kind::Objects {
            if self.values.is_empty() {
                // A selection made beforehand answers the first step asking for objects, as in
                // Rhino, when it has enough of the right sort.
                let chosen = self.chosen(doc, &step, doc.selection());
                if (step.least..=step.most).contains(&chosen.len()) {
                    return self.step(doc, Some(Answer::Objects(chosen)));
                }
            } else {
                // What was chosen before must not count as chosen again.
                doc.select(&[]);
            }
        }
        let (options, option_labels) = self.offered(doc, &step);
        let preview = step.from.into_iter().collect();
        // A step whose typed number sets an option takes numbers as well as positions.
        let kind = if step.sets.is_some() && step.kind == Kind::Point {
            Kind::PointOrNumber
        } else {
            step.kind
        };
        let prompt = match step.remembered {
            Some((name, starting)) => Message::with(
                step.prompt,
                vec![
                    doc.remembered(&key(R::NAME, name))
                        .unwrap_or(starting)
                        .to_string(),
                ],
            ),
            None => Message::new(step.prompt),
        };
        Step::Ask(Request {
            prompt,
            allow_empty: step.optional,
            options,
            option_labels,
            preview,
            from: step.from,
            along: step.along,
            corner: step.corner,
            ..Request::new(step.prompt, kind)
        })
    }

    /// The objects among `ids` that `step` takes.
    fn chosen(&self, doc: &Document, step: &StepDef, ids: Vec<u64>) -> Vec<u64> {
        let earlier: Vec<u64> = if step.apart {
            self.values
                .iter()
                .filter_map(|v| match v {
                    Value::Objects(ids) => Some(ids.clone()),
                    _ => None,
                })
                .flatten()
                .collect()
        } else {
            Vec::new()
        };
        ids.into_iter()
            .filter(|id| !earlier.contains(id))
            .filter(|id| {
                doc.state().objects.iter().any(|o| {
                    o.id == *id
                        && match step.only {
                            Only::Any => true,
                            Only::Curves => o.curve().is_some(),
                            Only::Bodies => o.shape.body().is_some(),
                        }
                })
            })
            .collect()
    }

    /// Takes a typed value for the option `name`, unless the option refuses it.
    fn set_option(&mut self, doc: &mut Document, name: &'static str, value: f64) -> Step {
        let Some(option) = R::OPTIONS.iter().find(|o| o.name == name) else {
            return crate::commands::unexpected_step();
        };
        if let Some(message) = option.refuses(value) {
            return Step::Failed(message);
        }
        doc.remember(&key(R::NAME, name), value);
        self.ask(doc)
    }

    fn finish(&mut self, doc: &mut Document) -> Step {
        doc.remove(&std::mem::take(&mut self.placed));
        let made = match self.recipe.make(&self.context(doc, &self.values)) {
            Ok(made) => made,
            Err(message) => return Step::Failed(message),
        };
        let message = made.message.clone();
        Self::apply(doc, made);
        Step::Done(message)
    }

    /// Puts what was made into the model, and returns the ids of the objects added.
    fn apply(doc: &mut Document, made: Made) -> Vec<u64> {
        let mut added = Vec::new();
        // Layers are looked up first, as a source may be among the objects taken away.
        let layers: Vec<Option<u64>> = made.derived.iter().map(|d| doc.layer_of(d.from)).collect();
        for id in &made.remove {
            doc.remove(&[*id]);
        }
        for (id, shape) in made.replace {
            doc.replace_shape(id, shape);
        }
        for shape in made.add {
            added.push(doc.add_shape(shape));
        }
        for (derived, layer) in made.derived.into_iter().zip(layers) {
            let id = doc.add_shape_on(derived.shape, layer);
            if let Some(history) = derived.history {
                doc.record_recipe(id, history);
            }
            added.push(id);
        }
        added
    }

    fn choose_option(&mut self, doc: &mut Document, name: &'static str) -> Step {
        match R::OPTIONS.iter().find(|o| o.name == name).map(|o| o.kind) {
            Some(OptionKind::Toggle(_)) => {
                let now = self.context(doc, &self.values).toggle(name);
                doc.remember(&key(R::NAME, name), f64::from(u8::from(!now)));
                self.ask(doc)
            }
            Some(
                OptionKind::Number(_) | OptionKind::Whole { .. } | OptionKind::Distance { .. },
            ) => {
                self.typing_option = Some(name);
                let value = self.context(doc, &self.values).number(name);
                Step::Ask(Request {
                    prompt: Message::with(
                        "prompt.option.number",
                        vec![name.to_owned(), value.to_string()],
                    ),
                    allow_empty: true,
                    ..Request::new("prompt.option.number", Kind::Number)
                })
            }
            Some(OptionKind::Mode) => {
                self.mode = Some(name);
                self.ask(doc)
            }
            Some(OptionKind::Finish) => {
                self.mode = Some(name);
                if let Err(message) = self.recipe.check(&self.context(doc, &self.values)) {
                    return Step::Failed(message);
                }
                self.finish(doc)
            }
            Some(OptionKind::Undo) => {
                self.values.pop();
                self.ask(doc)
            }
            None => crate::commands::unexpected_step(),
        }
    }
}

impl<R: Recipe> Command for RecipeCommand<R> {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if let Some(name) = self.typing_option.take() {
            return match answer {
                Some(Answer::Number(value)) => self.set_option(doc, name, value),
                _ => self.ask(doc),
            };
        }
        if let Some(Answer::Number(value)) = answer
            && let Some(name) = self
                .steps(doc)
                .get(self.values.len())
                .and_then(|step| step.sets)
        {
            return self.set_option(doc, name, value);
        }
        // A pick must land on a curve; a click on nothing asks again.
        if let Some(Answer::Point(at)) = answer
            && self.steps(doc).get(self.values.len()).map(|s| s.kind) == Some(Kind::PickPart)
            && !matches!(crate::commands::curve_at(doc, at, ON_CURVE), Ok(Some(_)))
        {
            return self.ask(doc);
        }
        let value = match answer {
            None => return self.ask(doc),
            Some(Answer::Option(name)) => return self.choose_option(doc, name),
            Some(Answer::Point(p)) => Value::Point(p),
            Some(Answer::Number(n)) => Value::Number(n),
            Some(Answer::Objects(ids)) => {
                let step = self.steps(doc).get(self.values.len()).cloned();
                let Some(step) = step.filter(|s| s.kind == Kind::Objects) else {
                    return crate::commands::unexpected_step();
                };
                let chosen = self.chosen(doc, &step, ids);
                if !(step.least..=step.most).contains(&chosen.len()) {
                    return self.ask(doc);
                }
                Value::Objects(chosen)
            }
            // Picking nothing asks again.
            Some(Answer::Edges(picked) | Answer::Faces(picked)) if picked.is_empty() => {
                return self.ask(doc);
            }
            Some(Answer::Edges(picked)) => Value::Edges(picked),
            Some(Answer::Faces(picked)) => Value::Faces(picked),
            Some(Answer::Empty) => match self
                .steps(doc)
                .get(self.values.len())
                .and_then(|s| s.remembered)
            {
                Some((name, starting)) => {
                    Value::Number(doc.remembered(&key(R::NAME, name)).unwrap_or(starting))
                }
                None => Value::Empty,
            },
            Some(_) => return crate::commands::unexpected_step(),
        };
        if self.values.is_empty() {
            self.plane = Some(doc.work_plane());
        }
        if let (Some(Value::Number(n)), Some((name, _))) = (
            Some(&value),
            self.steps(doc)
                .get(self.values.len())
                .and_then(|s| s.remembered),
        ) {
            doc.remember(&key(R::NAME, name), *n);
        }
        self.values.push(value);
        if !self.recipe.accepts(&self.context(doc, &self.values)) {
            self.values.pop();
            return self.ask(doc);
        }
        if let Err(message) = self.recipe.check(&self.context(doc, &self.values)) {
            return Step::Failed(message);
        }
        if R::PLACES_AS_IT_GOES && self.steps(doc).len() > self.values.len() {
            doc.remove(&std::mem::take(&mut self.placed));
            match self.recipe.make(&self.context(doc, &self.values)) {
                Ok(made) => self.placed = Self::apply(doc, made),
                Err(message) => return Step::Failed(message),
            }
        }
        self.ask(doc)
    }

    fn keeps_on_cancel(&self) -> bool {
        !self.placed.is_empty()
    }

    fn preview_replaces(&self, doc: &Document, live: &Live) -> Vec<u64> {
        self.preview_made(doc, live)
            .map(|made| made.replace.iter().map(|(id, _)| *id).collect())
            .unwrap_or_default()
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(made) = self.preview_made(doc, live) else {
            return Vec::new();
        };
        let mut shown = made.guides.clone();
        for shape in made.shapes() {
            shown.extend(shape.outlines());
        }
        shown
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<curva_kernel::Body> {
        let Some(made) = self.preview_made(doc, live) else {
            return Vec::new();
        };
        made.shapes()
            .filter_map(|shape| shape.body().cloned())
            .collect()
    }
}

impl<R: Recipe> RecipeCommand<R> {
    /// What the answers so far and the pointer or typed number would make, kept while they and
    /// the options stay the same.
    fn preview_made(&self, doc: &Document, live: &Live) -> Option<Made> {
        let steps = self.steps(doc);
        let mut values = self.values.clone();
        let mut held = None;
        if let Some(step) = steps.get(values.len()) {
            match (step.kind, step.sets, live.number) {
                // A number being typed for the option the step sets shows at once.
                (Kind::Point | Kind::PickPart, Some(name), Some(number)) => {
                    held = Some((name, number));
                    values.extend(live.cursor.map(Value::Point));
                }
                (Kind::Point | Kind::PickPart, _, _) => {
                    values.extend(live.cursor.map(Value::Point))
                }
                // A typed number, or else the pointer's distance from where the step measures,
                // as a click would give.
                (Kind::Number, _, _) => values.extend(
                    live.number
                        .or_else(|| {
                            let cursor = live.cursor?;
                            // Along a direction the size is signed, as a click gives it.
                            Some(match step.along {
                                Some((origin, direction)) => {
                                    curva_kernel::dot(curva_kernel::sub(cursor, origin), direction)
                                }
                                None => curva_kernel::gap(cursor, step.from?),
                            })
                        })
                        // With nothing to measure yet, a remembered size shows, as Enter would give.
                        .filter(|size| size.abs() > f64::EPSILON)
                        .or_else(|| {
                            let (name, starting) = step.remembered?;
                            Some(doc.remembered(&key(R::NAME, name)).unwrap_or(starting))
                        })
                        .map(Value::Number),
                ),
                (Kind::PointOrNumber, _, _) => values.extend(
                    live.number
                        .map(Value::Number)
                        .or_else(|| live.cursor.map(Value::Point)),
                ),
                _ => {}
            }
        }
        let mut cx = self.context(doc, &values);
        cx.held = held;
        cx.previewing = true;
        cx.answered = self.values.len();
        let options: Vec<f64> = R::OPTIONS
            .iter()
            .map(|o| match o.kind {
                OptionKind::Toggle(_) => f64::from(u8::from(cx.toggle(o.name))),
                _ => cx.number(o.name),
            })
            .collect();
        if let Some((was, mode, with, made)) = &*self.shown.borrow()
            && *was == values
            && *mode == self.mode
            && *with == options
        {
            return Some(made.clone());
        }
        let mut made = self.recipe.make(&cx).ok()?;
        // What is already in the model is not shown again.
        let placed = self.placed.len();
        let from_add = placed.min(made.add.len());
        made.add.drain(..from_add);
        made.derived
            .drain(..(placed - from_add).min(made.derived.len()));
        *self.shown.borrow_mut() = Some((values.clone(), self.mode, options, made.clone()));
        Some(made)
    }
}
