//! Runs commands: turns typed text and clicks into answers, and makes each command one undo step.

use crate::commands::{CommandDef, autofilled, completions, resolve};
use crate::{Answer, Command, Kind, Live, Lock, Message, Request, Step, Translations, ViewRequest};
use curva_document::units::parse_length;
use curva_document::{Document, State, Unit};
use curva_kernel::{Curve, Point, cross};

struct Active {
    def: &'static CommandDef,
    command: Box<dyn Command>,
    request: Option<Request>,
    before: State,
    /// The selection when the command began, which an undo of it brings back.
    selection_before: Vec<u64>,
    /// Started with a dash, such as `-Open`: file locations are typed, not picked in a dialog.
    typed: bool,
}

/// The command line's engine.
pub struct Processor {
    active: Option<Active>,
    last: Option<&'static CommandDef>,
    last_point: Option<Point>,
    /// A distance or angle typed for the next point, until it is placed.
    lock: Lock,
    log: Vec<String>,
    text: Translations,
    view_requests: std::collections::VecDeque<ViewRequest>,
    /// Whether the interface shows file dialogs; off for scripts and agents.
    dialogs: bool,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            active: None,
            last: None,
            last_point: None,
            lock: Lock::default(),
            log: Vec::new(),
            text: Translations::english(),
            view_requests: std::collections::VecDeque::new(),
            dialogs: false,
        }
    }
}

/// A distance or angle typed for a point measured from a base.
#[derive(Debug, PartialEq)]
enum TypedLock {
    /// `25`: hold the distance.
    Distance(f64),
    /// `<30`: hold the angle, in degrees.
    Angle(f64),
    /// `25<30`: both, which places the point at once.
    Both(f64, f64),
}

/// Reads `25`, `<30` or `25<30`. A positive, finite distance only, in any typed unit; any
/// finite angle.
fn parse_lock(text: &str, unit: Unit) -> Option<TypedLock> {
    let distance = |part: &str| parse_length(part, unit).filter(|d| *d > 0.0);
    let angle = |part: &str| part.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    match text.split_once('<') {
        None => distance(text).map(TypedLock::Distance),
        Some((before, after)) if before.trim().is_empty() => angle(after).map(TypedLock::Angle),
        Some((before, after)) => Some(TypedLock::Both(distance(before)?, angle(after)?)),
    }
}

/// Where typed coordinates are measured: the origin and the X and Y directions of a plane.
type Frame = (Point, [Point; 2]);

/// Reads `x,y` or `x,y,z` on the plane, as in Rhino, each in any typed unit (`5',3'`). `r` or
/// `@` in front means relative to the last point, and `w` means world coordinates whatever the
/// plane.
fn parse_point(
    text: &str,
    last: Option<Point>,
    (origin, [x, y]): Frame,
    unit: Unit,
) -> Option<Point> {
    let lowered = text.trim().to_lowercase();
    let (world, rest) = match lowered.strip_prefix('w') {
        Some(rest) => (true, rest),
        None => (false, lowered.as_str()),
    };
    let (relative, body) = match rest.strip_prefix(['r', '@']) {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let values: Vec<f64> = body
        .split(',')
        .map(|part| parse_length(part, unit))
        .collect::<Option<_>>()?;
    let offset = match values.as_slice() {
        [a, b] => [*a, *b, 0.0],
        [a, b, c] => [*a, *b, *c],
        _ => return None,
    };
    let along = if world {
        offset
    } else {
        let normal = cross(x, y);
        let axis = |i: usize| x[i] * offset[0] + y[i] * offset[1] + normal[i] * offset[2];
        [axis(0), axis(1), axis(2)]
    };
    let base = match (relative, world) {
        (true, _) => last.unwrap_or([0.0; 3]),
        (false, true) => [0.0; 3],
        (false, false) => origin,
    };
    Some([base[0] + along[0], base[1] + along[1], base[2] + along[2]])
}

impl Processor {
    /// What the running command is asking, ready to show.
    #[must_use]
    pub fn prompt(&self) -> Option<String> {
        let request = self.request()?;
        let mut shown = self.text.render(&request.prompt);
        if !request.option_labels.is_empty() {
            shown = format!("{shown} ( {} )", request.option_labels.join("  "));
        } else if !request.options.is_empty() {
            shown = format!("{shown} ({})", request.options.join(", "));
        }
        Some(shown)
    }

    /// The running command's question alone, without its options, for a prompt that shows the
    /// options as buttons.
    #[must_use]
    pub fn question(&self) -> Option<String> {
        Some(self.text.render(&self.request()?.prompt))
    }

    /// The options the current question offers: each one's name, to answer with, and how it shows
    /// ("Diameter=No").
    #[must_use]
    pub fn options(&self) -> Vec<(&'static str, String)> {
        let Some(request) = self.request() else {
            return Vec::new();
        };
        request
            .options
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let label = request
                    .option_labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| (*name).to_owned());
                (*name, label)
            })
            .collect()
    }

    fn request(&self) -> Option<&Request> {
        self.active.as_ref()?.request.as_ref()
    }

    /// The running command's current question.
    #[must_use]
    pub fn current_request(&self) -> Option<&Request> {
        self.request()
    }

    /// Whether a command is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Messages so far, oldest first.
    #[must_use]
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// The curves the running command would make or move, for the viewport to draw.
    #[must_use]
    pub fn preview(&self, doc: &Document, cursor: Option<Point>, typed: &str) -> Vec<Curve> {
        let live = self.live(doc, cursor, typed);
        self.active
            .as_ref()
            .map_or_else(Vec::new, |active| active.command.preview(doc, &live))
    }

    /// The surfaces and solids the running command would make or change, for the viewport to show
    /// ghosted.
    #[must_use]
    pub fn preview_bodies(
        &self,
        doc: &Document,
        cursor: Option<Point>,
        typed: &str,
    ) -> Vec<curva_kernel::Body> {
        let live = self.live(doc, cursor, typed);
        self.active
            .as_ref()
            .map_or_else(Vec::new, |active| active.command.preview_bodies(doc, &live))
    }

    /// The objects the running command's preview changes in place, which the viewport hides
    /// while the changed ones show, as FilletEdge's rounded solid replaces the original.
    #[must_use]
    pub fn preview_replaces(&self, doc: &Document, cursor: Option<Point>, typed: &str) -> Vec<u64> {
        let live = self.live(doc, cursor, typed);
        self.active.as_ref().map_or_else(Vec::new, |active| {
            active.command.preview_replaces(doc, &live)
        })
    }

    /// What a preview follows: the pointer, with any typed distance or angle held, and the typed
    /// number, or the size along the step's direction at the pointer.
    fn live(&self, doc: &Document, cursor: Option<Point>, typed: &str) -> Live {
        let cursor = cursor.map(|at| self.constrained(doc, at));
        let typed_number = parse_length(typed, doc.state().unit);
        // A size measured along a direction follows the pointer until a number is typed.
        let along = cursor.and_then(|at| self.request()?.along_value(at));
        Live {
            cursor,
            number: typed_number.or(along),
        }
    }

    /// The distance or angle typed for the next point, if any.
    #[must_use]
    pub fn lock(&self) -> Lock {
        self.lock
    }

    /// Where the pointer at `cursor` lands for the current step, with any typed distance or angle
    /// held (Rhino's typed constraints).
    #[must_use]
    pub fn constrained(&self, doc: &Document, cursor: Point) -> Point {
        match self.request().and_then(Request::base) {
            Some(base) if self.lock.is_set() && self.takes_point() => {
                self.lock.apply(base, cursor, doc.work_plane())
            }
            _ => cursor,
        }
    }

    /// What the current step measures at the pointer, for the tag beside it: a size along its
    /// direction, or the distance from its base point.
    #[must_use]
    pub fn measure(&self, doc: &Document, cursor: Point) -> Option<f64> {
        let request = self.request()?;
        if let Some(value) = request.along_value(cursor) {
            return Some(value);
        }
        let takes_distance = self.takes_point() || request.kind == Kind::Number;
        let base = request.base().filter(|_| takes_distance)?;
        let at = self.constrained(doc, cursor);
        Some(curva_kernel::gap(at, base))
    }

    /// For a step picking a rectangle's far corner: its width and depth on the construction plane
    /// with the pointer there.
    #[must_use]
    pub fn corner_size(&self, doc: &Document, cursor: Point) -> Option<(f64, f64)> {
        let request = self.request().filter(|r| r.corner)?;
        let base = request.from?;
        let at = self.constrained(doc, cursor);
        let [x, y] = doc.work_plane();
        let across: Point = std::array::from_fn(|i| at[i] - base[i]);
        let along = |axis: Point| (0..3).map(|i| across[i] * axis[i]).sum::<f64>().abs();
        Some((along(x), along(y)))
    }

    /// Whether the current step asks for a position that typed constraints apply to.
    fn takes_point(&self) -> bool {
        self.request().is_some_and(|r| r.kind == Kind::Point)
    }

    /// Adds a line to the log, for reports from outside the command line.
    pub fn note(&mut self, line: String) {
        self.log.push(line);
    }

    /// The oldest change of view a command asked for that is still waiting to be carried out.
    pub fn take_view_request(&mut self) -> Option<ViewRequest> {
        self.view_requests.pop_front()
    }

    /// Commands the typed text could be, best first.
    #[must_use]
    pub fn completions(&self, prefix: &str) -> Vec<&'static str> {
        completions(prefix)
    }

    /// What Enter or Space should submit for the typed text and highlighted completion.
    #[must_use]
    pub fn autofilled(&self, typed: &str, highlighted: usize) -> String {
        if self.is_running() {
            typed.to_owned()
        } else {
            autofilled(typed, highlighted)
        }
    }

    fn say(&mut self, message: &Message) {
        let line = self.text.render(message);
        self.log.push(line);
    }

    /// Handles a line from the command line, including Enter on an empty line.
    pub fn submit(&mut self, doc: &mut Document, typed: &str) {
        let typed = typed.trim();
        if self.active.is_some() {
            self.answer_typed(doc, typed);
        } else if typed.is_empty() {
            if let Some(def) = self.last {
                self.start(doc, def);
            }
        } else if let Some(def) = resolve(typed) {
            self.start(doc, def);
        } else if let Some(def) = typed.strip_prefix('-').and_then(resolve) {
            // As in Rhino, a dash asks for file locations on the command line.
            self.start(doc, def);
            if let Some(active) = self.active.as_mut() {
                active.typed = true;
                if let Some(request) = active.request.as_mut() {
                    request.file = None;
                }
            }
        } else {
            self.say(&Message::with(
                "error.unknown_command",
                vec![typed.to_owned()],
            ));
        }
    }

    fn start(&mut self, doc: &mut Document, def: &'static CommandDef) {
        if def.repeatable {
            self.last = Some(def);
        }
        self.active = Some(Active {
            def,
            command: (def.make)(),
            request: None,
            before: doc.state().clone(),
            selection_before: doc.selection(),
            typed: false,
        });
        self.advance(doc, None);
    }

    fn answer_typed(&mut self, doc: &mut Document, typed: &str) {
        let Some(request) = self.request().cloned() else {
            return;
        };
        if typed.is_empty() {
            let selection = doc.selection();
            let edges = doc.edge_selection();
            if request.kind == Kind::Objects && !selection.is_empty() {
                self.advance(doc, Some(Answer::Objects(selection)));
            } else if request.kind == Kind::Edges && !edges.is_empty() {
                doc.select_edges(&[]);
                self.advance(doc, Some(Answer::Edges(edges)));
            } else if request.kind == Kind::Faces && !doc.face_selection().is_empty() {
                let faces = doc.face_selection();
                doc.select_faces(&[]);
                self.advance(doc, Some(Answer::Faces(faces)));
            } else if request.allow_empty {
                self.advance(doc, Some(Answer::Empty));
            } else {
                self.say(&Message::new("error.needs_input"));
            }
            return;
        }
        if let Some(option) = request
            .options
            .iter()
            .find(|option| option.eq_ignore_ascii_case(typed))
        {
            self.advance(doc, Some(Answer::Option(option)));
            return;
        }
        // A point measured from a base takes a typed distance, angle or both, as in Rhino.
        if request.kind == Kind::Point
            && let Some(base) = request.base()
            && let Some(typed_lock) = parse_lock(typed, doc.state().unit)
        {
            match typed_lock {
                TypedLock::Distance(distance) => {
                    self.lock.distance = Some(distance);
                    self.say(&Message::with(
                        "note.lock.distance",
                        vec![distance.to_string()],
                    ));
                }
                TypedLock::Angle(angle) => {
                    self.lock.angle = Some(angle);
                    self.say(&Message::with("note.lock.angle", vec![angle.to_string()]));
                }
                TypedLock::Both(distance, angle) => {
                    let both = Lock {
                        distance: Some(distance),
                        angle: Some(angle),
                    };
                    let at = both.apply(base, base, doc.work_plane());
                    self.advance(doc, Some(Answer::Point(at)));
                }
            }
            return;
        }
        let answer = match request.kind {
            Kind::Point => parse_point(typed, self.last_point, doc.work_frame(), doc.state().unit)
                .map(Answer::Point)
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::PointOrNumber => parse_length(typed, doc.state().unit)
                .map(Answer::Number)
                .or_else(|| {
                    parse_point(typed, self.last_point, doc.work_frame(), doc.state().unit)
                        .map(Answer::Point)
                })
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::Number => parse_length(typed, doc.state().unit)
                .map(Answer::Number)
                .ok_or_else(|| Message::with("error.bad_number", vec![typed.to_owned()])),
            Kind::Path => Ok(Answer::Path(typed.to_owned())),
            Kind::Objects => Err(Message::new("error.pick_objects")),
            Kind::Edges => Err(Message::new("error.pick_edges")),
            Kind::Faces => Err(Message::new("error.pick_faces")),
            Kind::PickPart => parse_length(typed, doc.state().unit)
                .map(Answer::Number)
                .ok_or_else(|| Message::new("error.pick_part")),
            Kind::Choice => Err(Message::with(
                "error.bad_choice",
                vec![request.options.join(", ")],
            )),
        };
        // As in Rhino, the start of an option's name is enough when no other option shares it.
        let answer = answer.or_else(|message| {
            let lowered = typed.to_lowercase();
            let mut starting = request
                .options
                .iter()
                .filter(|option| option.to_lowercase().starts_with(&lowered));
            match (starting.next(), starting.next()) {
                (Some(option), None) => Ok(Answer::Option(option)),
                _ => Err(message),
            }
        });
        match answer {
            Ok(answer) => self.advance(doc, Some(answer)),
            Err(message) => self.say(&message),
        }
    }

    /// Handles a click at a model position. Ignored when the current step takes no position.
    pub fn click(&mut self, doc: &mut Document, at: Point) {
        let Some(request) = self.request().cloned() else {
            return;
        };
        if request.kind == Kind::Number
            && let Some(value) = request.along_value(at)
        {
            self.advance(doc, Some(Answer::Number(value)));
            return;
        }
        match (request.kind, request.from) {
            (Kind::Point, _) => {
                let at = self.constrained(doc, at);
                self.advance(doc, Some(Answer::Point(at)));
            }
            (Kind::PointOrNumber | Kind::PickPart, _) => {
                self.advance(doc, Some(Answer::Point(at)));
            }
            (Kind::Number, Some(from)) => {
                let distance = (0..3)
                    .map(|axis| (at[axis] - from[axis]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                self.advance(doc, Some(Answer::Number(distance)));
            }
            _ => {}
        }
    }

    /// Lets requests for files carry a file dialog for the interface to show
    /// ([`crate::Request::file`]); scripts and agents leave this off and type locations.
    pub fn use_file_dialogs(&mut self, on: bool) {
        self.dialogs = on;
    }

    /// Stops the running command and discards what it changed.
    pub fn cancel(&mut self, doc: &mut Document) {
        if let Some(active) = self.active.take() {
            if active.def.records_undo {
                if active.command.keeps_on_cancel() && doc.state() != &active.before {
                    // What it placed stays, as one undo step.
                    doc.record_change_with(active.before, &active.selection_before);
                } else {
                    doc.restore(active.before);
                }
            }
            self.say(&Message::new("note.cancelled"));
        }
    }

    fn advance(&mut self, doc: &mut Document, answer: Option<Answer>) {
        if let Some(Answer::Point(point)) = &answer {
            self.last_point = Some(*point);
        }
        // A typed distance or angle holds for one point only.
        if !matches!(answer, Some(Answer::Option(_))) {
            self.lock = Lock::default();
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        match active.command.step(doc, answer) {
            Step::Ask(mut request) => {
                if !self.dialogs || active.typed {
                    request.file = None;
                }
                active.request = Some(request);
            }
            Step::Done(message) => {
                doc.refresh_history();
                if let Some(active) = self.active.take()
                    && active.def.records_undo
                    && doc.state() != &active.before
                {
                    doc.record_change_with(active.before, &active.selection_before);
                }
                if let Some(message) = message {
                    self.say(&message);
                }
            }
            Step::View(request) => {
                self.active = None;
                self.view_requests.push_back(request);
            }
            Step::Failed(message) => {
                if let Some(active) = self.active.take()
                    && active.def.records_undo
                {
                    doc.restore(active.before);
                }
                self.say(&message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: Frame = ([0.0; 3], [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);

    const FRONT: Frame = ([0.0; 3], [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
    const RAISED: Frame = ([5.0, 5.0, 5.0], [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);

    #[test]
    fn file_commands_offer_a_dialog_only_where_the_interface_shows_them() {
        let mut doc = Document::default();
        let mut scripted = Processor::default();
        scripted.submit(&mut doc, "Open");
        assert_eq!(
            scripted.current_request().map(|r| r.file.is_some()),
            Some(false)
        );
        let mut app = Processor::default();
        app.use_file_dialogs(true);
        app.submit(&mut doc, "Open");
        let dialog = app.current_request().and_then(|r| r.file.clone()).unwrap();
        assert!(!dialog.save);
        assert_eq!(dialog.formats[0], crate::CURVA_FILES);
        app.cancel(&mut doc);
        app.submit(&mut doc, "SaveAs");
        let dialog = app.current_request().and_then(|r| r.file.clone()).unwrap();
        assert!(dialog.save);
        assert_eq!(dialog.name, None);
    }

    #[test]
    fn a_dash_asks_for_the_location_on_the_command_line() {
        let folder = std::env::temp_dir().join(format!("curva-dash-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("dashed.curva");
        let mut doc = Document::default();
        let mut app = Processor::default();
        app.use_file_dialogs(true);
        app.submit(&mut doc, "-SaveAs");
        assert_eq!(app.current_request().map(|r| r.file.is_some()), Some(false));
        app.submit(&mut doc, &path.display().to_string());
        assert!(app.current_request().is_none());
        assert_eq!(doc.path(), Some(path.as_path()));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    fn near(a: Point, b: Point) -> bool {
        curva_kernel::gap(a, b) < 1e-9
    }

    fn line_end(doc: &Document) -> Point {
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        points[points.len() - 1]
    }

    #[test]
    fn typed_locks_are_read() {
        assert_eq!(
            parse_lock("25", Unit::Millimetres),
            Some(TypedLock::Distance(25.0))
        );
        assert_eq!(
            parse_lock("<30", Unit::Millimetres),
            Some(TypedLock::Angle(30.0))
        );
        assert_eq!(
            parse_lock("25<30", Unit::Millimetres),
            Some(TypedLock::Both(25.0, 30.0))
        );
        assert_eq!(parse_lock("-5", Unit::Millimetres), None);
        assert_eq!(parse_lock("3,4", Unit::Millimetres), None);
    }

    #[test]
    fn a_typed_distance_holds_while_the_mouse_gives_the_direction() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["line", "1,1", "25"] {
            processor.submit(&mut doc, line);
        }
        assert!(processor.lock().is_set());
        // The pointer is 3, 4 away; the end is 25 away in that direction.
        processor.click(&mut doc, [4.0, 5.0, 0.0]);
        assert!(
            near(line_end(&doc), [16.0, 21.0, 0.0]),
            "{:?}",
            line_end(&doc)
        );
    }

    #[test]
    fn a_typed_angle_holds_while_the_mouse_gives_the_distance() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["line", "0,0", "<90"] {
            processor.submit(&mut doc, line);
        }
        processor.click(&mut doc, [3.0, 7.0, 0.0]);
        assert!(
            near(line_end(&doc), [0.0, 7.0, 0.0]),
            "{:?}",
            line_end(&doc)
        );
    }

    #[test]
    fn a_typed_distance_and_angle_place_the_point_at_once() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["line", "0,0", "10<90"] {
            processor.submit(&mut doc, line);
        }
        assert!(
            near(line_end(&doc), [0.0, 10.0, 0.0]),
            "{:?}",
            line_end(&doc)
        );
    }

    #[test]
    fn solid_previews_come_as_bodies_to_ghost() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["box", "0,0", "10,10"] {
            processor.submit(&mut doc, line);
        }
        // Choosing the base: no solid yet. A height: one solid, exactly the box Enter would make.
        assert!(processor.preview_bodies(&doc, None, "").is_empty());
        let bodies = processor.preview_bodies(&doc, None, "5");
        assert_eq!(bodies.len(), 1);
        assert!((bodies[0].volume().unwrap() - 500.0).abs() < 1e-6);
        // A fillet's preview is the rounded solid.
        let mut doc = Document::default();
        let id = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [10.0; 3]).unwrap());
        doc.select_edges(&[(id, 0)]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "filletedge");
        processor.submit(&mut doc, "");
        let rounded = processor.preview_bodies(&doc, None, "2");
        assert_eq!(rounded.len(), 1);
        assert!(rounded[0].volume().unwrap() < 1000.0);
        assert!(
            doc.state().objects[0]
                .shape
                .body()
                .unwrap()
                .volume()
                .unwrap()
                > 999.0
        );
    }

    #[test]
    fn a_height_follows_a_point_off_the_plane() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["box", "0,0", "10,10"] {
            processor.submit(&mut doc, line);
        }
        // A point snapped 7 above the plane, off to the side, gives a height of 7.
        let above = [30.0, -4.0, 7.0];
        assert_eq!(processor.measure(&doc, above), Some(7.0));
        processor.click(&mut doc, above);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 700.0).abs() < 1e-6, "{volume}");
    }

    #[test]
    fn typed_points_follow_the_plane() {
        assert_eq!(
            parse_point("3,2", None, FRONT, Unit::Millimetres),
            Some([3.0, 0.0, 2.0])
        );
        assert_eq!(
            parse_point("1,1", None, RAISED, Unit::Millimetres),
            Some([6.0, 6.0, 5.0])
        );
        assert_eq!(
            parse_point("0,0,2", None, RAISED, Unit::Millimetres),
            Some([5.0, 5.0, 7.0])
        );
    }

    #[test]
    fn relative_points_follow_the_plane_from_the_last_point() {
        let last = Some([1.0, 1.0, 1.0]);
        assert_eq!(
            parse_point("r3,2", last, FRONT, Unit::Millimetres),
            Some([4.0, 1.0, 3.0])
        );
    }

    #[test]
    fn w_means_world_coordinates_whatever_the_plane() {
        assert_eq!(
            parse_point("w3,2", None, RAISED, Unit::Millimetres),
            Some([3.0, 2.0, 0.0])
        );
        assert_eq!(
            parse_point("w1,2,3", None, FRONT, Unit::Millimetres),
            Some([1.0, 2.0, 3.0])
        );
    }

    #[test]
    fn a_typed_point_uses_the_plane_being_drawn_on() {
        let mut doc = Document::default();
        doc.set_work_origin([0.0, 0.0, 10.0]);
        let mut processor = Processor::default();
        for typed in ["line", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert_eq!(points[0], [0.0, 0.0, 10.0]);
    }
    #[test]
    fn points_read_like_rhino() {
        assert_eq!(
            parse_point("1,2", None, WORLD, Unit::Millimetres),
            Some([1.0, 2.0, 0.0])
        );
        assert_eq!(
            parse_point("1,2,3", None, WORLD, Unit::Millimetres),
            Some([1.0, 2.0, 3.0])
        );
        let last = Some([10.0, 10.0, 10.0]);
        assert_eq!(
            parse_point("r1,2", last, WORLD, Unit::Millimetres),
            Some([11.0, 12.0, 10.0])
        );
        assert_eq!(
            parse_point("@1,2,3", last, WORLD, Unit::Millimetres),
            Some([11.0, 12.0, 13.0])
        );
        assert_eq!(parse_point("abc", None, WORLD, Unit::Millimetres), None);
        assert_eq!(parse_point("1,2,3,4", None, WORLD, Unit::Millimetres), None);
    }

    #[test]
    fn enter_on_an_empty_line_repeats_the_last_command() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for typed in ["line", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        for typed in ["", "0,0", "0,1"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn the_start_of_an_option_is_enough_when_only_one_option_has_it() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for line in ["polyline", "0,0", "5,0", "5,5", "c"] {
            processor.submit(&mut doc, line);
        }
        assert!(!processor.is_running());
        assert!(matches!(
            doc.state().objects[0].curve().unwrap(),
            curva_kernel::Curve::Polyline { closed: true, .. }
        ));
        let id = doc.state().objects[0].id;
        doc.select(&[id]);
        for line in ["setpt", "SetX"] {
            processor.submit(&mut doc, line);
        }
        let before = processor.log().len();
        // `set` starts all three options, so it is not taken as any of them.
        processor.submit(&mut doc, "set");
        assert!(processor.is_running());
        assert!(processor.log().len() > before);
    }

    #[test]
    fn an_unknown_command_is_reported() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        processor.submit(&mut doc, "nonsense");
        assert!(processor.log()[0].contains("nonsense"));
        assert!(!processor.is_running());
    }

    #[test]
    fn a_bad_answer_asks_again() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        processor.submit(&mut doc, "line");
        processor.submit(&mut doc, "not a point");
        assert!(processor.is_running());
        processor.submit(&mut doc, "0,0");
        processor.submit(&mut doc, "1,0");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn clicks_answer_point_steps_and_measure_distances() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        processor.submit(&mut doc, "circle");
        processor.click(&mut doc, [1.0, 1.0, 0.0]);
        processor.click(&mut doc, [4.0, 5.0, 0.0]);
        let radius = match doc.state().objects[0].curve().unwrap() {
            curva_kernel::Curve::Circle { radius, .. } => *radius,
            _ => 0.0,
        };
        assert!((radius - 5.0).abs() < 1e-12);
    }

    #[test]
    fn cancel_discards_the_commands_changes() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for typed in ["line", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        let before = doc.state().clone();
        // A command that has placed nothing leaves the model as it was.
        for typed in ["line", "3,3"] {
            processor.submit(&mut doc, typed);
        }
        processor.cancel(&mut doc);
        assert_eq!(doc.state(), &before);
    }

    #[test]
    fn cancel_keeps_the_copies_already_placed_as_rhino_does() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for typed in ["line", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[0]);
        for typed in ["copy", "0,0", "5,0"] {
            processor.submit(&mut doc, typed);
        }
        processor.cancel(&mut doc);
        assert_eq!(doc.state().objects.len(), 2);
        // One undo takes the copy away.
        assert!(doc.undo());
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn a_whole_command_is_one_undo_step() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for typed in ["line", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        let before = doc.state().clone();
        doc.select(&[0]);
        for typed in ["copy", "0,0", "5,0", "9,0", ""] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(doc.state().objects.len(), 3);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state(), &before);
    }
}
