//! Runs commands: turns typed text and clicks into answers, and makes each command one undo step.

use crate::commands::{CommandDef, autofilled, completions, resolve};
use crate::{Answer, Command, Kind, Message, Request, Step, Translations, ViewRequest};
use curva_document::{Document, State};
use curva_kernel::Point;

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
    view_request: Option<ViewRequest>,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            active: None,
            last: None,
            last_point: None,
            log: Vec::new(),
            text: Translations::english(),
            view_request: None,
        }
    }
}

/// Reads `x,y`, `x,y,z`, or the same with `r` or `@` for relative to the last point, as in Rhino.
fn parse_point(text: &str, last: Option<Point>) -> Option<Point> {
    let lowered = text.trim().to_lowercase();
    let (relative, body) = match lowered.strip_prefix(['r', '@']) {
        Some(rest) => (true, rest),
        None => (false, lowered.as_str()),
    };
    let values: Vec<f64> = body
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()?;
    let offset = match values.as_slice() {
        [x, y] => [*x, *y, 0.0],
        [x, y, z] => [*x, *y, *z],
        _ => return None,
    };
    if relative {
        let base = last.unwrap_or([0.0; 3]);
        Some([
            base[0] + offset[0],
            base[1] + offset[1],
            base[2] + offset[2],
        ])
    } else {
        Some(offset)
    }
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

    /// The change of view a command asked for, if one is waiting to be carried out.
    pub fn take_view_request(&mut self) -> Option<ViewRequest> {
        self.view_request.take()
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
            if request.kind == Kind::Objects && !selection.is_empty() {
                self.advance(doc, Some(Answer::Objects(selection)));
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
            Kind::Point => parse_point(typed, self.last_point)
                .map(Answer::Point)
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::PointOrNumber => typed
                .parse::<f64>()
                .map(Answer::Number)
                .ok()
                .or_else(|| parse_point(typed, self.last_point).map(Answer::Point))
                .ok_or_else(|| Message::with("error.bad_point", vec![typed.to_owned()])),
            Kind::Number => typed
                .parse::<f64>()
                .map(Answer::Number)
                .map_err(|_| Message::with("error.bad_number", vec![typed.to_owned()])),
            Kind::Path => Ok(Answer::Path(typed.to_owned())),
            Kind::Objects => Err(Message::new("error.pick_objects")),
            Kind::Choice => Err(Message::with(
                "error.bad_choice",
                vec![request.options.join(", ")],
            )),
        };
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
            (Kind::Point | Kind::PointOrNumber, _) => {
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
                self.view_request = Some(request);
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

    #[test]
    fn points_read_like_rhino() {
        assert_eq!(parse_point("1,2", None), Some([1.0, 2.0, 0.0]));
        assert_eq!(parse_point("1,2,3", None), Some([1.0, 2.0, 3.0]));
        let last = Some([10.0, 10.0, 10.0]);
        assert_eq!(parse_point("r1,2", last), Some([11.0, 12.0, 10.0]));
        assert_eq!(parse_point("@1,2,3", last), Some([11.0, 12.0, 13.0]));
        assert_eq!(parse_point("abc", None), None);
        assert_eq!(parse_point("1,2,3,4", None), None);
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
        let radius = match doc.state().objects[0].curve {
            curva_kernel::Curve::Circle { radius, .. } => radius,
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
