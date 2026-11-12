//! The command framework. A command is a small state machine that asks for one input at a time,
//! so the command line, viewport clicks and, later, scripts and agents all drive it the same way.

pub mod commands;
mod processor;
mod text;

use curva_document::Document;
use curva_kernel::{Curve, Point};

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
    /// A position, or a typed number instead.
    PointOrNumber,
    /// A clicked part of a curve; the answer is the exact point on it.
    PickPart,
    /// One of the request's option words, and nothing else.
    Choice,
    /// Edges of surfaces and solids, picked in the viewport.
    Edges,
    /// Faces of surfaces and solids, picked in the viewport.
    Faces,
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
    /// For a size measured along a direction, such as a height or an extrusion: where it starts and
    /// the direction, of length 1. The pointer, snapped, is projected onto that line, so the size
    /// follows the mouse and lines up with what it snaps to, as in Rhino.
    pub along: Option<(Point, Point)>,
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
            along: None,
        }
    }

    /// The point this step measures from: the last point placed, or where distances start.
    #[must_use]
    pub fn base(&self) -> Option<Point> {
        self.preview.last().copied().or(self.from)
    }

    /// The signed size of `point` along [`Self::along`], if this step measures one.
    #[must_use]
    pub fn along_value(&self, point: Point) -> Option<f64> {
        let (origin, direction) = self.along?;
        Some((0..3).map(|i| (point[i] - origin[i]) * direction[i]).sum())
    }
}

/// A distance or angle typed for the next point, held while the mouse supplies the rest, as Rhino's
/// typed constraints are: `25` holds the distance from the step's base, `<30` the angle on the
/// construction plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lock {
    /// The distance from the base point.
    pub distance: Option<f64>,
    /// The angle on the construction plane, in degrees from its X axis.
    pub angle: Option<f64>,
}

impl Lock {
    /// Whether anything is held.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.distance.is_some() || self.angle.is_some()
    }

    /// Where the pointer at `cursor` lands with this lock, measured from `base`, on the construction
    /// plane with axes `plane`. A held angle fixes the direction on the plane and the mouse gives the
    /// distance along it; a held distance keeps the mouse's direction at that distance.
    #[must_use]
    pub fn apply(&self, base: Point, cursor: Point, plane: [Point; 2]) -> Point {
        let [x, y] = plane;
        let offset: Point = std::array::from_fn(|i| cursor[i] - base[i]);
        let direction = match self.angle {
            Some(degrees) => {
                let (sin, cos) = degrees.to_radians().sin_cos();
                std::array::from_fn(|i| cos * x[i] + sin * y[i])
            }
            None => curva_kernel::unit(offset).unwrap_or(x),
        };
        let reach = match (self.distance, self.angle) {
            (Some(distance), _) => distance,
            (None, Some(_)) => (0..3).map(|i| offset[i] * direction[i]).sum(),
            (None, None) => return cursor,
        };
        std::array::from_fn(|i| base[i] + direction[i] * reach)
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
    /// Picked edges of surfaces and solids.
    Edges(Vec<curva_document::EdgePick>),
    /// Picked faces of surfaces and solids.
    Faces(Vec<curva_document::FacePick>),
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
    /// It finished by asking the interface to change a view.
    View(ViewRequest),
}

/// A named view a command can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewName {
    /// From above.
    Top,
    /// From the front.
    Front,
    /// From the right.
    Right,
    /// The orbiting perspective view.
    Perspective,
}

/// A change of view, which the interface carries out as the document knows nothing of views.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewRequest {
    /// Bring the last-used viewport closer by this factor; below 1 moves it away.
    ZoomBy(f64),
    /// Fit the box between two corners in the last-used viewport.
    ZoomWindow(Point, Point),
    /// Look at this point from the same distance in the last-used viewport.
    ZoomTarget(Point),
    /// Save the last-used viewport's camera under this name.
    SaveView(String),
    /// Show the view saved under this name in the last-used viewport.
    RestoreView(String),
    /// Fit the model in every viewport.
    ZoomExtents,
    /// Fit the selection in every viewport.
    ZoomSelected,
    /// Go back to four viewports in their standard arrangement.
    FourViews,
    /// Give the last-used viewport a construction plane of its own.
    SetCPlane {
        /// Where the plane starts.
        origin: Point,
        /// Its X direction, of length 1.
        x: Point,
        /// Its Y direction, of length 1.
        y: Point,
    },
    /// Put the last-used viewport back on its standard plane.
    ResetCPlane,
    /// Turn the last-used viewport into this view.
    Show(ViewName),
    /// Draw the last-used viewport this way.
    Display(DisplayMode),
    /// Show or hide the selection filter row in the status bar.
    ShowSelectionFilter,
    /// Turn the selection filter off or back on, keeping its ticks.
    ToggleSelectionFilter,
}

/// How a viewport draws the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    /// Lines only: curves, and the edges of surfaces and solids.
    Wireframe,
    /// Surfaces and solids filled in and lit, with their edges on top.
    Shaded,
    /// Surfaces and solids see-through at half strength, so edges behind them show.
    Ghosted,
}

/// What a preview can follow while a command waits for its next answer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Live {
    /// Where the next click would land, if the cursor is over a viewport.
    pub cursor: Option<Point>,
    /// The number typed so far on the command line, if it is one.
    pub number: Option<f64>,
}

/// A running command.
pub trait Command {
    /// Advances the command. The first call has no answer.
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step;

    /// The curves to show while waiting for the next answer: the result so far and any
    /// construction lines. It must not change anything.
    fn preview(&self, _doc: &Document, _live: &Live) -> Vec<Curve> {
        Vec::new()
    }
}
