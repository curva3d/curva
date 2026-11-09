//! The command framework. A command is a small state machine that asks for one input at a time,
//! so the command line, viewport clicks and, later, scripts and agents all drive it the same way.

pub mod commands;
mod processor;
mod text;

use curva_document::Document;
use curva_kernel::Point;

pub use processor::Processor;
pub use text::Translations;

/// Wording to show, by its key in the translation file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The key in the translation file.
    pub key: &'static str,
    /// Values for the placeholders.
    pub args: Vec<String>,
}

impl Message {
    /// A message without values.
    #[must_use]
    pub fn new(key: &'static str) -> Self {
        Self::with(key, Vec::new())
    }

    /// A message with values for its placeholders.
    #[must_use]
    pub fn with(key: &'static str, args: Vec<String>) -> Self {
        Self { key, args }
    }
}

/// What sort of answer a step wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A position.
    Point,
    /// A number; a click can give a distance from [`Request::from`].
    Number,
    /// Objects, chosen in the viewport.
    Objects,
    /// A file location.
    Path,
    /// One of the request's option words, and nothing else.
    Choice,
}

/// One question a command asks.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// What to show the user.
    pub prompt: Message,
    /// The sort of answer wanted.
    pub kind: Kind,
    /// Whether pressing Enter with nothing typed is an answer.
    pub allow_empty: bool,
    /// Words the user may type instead, such as `Close`.
    pub options: Vec<&'static str>,
    /// Points to draw a live preview line through, ending at the cursor.
    pub preview: Vec<Point>,
    /// Where a clicked distance is measured from.
    pub from: Option<Point>,
}

impl Request {
    /// A request with no options or preview.
    #[must_use]
    pub fn new(key: &'static str, kind: Kind) -> Self {
        Self {
            prompt: Message::new(key),
            kind,
            allow_empty: false,
            options: Vec::new(),
            preview: Vec::new(),
            from: None,
        }
    }
}

/// A reply to a [`Request`].
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// A position.
    Point(Point),
    /// A number.
    Number(f64),
    /// Chosen objects.
    Objects(Vec<u64>),
    /// A file location.
    Path(String),
    /// One of the request's option words.
    Option(&'static str),
    /// Enter with nothing typed.
    Empty,
}

/// What a command does next.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// It needs another input.
    Ask(Request),
    /// It finished, with an optional report.
    Done(Option<Message>),
    /// It could not continue; its changes are discarded.
    Failed(Message),
}

/// A running command.
pub trait Command {
    /// Advances the command. The first call has no answer.
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step;
}
