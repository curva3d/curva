//! Runs commands: turns typed text and clicks into answers, and makes each command one undo step.

use crate::commands::{CommandDef, autofilled, completions, resolve};
use crate::{Answer, Command, Kind, Live, Message, Request, Step, Translations, ViewRequest};
use curva_document::{Document, State};
use curva_kernel::{Curve, Point, cross};

struct Active {
    def: &'static CommandDef,
    command: Box<dyn Command>,
    request: Option<Request>,
    before: State,
}

/// The command line's engine.
pub struct Processor {
    active: Option<Active>,
    last: Option<&'static CommandDef>,
    last_point: Option<Point>,
    log: Vec<String>,
    text: Translations,
    view_requests: std::collections::VecDeque<ViewRequest>,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            active: None,
            last: None,
            last_point: None,
            log: Vec::new(),
            text: Translations::english(),
            view_requests: std::collections::VecDeque::new(),
        }
    }
}

/// Where typed coordinates are measured: the origin and the X and Y directions of a plane.
type Frame = (Point, [Point; 2]);

/// Reads `x,y` or `x,y,z` on the plane, as in Rhino. `r` or `@` in front means relative to the
/// last point, and `w` means world coordinates whatever the plane.
fn parse_point(text: &str, last: Option<Point>, (origin, [x, y]): Frame) -> Option<Point> {
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
        .map(|part| part.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
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
        if !request.options.is_empty() {
            shown = format!("{shown} ({})", request.options.join(", "));
        }
        Some(shown)
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
        let live = Live {
            cursor,
            number: typed.trim().parse().ok().filter(|n: &f64| n.is_finite()),
        };
        self.active
            .as_ref()
            .map_or_else(Vec::new, |active| active.command.preview(doc, &live))
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
        let answer = match request.kind {
            Kind::Point => parse_point(typed, self.last_point, doc.work_frame())
                .map(Answer::Point)
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::PointOrNumber => typed
                .parse::<f64>()
                .map(Answer::Number)
                .ok()
                .or_else(|| {
                    parse_point(typed, self.last_point, doc.work_frame()).map(Answer::Point)
                })
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::Number => typed
                .parse::<f64>()
                .map(Answer::Number)
                .map_err(|_| Message::with("error.bad_number", vec![typed.to_owned()])),
            Kind::Path => Ok(Answer::Path(typed.to_owned())),
            Kind::Objects => Err(Message::new("error.pick_objects")),
            Kind::Edges => Err(Message::new("error.pick_edges")),
            Kind::PickPart => typed
                .parse::<f64>()
                .map(Answer::Number)
                .map_err(|_| Message::new("error.pick_part")),
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
        match (request.kind, request.from) {
            (Kind::Point | Kind::PointOrNumber | Kind::PickPart, _) => {
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

    /// Stops the running command and discards what it changed.
    pub fn cancel(&mut self, doc: &mut Document) {
        if let Some(active) = self.active.take() {
            if active.def.records_undo {
                doc.restore(active.before);
            }
            self.say(&Message::new("note.cancelled"));
        }
    }

    fn advance(&mut self, doc: &mut Document, answer: Option<Answer>) {
        if let Some(Answer::Point(point)) = &answer {
            self.last_point = Some(*point);
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        match active.command.step(doc, answer) {
            Step::Ask(request) => active.request = Some(request),
            Step::Done(message) => {
                doc.refresh_history();
                if let Some(active) = self.active.take()
                    && active.def.records_undo
                    && doc.state() != &active.before
                {
                    doc.record_change(active.before);
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
    fn typed_points_follow_the_plane() {
        assert_eq!(parse_point("3,2", None, FRONT), Some([3.0, 0.0, 2.0]));
        assert_eq!(parse_point("1,1", None, RAISED), Some([6.0, 6.0, 5.0]));
        assert_eq!(parse_point("0,0,2", None, RAISED), Some([5.0, 5.0, 7.0]));
    }

    #[test]
    fn relative_points_follow_the_plane_from_the_last_point() {
        let last = Some([1.0, 1.0, 1.0]);
        assert_eq!(parse_point("r3,2", last, FRONT), Some([4.0, 1.0, 3.0]));
    }

    #[test]
    fn w_means_world_coordinates_whatever_the_plane() {
        assert_eq!(parse_point("w3,2", None, RAISED), Some([3.0, 2.0, 0.0]));
        assert_eq!(parse_point("w1,2,3", None, FRONT), Some([1.0, 2.0, 3.0]));
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
        assert_eq!(parse_point("1,2", None, WORLD), Some([1.0, 2.0, 0.0]));
        assert_eq!(parse_point("1,2,3", None, WORLD), Some([1.0, 2.0, 3.0]));
        let last = Some([10.0, 10.0, 10.0]);
        assert_eq!(parse_point("r1,2", last, WORLD), Some([11.0, 12.0, 10.0]));
        assert_eq!(parse_point("@1,2,3", last, WORLD), Some([11.0, 12.0, 13.0]));
        assert_eq!(parse_point("abc", None, WORLD), None);
        assert_eq!(parse_point("1,2,3,4", None, WORLD), None);
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
        doc.select(&[0]);
        for typed in ["copy", "0,0", "5,0"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(doc.state().objects.len(), 2);
        processor.cancel(&mut doc);
        assert_eq!(doc.state(), &before);
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
