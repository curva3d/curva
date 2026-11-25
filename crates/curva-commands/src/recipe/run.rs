//! Running a recipe as a command: asking, options, the preview and the result.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A preview made, with the answers and mode it was made from.
type Shown = std::cell::RefCell<Option<(Preview, Made)>>;

/// What a preview was made from: the answers with the pointer or typed number standing in for the
/// step asked, the mode, the options' values, and the pointer while a number is typed.
type Preview = (Vec<Value>, Option<&'static str>, Vec<f64>, Option<Point>);

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
    /// Whether a recipe that applies each answer has changed the model yet.
    applied: bool,
    /// Where the pointer was when the latest answer was given, for [`Context::cursor`].
    cursor: Option<Point>,
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
            cursor: self.cursor,
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
                    OptionKind::Name(_) => format!("{}={}", o.name, cx.name(o.name)),
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
                // Edges picked with Ctrl+Shift stand in for curves, as Rhino lets a command take
                // another object's edge where it asks for curves.
                if step.only == Only::Curves
                    && doc.selection().is_empty()
                    && !doc.edge_selection().is_empty()
                {
                    let lent = doc.lend_picked_edges();
                    return self.step(doc, Some(Answer::Objects(lent)));
                }
                if step.parts && doc.selection().is_empty() && doc.has_picked_parts() {
                    let owners = doc.picked_part_owners();
                    return self.step(doc, Some(Answer::Objects(owners)));
                }
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
        if let Some((option, words)) = step.choice {
            let shown = self.context(doc, &self.values).name(option);
            return Step::Ask(Request {
                prompt: Message::with(step.prompt, vec![shown]),
                allow_empty: true,
                options: words.to_vec(),
                ..Request::new(step.prompt, Kind::Choice)
            });
        }
        let (options, option_labels) = self.offered(doc, &step);
        // The points given so far, ending at the one this step measures from, so the drawing can
        // snap to its own points (a polyline closing on its start) as in Rhino.
        let preview = step.from.map_or_else(Vec::new, |from| {
            let mut points: Vec<Point> = self.values.iter().filter_map(Value::point).collect();
            if points.last() != Some(&from) {
                points.push(from);
            }
            points
        });
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
            panel: step.panel,
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
                            Only::Meshable => {
                                o.shape.body().is_some()
                                    || matches!(o.shape, curva_document::Shape::SubD(_))
                            }
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
        let memory = self.context(doc, &self.values).memory_key(option);
        doc.remember(&memory, value);
        self.ask(doc)
    }

    /// Sets a name option to what was typed: one of the recipe's choices, written as it gives
    /// it, or asked again when it is none of them.
    fn set_name(&mut self, doc: &mut Document, name: &'static str, typed: &str) -> Step {
        let typed = typed.trim();
        if typed.is_empty() {
            return self.ask(doc);
        }
        let choices = self.recipe.choices(&self.context(doc, &self.values), name);
        let chosen = match choices {
            None => Some(typed.to_owned()),
            Some(list) => list.into_iter().find(|c| c.eq_ignore_ascii_case(typed)),
        };
        let Some(chosen) = chosen else {
            self.typing_option = Some(name);
            return Step::Ask(Request {
                prompt: Message::with("prompt.option.no_such_name", vec![typed.to_owned()]),
                allow_empty: true,
                ..Request::new("prompt.option.no_such_name", Kind::Path)
            });
        };
        doc.remember_text(&key(R::NAME, name), &chosen);
        self.ask(doc)
    }

    fn finish(&mut self, doc: &mut Document) -> Step {
        doc.remove(&std::mem::take(&mut self.placed));
        let made = match self.recipe.make(&self.context(doc, &self.values)) {
            Ok(made) => made,
            Err(message) => return Step::Failed(message),
        };
        let message = made.message.clone();
        let view = made.view.clone();
        Self::apply(doc, made);
        match view {
            Some(view) => Step::View(view),
            None => Step::Done(message),
        }
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
        if let (Some((how, first, second)), [result]) = (made.live, added.as_slice()) {
            doc.make_live(*result, how, first, second);
        }
        for edit in made.edits {
            match edit {
                Edit::Lock(ids) => {
                    doc.lock(&ids);
                }
                Edit::UnlockAll => {
                    doc.unlock_all();
                }
                Edit::Group(ids) => {
                    doc.group(&ids);
                }
                Edit::Ungroup(ids) => {
                    doc.ungroup(&ids);
                }
                Edit::Select(ids) => doc.select(&ids),
                Edit::SectionStyle(ids, style) => doc.set_section_style(&ids, style),
                Edit::LayerSectionStyle(layers, style) => {
                    for layer in layers {
                        doc.set_layer_section_style(layer, style.clone());
                    }
                }
            }
        }
        added
    }

    /// Answers taken before the step's own: a number that sets an option, a pick that missed, and a word at a choice step.
    fn answered_early(&mut self, doc: &mut Document, answer: &Option<Answer>) -> Option<Step> {
        if let Some(Answer::Number(value)) = *answer
            && let Some(name) = self
                .steps(doc)
                .get(self.values.len())
                .and_then(|step| step.sets)
        {
            return Some(self.set_option(doc, name, value));
        }
        // A pick must land on a curve; a click on nothing asks again.
        if let Some(Answer::Point(at)) = *answer
            && self.steps(doc).get(self.values.len()).map(|s| s.kind) == Some(Kind::PickPart)
            && !matches!(crate::commands::curve_at(doc, at, ON_CURVE), Ok(Some(_)))
        {
            return Some(self.ask(doc));
        }
        // A word picked at a choice step sets its name option and answers the step.
        if let Some(Answer::Option(word)) = answer
            && let Some((option, words)) = self
                .steps(doc)
                .get(self.values.len())
                .and_then(|step| step.choice)
            && words.contains(word)
        {
            doc.remember_text(&key(R::NAME, option), word);
            return Some(self.step(doc, Some(Answer::Empty)));
        }
        None
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
            Some(OptionKind::Name(_)) => {
                self.typing_option = Some(name);
                let value = self.context(doc, &self.values).name(name);
                Step::Ask(Request {
                    prompt: Message::with("prompt.option.name", vec![name.to_owned(), value]),
                    allow_empty: true,
                    ..Request::new("prompt.option.name", Kind::Path)
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
    fn note_cursor(&mut self, cursor: Option<Point>) {
        self.cursor = cursor;
    }

    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if let Some(name) = self.typing_option.take() {
            return match answer {
                Some(Answer::Number(value)) => self.set_option(doc, name, value),
                Some(Answer::Path(typed)) => self.set_name(doc, name, &typed),
                _ => self.ask(doc),
            };
        }
        if let Some(step) = self.answered_early(doc, &answer) {
            return step;
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
            Some(Answer::MeshParts(picked)) if picked.is_empty() => return self.ask(doc),
            Some(Answer::MeshParts(picked)) => Value::MeshParts(picked),
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
        if R::APPLIES_EACH_ANSWER && self.steps(doc).len() > self.values.len() {
            match self.recipe.make(&self.context(doc, &self.values)) {
                Ok(made) => {
                    self.applied |= !(made.add.is_empty()
                        && made.replace.is_empty()
                        && made.remove.is_empty()
                        && made.derived.is_empty());
                    Self::apply(doc, made);
                }
                Err(message) => return Step::Failed(message),
            }
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
        !self.placed.is_empty() || self.applied
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
        for shape in &shown_shapes(&made, doc) {
            match shape {
                // Annotation previews as it will be drawn, with its style's arrows and text,
                // rather than as the bare lines between its points.
                Shape::Annotation(annotation) => shown.extend(
                    curva_document::shape::annotation_drawing(annotation, doc.state(), doc.space())
                        .lines,
                ),
                _ => shown.extend(shape.outlines()),
            }
        }
        shown
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<curva_kernel::Body> {
        let Some(made) = self.preview_made(doc, live) else {
            return Vec::new();
        };
        shown_shapes(&made, doc)
            .iter()
            .filter_map(|shape| shape.body().cloned())
            .collect()
    }
}

/// What a preview shows of `made`: its shapes, with block instances opened into the shapes they
/// draw, placed, so a block being copied, moved or placed shows too (#507: instances showed
/// nothing, as their lines live in their definition).
fn shown_shapes(made: &Made, doc: &Document) -> Vec<Shape> {
    let mut shown = Vec::new();
    for shape in made.shapes() {
        match shape {
            Shape::Instance(instance) => shown.extend(
                doc.instance_shapes(instance)
                    .into_iter()
                    .map(|(shape, _)| shape),
            ),
            shape => shown.push(shape.clone()),
        }
    }
    shown
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
        cx.cursor = live.cursor;
        cx.answered = self.values.len();
        let options: Vec<f64> = R::OPTIONS
            .iter()
            .map(|o| match o.kind {
                OptionKind::Toggle(_) => f64::from(u8::from(cx.toggle(o.name))),
                _ => cx.number(o.name),
            })
            .collect();
        // While a number is typed the answers stay the same as the pointer moves, so it is kept too.
        let typed_at = live.number.and(live.cursor);
        let from = (values.clone(), self.mode, options, typed_at);
        if let Some((was, made)) = &*self.shown.borrow()
            && *was == from
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
        *self.shown.borrow_mut() = Some((from, made.clone()));
        Some(made)
    }
}
