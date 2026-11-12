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
        }
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
    /// Lines that only guide while the command runs, such as from a centre to the pointer.
    pub guides: Vec<Curve>,
    /// What to report when done.
    pub message: Option<Message>,
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
        match self.options.iter().find(|o| o.name == name).map(|o| o.kind) {
            Some(OptionKind::Number(default)) => self
                .doc
                .remembered(&key(self.command, name))
                .unwrap_or(default),
            Some(OptionKind::Whole { default, .. }) => self
                .doc
                .remembered(&key(self.command, name))
                .unwrap_or(f64::from(default)),
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

    /// What the answers make. For the preview, `cx.values` may stop short, with the pointer or
    /// typed number standing in for the step being asked; return what can be shown so far.
    ///
    /// # Errors
    /// A message saying why nothing can be made from these answers.
    fn make(&self, cx: &Context<'_>) -> Result<Made, Message>;
}

/// A preview made, with the answers and mode it was made from.
type Shown = std::cell::RefCell<Option<(Vec<Value>, Option<&'static str>, Made)>>;

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
                    OptionKind::Number(_) | OptionKind::Whole { .. } => {
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
        // A selection made beforehand answers a step asking for objects, as in Rhino.
        if step.kind == Kind::Objects && !doc.selection().is_empty() {
            self.values.push(Value::Objects(doc.selection()));
            return self.ask(doc);
        }
        let (options, option_labels) = self.offered(doc, &step);
        let preview = step.from.into_iter().collect();
        Step::Ask(Request {
            allow_empty: step.optional,
            options,
            option_labels,
            preview,
            from: step.from,
            along: step.along,
            ..Request::new(step.prompt, step.kind)
        })
    }

    fn finish(&self, doc: &mut Document) -> Step {
        let made = match self.recipe.make(&self.context(doc, &self.values)) {
            Ok(made) => made,
            Err(message) => return Step::Failed(message),
        };
        for id in &made.remove {
            doc.remove(&[*id]);
        }
        for (id, shape) in made.replace {
            doc.replace_shape(id, shape);
        }
        for shape in made.add {
            doc.add_shape(shape);
        }
        Step::Done(made.message)
    }

    fn choose_option(&mut self, doc: &mut Document, name: &'static str) -> Step {
        match R::OPTIONS.iter().find(|o| o.name == name).map(|o| o.kind) {
            Some(OptionKind::Toggle(_)) => {
                let now = self.context(doc, &self.values).toggle(name);
                doc.remember(&key(R::NAME, name), f64::from(u8::from(!now)));
                self.ask(doc)
            }
            Some(OptionKind::Number(_) | OptionKind::Whole { .. }) => {
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
            if let (Some(Answer::Number(value)), Some(OptionKind::Whole { low, high, .. })) = (
                &answer,
                R::OPTIONS.iter().find(|o| o.name == name).map(|o| o.kind),
            ) && (value.fract() != 0.0 || *value < f64::from(low) || *value > f64::from(high))
            {
                return Step::Failed(Message::with(
                    "error.option_whole",
                    vec![name.to_owned(), low.to_string(), high.to_string()],
                ));
            }
            if let Some(Answer::Number(value)) = answer
                && value.is_finite()
            {
                doc.remember(&key(R::NAME, name), value);
            }
            return self.ask(doc);
        }
        let value = match answer {
            None => return self.ask(doc),
            Some(Answer::Option(name)) => return self.choose_option(doc, name),
            Some(Answer::Point(p)) => Value::Point(p),
            Some(Answer::Number(n)) => Value::Number(n),
            Some(Answer::Objects(ids)) => Value::Objects(ids),
            Some(Answer::Empty) => Value::Empty,
            Some(_) => return crate::commands::unexpected_step(),
        };
        if self.values.is_empty() {
            self.plane = Some(doc.work_plane());
        }
        self.values.push(value);
        if let Err(message) = self.recipe.check(&self.context(doc, &self.values)) {
            return Step::Failed(message);
        }
        self.ask(doc)
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(made) = self.preview_made(doc, live) else {
            return Vec::new();
        };
        let mut shown = made.guides;
        for shape in made.add.iter().chain(made.replace.iter().map(|(_, s)| s)) {
            shown.extend(shape.outlines());
        }
        shown
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<curva_kernel::Body> {
        let Some(made) = self.preview_made(doc, live) else {
            return Vec::new();
        };
        made.add
            .iter()
            .chain(made.replace.iter().map(|(_, s)| s))
            .filter_map(|shape| shape.body().cloned())
            .collect()
    }
}

impl<R: Recipe> RecipeCommand<R> {
    /// What the answers so far and the pointer or typed number would make, kept while they stay
    /// the same.
    fn preview_made(&self, doc: &Document, live: &Live) -> Option<Made> {
        let steps = self.steps(doc);
        let mut values = self.values.clone();
        if let Some(step) = steps.get(values.len()) {
            match step.kind {
                Kind::Point => values.extend(live.cursor.map(Value::Point)),
                // A typed number, or else the pointer's distance from where the step measures,
                // as a click would give.
                Kind::Number => values.extend(
                    live.number
                        .or_else(|| Some(curva_kernel::gap(live.cursor?, step.from?)))
                        .map(Value::Number),
                ),
                Kind::PointOrNumber => values.extend(
                    live.number
                        .map(Value::Number)
                        .or_else(|| live.cursor.map(Value::Point)),
                ),
                _ => {}
            }
        }
        if let Some((held, mode, made)) = &*self.shown.borrow()
            && *held == values
            && *mode == self.mode
        {
            return Some(made.clone());
        }
        let made = self.recipe.make(&self.context(doc, &values)).ok()?;
        *self.shown.borrow_mut() = Some((values, self.mode, made.clone()));
        Some(made)
    }
}
