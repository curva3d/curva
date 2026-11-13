//! The Curva application: a window with a viewport and a command line.
//!
//! `curva --picture out.png --run "line;0,0;50,20"` runs commands without a window and saves a
//! picture of the viewport, which is how automatic runs check the drawing.
//! `curva --mcp` runs as an MCP server for AI agents, on standard input and output, on a model of
//! its own. `curva --mcp --connect` connects an agent to the window that is open instead.
//! `curva --orbit-benchmark --curves 5000` opens the window on a scene of that many circles, spins
//! the view and prints how fast it drew and snapped. `--solids 500` adds that many cylinders.
//!
//! On Windows Curva is a GUI program, so no console opens beside it. Output from the command-line
//! modes reaches a terminal when it is piped (`curva --picture out.png --run line | more`), and the
//! MCP modes talk through the pipes the agent opens. An error that stops Curva starting is also kept
//! in `curva.log` in its settings folder.

// No console window beside Curva on Windows (spec `windows-no-console.md`).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod assistant;
mod icons;
mod layout;
mod recovery;
mod theme;
mod updates;

use curva_commands::{DisplayMode, Kind, Message, Processor, Translations, ViewName, ViewRequest};
use curva_document::{Document, Grip, Shape, State};
use curva_kernel::{Curve, Point};
use curva_mcp::Session;
use curva_viewport::{
    CPlane, Camera, DEPTH_BITS, Drawing, Gumball, GumballChange, Handle, LineRenderer, MeshCache,
    MeshVertex, Outlines, PickFilter, Picture, SnapHit, SnapKind, SnapSet, StandardView,
    VIEW_SLOTS, constrained, face_vertices, ghost_faces, ghost_lines, grid_lines_in, object_lines,
    save_picture, scene_lines, shaded_object_lines, shaded_scene_lines,
};
use eframe::egui;
use std::sync::Arc;

/// How close to a curve a click must be, in pixels.
const PICK_REACH: f64 = 6.0;
/// How far, in points, a left press must travel to draw a selection box and not be a click.
const BOX_MIN_DRAG: f32 = 6.0;
const BOX_EDGE: egui::Color32 = egui::Color32::from_rgb(230, 230, 235);
const BOX_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 40, 60, 40);
const SNAP_MARKER_SIZE: f32 = 10.0;
/// How long the pointer rests on a snap point to hold it, or let it go, in seconds.
const HOLD_AFTER: f64 = 0.5;
/// Spacing and size of the circles in a benchmark scene, in millimetres.
const BENCHMARK_SPACING: f64 = 12.0;
const BENCHMARK_RADIUS: f64 = 5.0;
/// The Perspective viewport's place in the list of viewports.
const PERSPECTIVE: usize = 1;
/// The band at the top of a viewport that holds its title, in points.
const TITLE_HEIGHT: f32 = 22.0;
const PANE_GAP: f32 = 2.0;
/// The part of a viewport's corner that opens its display menu when clicked.
const TITLE_SIZE: egui::Vec2 = egui::vec2(120.0, 22.0);
/// Which viewport fills the window when Curva opens.
const STARTING_LAYOUT: Option<usize> = Some(PERSPECTIVE);
/// Most objects shown moving during a gumball drag.
const MOST_GUMBALL_PREVIEW: usize = 2000;
const GUMBALL_TIP: f32 = 5.0;
const GUMBALL_AXES: [egui::Color32; 3] = [
    egui::Color32::from_rgb(230, 80, 80),
    egui::Color32::from_rgb(90, 200, 100),
    egui::Color32::from_rgb(90, 130, 240),
];
const POINT_MARKER_SIZE: f32 = 6.0;
/// Size of a control point's square on screen.
const GRIP_SIZE: f32 = 7.0;
/// A control point that is not picked.
const GRIP_COLOUR: egui::Color32 = egui::Color32::from_rgb(235, 235, 245);
/// A picked control point, in the accent like selected objects.
const GRIP_PICKED: egui::Color32 = theme::ACCENT;
/// The lines between a smooth curve's control points.
const CONTROL_POLYGON: egui::Color32 = egui::Color32::from_rgb(150, 150, 165);
const LOG_LINES: usize = 3;
/// Rows the list of matching commands shows before it scrolls.
const MOST_MATCH_ROWS: f32 = 10.0;
const PANEL_RADIUS: u8 = 12;
const PANEL_MARGIN: f32 = 8.0;
/// The height of a panel's title row, holding its name or tabs.
const TITLE_ROW: f32 = 20.0;
/// The size of panel titles, the same on every panel.
const TITLE_FONT: f32 = 13.0;
/// How near a panel's edge, in points, the pointer turns into a resize arrow.
const RESIZE_REACH: f32 = 6.0;
/// The size of a command button with an icon, in points.
const COMMAND_BUTTON: f32 = 30.0;
/// Room left around a panel for its shadow, beyond which nothing it holds is drawn.
const SHADOW_ROOM: f32 = 20.0;
/// How long a panel takes to slide in or out, in seconds.
const SLIDE_TIME: f32 = 0.2;
/// Where a dragged panel would land.
const DOCK_PREVIEW_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(56, 30, 12, 56);
/// How long after an agent's last request it still counts as connected.
const AGENT_RECENT: std::time::Duration = std::time::Duration::from_secs(30);
const AGENT_ACTIVE: egui::Color32 = egui::Color32::from_rgb(110, 200, 120);
/// The command tabs at the top, each with its most used commands (spec
/// `docs/specs/ui-floating-panels.md`).
const COMMAND_TABS: [(&str, &[&str]); 6] = [
    (
        "ui.tab.curves",
        &[
            "Line",
            "Polyline",
            "Curve",
            "InterpCrv",
            "Circle",
            "Arc",
            "Ellipse",
            "Rectangle",
            "Polygon",
            "Slot",
            "Helix",
            "Spiral",
            "Parabola",
            "Conic",
            "Hyperbola",
            "HandleCurve",
            "Sketch",
            "Point",
            "Points",
            "Lines",
            "Offset",
            "Fillet",
            "Chamfer",
            "BlendCrv",
            "OffsetMultiple",
            "Extend",
            "Match",
            "Fair",
            "SimplifyCrv",
            "CurveBoolean",
            "MakeUniform",
            "Reparameterize",
            "Weight",
        ],
    ),
    (
        "ui.tab.surfaces",
        &[
            "PlanarSrf",
            "Plane",
            "SrfPt",
            "ExtrudeCrv",
            "Revolve",
            "Loft",
            "Sweep1",
            "OffsetSrf",
            "ExtrudeSrf",
            "ExtrudeCrvTapered",
            "ExtrudeCrvAlongCrv",
            "ExtrudeCrvToPoint",
            "RailRevolve",
            "DupEdge",
            "DupBorder",
            "ExtractSrf",
            "EdgeSrf",
            "NetworkSrf",
            "Patch",
            "ExtendSrf",
            "Untrim",
            "RebuildUV",
            "Project",
            "Pull",
            "Sweep2",
            "BlendSrf",
            "MatchSrf",
            "SplitSrf",
            "ShrinkTrimmedSrf",
            "ExtractIsocurve",
            "InterpCrvOnSrf",
            "OffsetCrvOnSrf",
            "UnrollSrf",
            "FilletSrf",
            "ChamferSrf",
        ],
    ),
    (
        "ui.tab.solids",
        &[
            "Box",
            "Sphere",
            "Cylinder",
            "Cone",
            "TruncatedCone",
            "Torus",
            "Tube",
            "Ellipsoid",
            "Paraboloid",
            "Pyramid",
            "Pipe",
            "FilletEdge",
            "ChamferEdge",
            "Shell",
            "MergeAllFaces",
            "Wirecut",
            "MoveFace",
            "PushPull",
            "Cap",
            "ExtrudeFace",
            "Volume",
        ],
    ),
    (
        "ui.tab.operations",
        &[
            "BooleanUnion",
            "BooleanDifference",
            "BooleanIntersection",
            "BooleanSplit",
            "Trim",
            "Split",
            "Join",
            "Explode",
            "Move",
            "Copy",
            "Rotate",
            "Scale",
            "Mirror",
            "Array",
            "ArrayLinear",
            "Align",
            "Distribute",
            "Shear",
            "Twist",
            "Bend",
            "Taper",
            "Flow",
            "CageEdit",
            "Group",
            "Ungroup",
            "Lock",
            "Unlock",
        ],
    ),
    (
        "ui.tab.analyze",
        &[
            "Distance",
            "Length",
            "Area",
            "Volume",
            "Angle",
            "Radius",
            "EvaluatePt",
            "What",
            "BoundingBox",
            "AreaCentroid",
            "VolumeCentroid",
            "ShowEdges",
            "CurvatureGraph",
            "Section",
            "Contour",
            "Intersect",
            "Silhouette",
            "Plan",
        ],
    ),
    (
        "ui.tab.select",
        &[
            "SelAll",
            "SelNone",
            "Invert",
            "SelCrv",
            "SelLine",
            "SelPolyline",
            "SelCircle",
            "SelArc",
            "SelClosedCrv",
            "SelOpenCrv",
            "SelSrf",
            "SelSolid",
            "SelPt",
            "SelColor",
            "Hide",
            "Show",
            "HideSwap",
        ],
    ),
];
const PICTURE_SIZE: [u32; 2] = [1200, 800];
/// How far the pointer moves, in points, between the points Sketch takes while drawing.
const SKETCH_STEP: f32 = 4.0;
/// The specializations in the top bar's dial; only Freeform has commands yet.
const SPECIALIZATIONS: [&str; 4] = [
    "ui.specialization.freeform",
    "ui.specialization.architecture",
    "ui.specialization.publisher",
    "ui.specialization.analysis",
];
/// The width of the specializations dial in the top bar, in points.
const DIAL_WIDTH: f32 = 440.0;
/// The height of the top bar, in points.
const TOP_BAR_HEIGHT: f32 = 72.0;
/// How long the command bar's opening lines glow, in seconds.
const NOTICE_GLOW: f32 = 2.0;
/// Views kept for each viewport's `UndoView`.
const MOST_VIEWS_KEPT: usize = 50;
/// How fast Ctrl + right-drag zooms, per pixel dragged.
const ZOOM_DRAG_SPEED: f64 = 0.01;
const BENCHMARK_FRAMES: usize = 240;
/// Leading frames left out of the report, as they include the window opening.
const SETTLE_FRAMES: usize = 30;
/// How far, in points, the benchmark drags the view each frame.
const BENCHMARK_DRAG: f64 = 4.0;

struct Curva {
    doc: Document,
    processor: Processor,
    text: Translations,
    typed: String,
    highlight: usize,
    panes: [Pane; VIEW_SLOTS],
    active: usize,
    maximised: Option<usize>,
    gumball_on: bool,
    /// Whether a text field other than the command line has the keyboard, so keys go there.
    naming_layer: bool,
    /// The command line's text field, to tell it from other fields.
    command_field: Option<egui::Id>,
    agent_on: bool,
    agent: Option<curva_mcp::Listener>,
    waiting_approval: Option<(curva_mcp::AgentRequest, String)>,
    gumball_drag: Option<GumballDrag>,
    /// While Sketch draws with the button held: where the last point was fed, and how many.
    sketch_stroke: Option<(egui::Pos2, usize)>,
    /// A gumball handle clicked without a drag, and the value being typed for it.
    gumball_typing: Option<(Handle, String)>,
    /// Whether Alt is held this frame, which turns object snaps off for the moment.
    alt_held: bool,
    snaps: SnapSet,
    ortho: bool,
    grid_snap: bool,
    /// Which kinds of object clicking and boxing may pick.
    pick_filter: PickFilter,
    /// Whether the selection filter row shows in the status bar.
    filter_row: bool,
    outlines: Outlines,
    scene: Arc<Vec<curva_viewport::Vertex>>,
    /// The objects' lines for shaded views, where edges on faces are dark.
    shaded_scene: Arc<Vec<curva_viewport::Vertex>>,
    /// The faces of surfaces and solids, for shaded views.
    faces: Arc<Vec<MeshVertex>>,
    /// Meshes of solids, kept until they change.
    meshes: MeshCache,
    /// The last solid or surface measured for the properties panel: its id, the body as it
    /// was, and what it is, which measure, and the value.
    measured: Option<(u64, curva_kernel::Body, (&'static str, &'static str, f64))>,
    drawn: Option<(State, Vec<u64>)>,
    benchmark: Option<Benchmark>,
    /// Keeps a recovery copy of the model; off for benchmarks.
    autosave: Option<recovery::Autosave>,
    /// A recovery file left by a Curva that did not close normally, waiting for the user to
    /// restore or discard it.
    left_behind: Option<std::path::PathBuf>,
    /// Where the panels are.
    layout: layout::Layout,
    /// The user's settings: theme, glass or solid panels, grid.
    settings: theme::Settings,
    /// The Settings panel's tab showing.
    settings_tab: SettingsTab,
    /// A panel whose cross was clicked, while asking whether to close it.
    closing: Option<layout::PanelId>,
    /// The window's area this frame, for panels that open in its centre.
    window_rect: egui::Rect,
    /// A unit chosen in Settings, while asking whether to scale the model to it.
    unit_asked: Option<curva_document::Unit>,
    /// Whether the light theme was last put in use, once one has been.
    applied_light: Option<bool>,
    /// The command tab showing, as an index into [`COMMAND_TABS`].
    command_tab: usize,
    /// The specialization selected in the top bar's dial, as an index into [`SPECIALIZATIONS`].
    specialization: usize,
    /// When an agent last asked something, to show whether one is connected.
    agent_seen: Option<std::time::Instant>,
    /// Where the pointer last aimed in a viewport, for Enter to place a point with a held distance.
    last_aim: Option<Point>,
    /// The last aim, with its snap, held while the view is turned or slid.
    held_aim: Option<(Point, Option<SnapHit>)>,
    /// The faces with those a preview replaces left out, and which those are.
    faces_without: (Vec<u64>, Arc<Vec<MeshVertex>>),
    /// Objects copied with Ctrl+C or Ctrl+X, with their layers, for Ctrl+V.
    clipboard: Vec<(Shape, Option<u64>)>,
    /// The command bar's own lines: an update on offer, the welcome.
    notices: Vec<Notice>,
    /// The answer of the check for updates, while it is under way.
    update_check: Option<std::sync::mpsc::Receiver<Option<updates::Release>>>,
    /// A newer release on offer.
    update: Option<updates::Release>,
    /// The update being downloaded.
    update_download: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    /// The AI Assistant's conversation as shown.
    assistant_lines: Vec<assistant::Line>,
    /// The conversation so far, as (request, answer) turns, for the model.
    assistant_history: Vec<(String, String)>,
    /// What is being typed to the assistant.
    assistant_input: String,
    /// The answer being worked out.
    assistant_busy: Option<std::sync::mpsc::Receiver<(String, Vec<assistant::Line>)>>,
    /// The way of connecting chosen, before it connects.
    assistant_choice: Option<assistant::Provider>,
    /// An API key being typed.
    assistant_key: String,
    /// The models LM Studio offers.
    assistant_models: Vec<String>,
    /// Why connecting did not work.
    assistant_error: Option<String>,
    /// Snap points held by resting on them, which send out guidelines, as Rhino's SmartTrack.
    held: Vec<Point>,
    /// The snap point the pointer rests on, since when, and whether resting there has acted.
    resting: Option<(Point, f64, bool)>,
    /// The bodies of the last solid preview, and their ghosted triangles.
    ghosted: (Vec<curva_kernel::Body>, Arc<Vec<MeshVertex>>),
    /// Focus mode: the panels slid out of view.
    focus: bool,
    /// In focus mode, the one panel brought back out by clicking its strip.
    revealed: Option<layout::PanelId>,
    /// Where each panel was last drawn, for hovering over a hidden one.
    drawn_panels: Vec<(layout::PanelId, egui::Rect)>,
    /// A panel being moved by its grip.
    panel_drag: Option<PanelDrag>,
}

/// What happened to a panel this frame.
enum PanelEvent {
    Nothing,
    /// A hidden panel's strip was clicked, in focus mode.
    StripClicked,
    /// Its title began to be dragged, held this far from its corner: one tab's piece, or the
    /// whole panel.
    DragStarted(egui::Vec2, Option<layout::Piece>),
    /// These sides of it were dragged this far to resize it.
    Resized(layout::Sides, egui::Vec2),
    /// Reset layout was chosen from its grip's menu.
    Reset,
    /// Its hide control was clicked.
    Hide,
    /// One of its tabs was clicked.
    TabChosen(usize),
    /// A tab was dragged along the row from one place to another.
    TabMoved(usize, usize),
    /// Its cross was clicked: ask, then close it.
    Close,
}

/// A panel being moved by its grip.
#[derive(Clone, Copy)]
struct PanelDrag {
    panel: layout::PanelId,
    /// The piece being pulled out by its tab, if only one is.
    piece: Option<layout::Piece>,
    /// Where the grip was held, from the panel's corner.
    grab: egui::Vec2,
    /// Its size while it moves.
    size: egui::Vec2,
}

/// Finds left-button clicks and drags from the raw mouse events.
///
/// egui's own click signal drops a press if the mouse travels 6 points before release, and
/// reports at most one per frame, which loses points clicked quickly while moving to the next.
#[derive(Default)]
struct LeftGestures {
    pressed_at: Option<egui::Pos2>,
}

impl LeftGestures {
    /// Where each press finished in these events began and ended. A press only counts if it
    /// was `inside` the viewport.
    fn take(
        &mut self,
        events: &[egui::Event],
        inside: impl Fn(egui::Pos2) -> bool,
    ) -> Vec<(egui::Pos2, egui::Pos2)> {
        let mut gestures = Vec::new();
        for event in events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } = event
            {
                if *pressed {
                    self.pressed_at = inside(*pos).then_some(*pos);
                } else {
                    gestures.extend(self.pressed_at.take().map(|from| (from, *pos)));
                }
            }
        }
        gestures
    }
}

/// The selection after a click or box, as in Rhino: Ctrl removes what was hit, Shift adds it,
/// and with neither it replaces the selection. Ctrl wins if both are held, as it loses nothing.
fn combined_selection<T: PartialEq>(
    current: Vec<T>,
    hits: Vec<T>,
    add: bool,
    remove: bool,
) -> Vec<T> {
    if remove {
        return current
            .into_iter()
            .filter(|id| !hits.contains(id))
            .collect();
    }
    let mut chosen = if add { current } else { Vec::new() };
    for hit in hits {
        if !chosen.contains(&hit) {
            chosen.push(hit);
        }
    }
    chosen
}

/// The rows of the log to show, newest last. Always the same number, one line each, so that a
/// new message never changes the height of the panel and moves the viewport.
#[cfg(test)]
fn log_rows(log: &[String]) -> Vec<&str> {
    let (blanks, _, shown) = history_rows(log, 0);
    std::iter::repeat_n(" ", blanks)
        .chain(shown.iter().map(String::as_str))
        .collect()
}

/// How the command bar's rows are shared between its own lines (`notices` of them) and the
/// history: blank rows first, how many of its own lines to leave out (the oldest), and the history
/// lines shown. Together they fill [`LOG_LINES`] rows, the oldest going as new ones come.
fn history_rows(log: &[String], notices: usize) -> (usize, usize, &[String]) {
    let total = notices + log.len();
    let skip = total.saturating_sub(LOG_LINES);
    let blanks = LOG_LINES.saturating_sub(total);
    let log_skip = skip.saturating_sub(notices);
    (blanks, skip.min(notices), &log[log_skip.min(log.len())..])
}

/// Whether a press that moved from one place to another counts as a drag, not a click.
fn is_box_drag(from: egui::Pos2, to: egui::Pos2) -> bool {
    (to - from).abs().max_elem() >= BOX_MIN_DRAG
}

/// The small tag beside the pointer saying what a step measures there, as in Serpentine3D.
fn paint_cursor_tag(ui: &egui::Ui, pointer: egui::Pos2, text: &str) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::NULL.with("cursor tag"),
    ));
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::monospace(12.0),
        theme::current().quiet,
    );
    let corner = pointer + egui::vec2(18.0, 14.0);
    let frame = egui::Rect::from_min_size(corner, galley.size()).expand2(egui::vec2(6.0, 3.0));
    painter.rect(
        frame,
        4.0,
        theme::current().panel,
        egui::Stroke::new(1.0, theme::current().panel_edge),
        egui::StrokeKind::Inside,
    );
    painter.galley(corner, galley, theme::current().quiet);
}

/// The selection box: solid for a window, dashed for a crossing box, as in Rhino.
fn paint_selection_box(ui: &egui::Ui, viewport: egui::Rect, from: egui::Pos2, to: egui::Pos2) {
    let painter = ui.painter_at(viewport);
    let area = egui::Rect::from_two_pos(from, to);
    painter.rect_filled(area, 0.0, BOX_FILL);
    let stroke = egui::Stroke::new(1.0, BOX_EDGE);
    let edge = vec![
        area.left_top(),
        area.right_top(),
        area.right_bottom(),
        area.left_bottom(),
        area.left_top(),
    ];
    if to.x >= from.x {
        painter.add(egui::Shape::line(edge, stroke));
    } else {
        painter.add(egui::Shape::dashed_line(&edge, stroke, 6.0, 4.0));
    }
}

/// A drag on the gumball under way, in the pixels of its viewport.
#[derive(Clone, Copy)]
struct GumballDrag {
    pane: usize,
    handle: Handle,
    size: [f64; 2],
    from: [f64; 2],
    now: [f64; 2],
    /// Whether Shift is held, for an even scale.
    even: bool,
    /// Whether Ctrl is held: with Shift, the circle scales evenly.
    ctrl: bool,
}

/// One viewport: what it looks at and what it is tracking.
struct Pane {
    view: StandardView,
    plane: CPlane,
    camera: Camera,
    grid: Arc<Vec<curva_viewport::Vertex>>,
    gestures: LeftGestures,
    /// The saved view it shows, whose name becomes its title.
    named: Option<String>,
    /// How it draws surfaces and solids: by their edges, filled in and lit, or see-through.
    display: DisplayMode,
    /// Views to step back to with `UndoView`, newest last, and the view they were left from.
    views_back: Vec<Camera>,
    /// Views stepped back from, for `RedoView`, newest last.
    views_ahead: Vec<Camera>,
    /// The view as it last stood still, which a camera move leaves behind.
    settled: Camera,
}

impl Pane {
    fn set_plane(&mut self, plane: CPlane) {
        self.plane = plane;
        self.grid = Arc::new(grid_lines_in(&plane, &theme::current().viewport));
    }

    /// Keeps the view history: once the camera has moved and stands still again, the view it
    /// left goes on the list `UndoView` steps back through.
    fn track_view(&mut self, moving: bool) {
        if moving || self.camera == self.settled {
            return;
        }
        self.views_back.push(self.settled);
        if self.views_back.len() > MOST_VIEWS_KEPT {
            self.views_back.remove(0);
        }
        self.views_ahead.clear();
        self.settled = self.camera;
    }

    /// Steps back, or with `forward` forward, through the view history.
    fn step_view(&mut self, forward: bool) {
        let (from, to) = if forward {
            (&mut self.views_ahead, &mut self.views_back)
        } else {
            (&mut self.views_back, &mut self.views_ahead)
        };
        if let Some(view) = from.pop() {
            to.push(self.camera);
            self.camera = view;
            self.settled = view;
        }
    }

    fn new(view: StandardView) -> Self {
        Self {
            view,
            views_back: Vec::new(),
            views_ahead: Vec::new(),
            settled: view.camera(),
            camera: view.camera(),
            plane: view.plane(),
            grid: Arc::new(grid_lines_in(&view.plane(), &theme::current().viewport)),
            gestures: LeftGestures::default(),
            named: None,
            // As in Rhino, the perspective view starts shaded and the flat views in wireframe.
            display: if view == StandardView::Perspective {
                DisplayMode::Shaded
            } else {
                DisplayMode::Wireframe
            },
        }
    }
}

/// The four viewports, in the order of [`pane_rects`].
fn standard_panes() -> [Pane; VIEW_SLOTS] {
    [
        Pane::new(StandardView::Top),
        Pane::new(StandardView::Perspective),
        Pane::new(StandardView::Front),
        Pane::new(StandardView::Right),
    ]
}

fn standard_view(name: ViewName) -> StandardView {
    match name {
        ViewName::Top => StandardView::Top,
        ViewName::Front => StandardView::Front,
        ViewName::Right => StandardView::Right,
        ViewName::Perspective => StandardView::Perspective,
    }
}

fn view_label(view: StandardView) -> &'static str {
    match view {
        StandardView::Top => "ui.view.top",
        StandardView::Front => "ui.view.front",
        StandardView::Right => "ui.view.right",
        StandardView::Perspective => "ui.view.perspective",
    }
}

/// Where each viewport goes: Top and Perspective above Front and Right, as in Rhino, or just the
/// maximised one filling the whole area.
fn pane_rects(area: egui::Rect, maximised: Option<usize>) -> Vec<(usize, egui::Rect)> {
    if let Some(pane) = maximised {
        return vec![(pane, area)];
    }
    let half = PANE_GAP / 2.0;
    let centre = area.center();
    vec![
        (
            0,
            egui::Rect::from_min_max(area.min, centre - egui::Vec2::splat(half)),
        ),
        (
            1,
            egui::Rect::from_min_max(
                egui::pos2(centre.x + half, area.top()),
                egui::pos2(area.right(), centre.y - half),
            ),
        ),
        (
            2,
            egui::Rect::from_min_max(
                egui::pos2(area.left(), centre.y + half),
                egui::pos2(centre.x - half, area.bottom()),
            ),
        ),
        (
            3,
            egui::Rect::from_min_max(centre + egui::Vec2::splat(half), area.max),
        ),
    ]
}

/// A viewport stretched from `area` out to `reach` on each side where it touches the edge of
/// `area`, so it carries on under a glass panel there.
fn reaching(rect: egui::Rect, area: egui::Rect, reach: egui::Rect) -> egui::Rect {
    let touches = |a: f32, b: f32| (a - b).abs() < 1.0;
    let mut stretched = rect;
    if touches(rect.min.x, area.min.x) {
        stretched.min.x = reach.min.x;
    }
    if touches(rect.max.x, area.max.x) {
        stretched.max.x = reach.max.x;
    }
    if touches(rect.min.y, area.min.y) {
        stretched.min.y = reach.min.y;
    }
    if touches(rect.max.y, area.max.y) {
        stretched.max.y = reach.max.y;
    }
    stretched
}

/// A viewport's size for the camera, never zero even while the window is being squashed.
fn pane_size(rect: egui::Rect) -> [f64; 2] {
    [
        f64::from(rect.width()).max(1.0),
        f64::from(rect.height()).max(1.0),
    ]
}

/// Spins the view for a fixed number of frames and reports how fast the window drew them.
#[derive(Default)]
struct Benchmark {
    times: Vec<f64>,
    work: Vec<f32>,
    snapping: Vec<f64>,
}

/// A square grid of `count` circles.
/// A grid of circles, then a grid of upright cylinders beside it.
fn benchmark_scene(doc: &mut Document, scene: BenchmarkScene) -> Result<(), String> {
    let place = |index: usize, count: usize, shift: f64| {
        let side = (count as f64).sqrt().ceil().max(1.0) as usize;
        let (column, row) = ((index % side) as f64, (index / side) as f64);
        [
            column * BENCHMARK_SPACING + shift,
            row * BENCHMARK_SPACING,
            0.0,
        ]
    };
    for index in 0..scene.curves {
        let centre = place(index, scene.curves, 0.0);
        doc.add(Curve::circle(centre, BENCHMARK_RADIUS).map_err(|error| error.to_string())?);
    }
    let beside = (scene.curves as f64).sqrt().ceil() * BENCHMARK_SPACING + BENCHMARK_SPACING;
    for index in 0..scene.solids {
        let base = Curve::circle(place(index, scene.solids, beside), BENCHMARK_RADIUS)
            .map_err(|error| error.to_string())?;
        let up = curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], BENCHMARK_SPACING, true)
            .map_err(|error| error.to_string())?;
        doc.add_body(up);
    }
    Ok(())
}

/// What a benchmark scene holds.
#[derive(Clone, Copy, Debug, Default)]
struct BenchmarkScene {
    curves: usize,
    solids: usize,
}

fn curve_kind(curve: &Curve) -> &'static str {
    match curve {
        Curve::Point { .. } => "ui.kind.point",
        Curve::Line { .. } => "ui.kind.line",
        Curve::Polyline { .. } => "ui.kind.polyline",
        Curve::Circle { .. } => "ui.kind.circle",
        Curve::Arc { .. } => "ui.kind.arc",
        Curve::Interpolated { .. } | Curve::Nurbs { .. } => "ui.kind.smooth",
        Curve::Compound { .. } => "ui.kind.joined",
    }
}

fn snap_label(kind: SnapKind) -> &'static str {
    match kind {
        SnapKind::End => "ui.snap.end",
        SnapKind::Mid => "ui.snap.mid",
        SnapKind::Center => "ui.snap.center",
        SnapKind::Intersection => "ui.snap.intersection",
        SnapKind::Perpendicular => "ui.snap.perpendicular",
        SnapKind::Near => "ui.snap.near",
    }
}

impl Benchmark {
    /// Notes one frame. Returns whether enough have been seen.
    fn record(&mut self, ui: &egui::Ui, frame: &eframe::Frame) -> bool {
        self.times.push(ui.input(|input| input.time));
        self.work.extend(frame.info().cpu_usage);
        ui.ctx().request_repaint();
        self.times.len() >= BENCHMARK_FRAMES
    }

    fn report(&self, frame: &eframe::Frame) {
        if let Some(state) = frame.wgpu_render_state() {
            let info = state.adapter.get_info();
            println!(
                "adapter: {} ({:?}, {:?})",
                info.name, info.backend, info.device_type
            );
        }
        let mut gaps: Vec<f64> = self.times[SETTLE_FRAMES..]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) * 1000.0)
            .collect();
        gaps.sort_by(f64::total_cmp);
        let middle = gaps[gaps.len() / 2];
        let worst = gaps[gaps.len() * 99 / 100];
        let snapping = self.snapping.iter().sum::<f64>() / self.snapping.len().max(1) as f64;
        println!("snapping per frame: {snapping:.2} ms");
        let work = self.work.iter().sum::<f32>() * 1000.0 / self.work.len().max(1) as f32;
        println!("frame gap: median {middle:.1} ms, 99th percentile {worst:.1} ms");
        println!("work per frame: {work:.2} ms");
    }
}

impl Curva {
    fn new(context: &eframe::CreationContext<'_>, benchmark: Option<BenchmarkScene>) -> Self {
        context.egui_ctx.set_fonts(theme::fonts());
        if let Some(state) = context.wgpu_render_state.as_ref() {
            let mut renderer = LineRenderer::new(&state.device, state.target_format);
            // Software graphics, as over some remote desktops, show the glass's tint alone.
            renderer.set_blur(
                state.adapter.get_info().device_type != eframe::egui_wgpu::wgpu::DeviceType::Cpu,
            );
            state.renderer.write().callback_resources.insert(renderer);
        }
        let mut doc = Document::default();
        // Benchmarks neither autosave nor offer recovery.
        let folder = recovery::folder().filter(|_| benchmark.is_none());
        let left_behind = folder.as_deref().and_then(recovery::left_behind);
        let autosave = folder
            .as_deref()
            .map(|folder| recovery::Autosave::new(folder, &doc, std::time::Instant::now()));
        let mut panes = standard_panes();
        if let Some(scene) = benchmark
            && benchmark_scene(&mut doc, scene).is_ok()
            && let Some((low, high)) = Outlines::of(doc.state()).bounds()
        {
            for pane in &mut panes {
                pane.camera.frame(low, high);
            }
        }
        Self {
            doc,
            processor: Processor::default(),
            text: Translations::english(),
            typed: String::new(),
            highlight: 0,
            panes,
            active: PERSPECTIVE,
            maximised: STARTING_LAYOUT,
            gumball_on: true,
            naming_layer: false,
            command_field: None,
            // A benchmark must not take over the connection of a window that is in use.
            agent_on: benchmark.is_none(),
            agent: None,
            waiting_approval: None,
            gumball_drag: None,
            sketch_stroke: None,
            gumball_typing: None,
            alt_held: false,
            snaps: SnapSet::default(),
            ortho: false,
            grid_snap: false,
            pick_filter: PickFilter::default(),
            filter_row: false,
            outlines: Outlines::default(),
            scene: Arc::new(Vec::new()),
            shaded_scene: Arc::new(Vec::new()),
            faces: Arc::new(Vec::new()),
            meshes: MeshCache::default(),
            measured: None,
            drawn: None,
            benchmark: benchmark.map(|_| Benchmark::default()),
            autosave,
            left_behind,
            settings: theme::file()
                .filter(|_| benchmark.is_none())
                .map(|file| theme::Settings::load(&file))
                .unwrap_or_default(),
            applied_light: None,
            settings_tab: SettingsTab::General,
            closing: None,
            window_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
            unit_asked: None,
            layout: layout::file()
                .filter(|_| benchmark.is_none())
                .map(|file| layout::Layout::load(&file))
                .unwrap_or_default(),
            command_tab: 0,
            specialization: 0,
            agent_seen: None,
            last_aim: None,
            held_aim: None,
            faces_without: (Vec::new(), Arc::default()),
            clipboard: Vec::new(),
            notices: Vec::new(),
            update_check: None,
            update: None,
            update_download: None,
            assistant_lines: Vec::new(),
            assistant_history: Vec::new(),
            assistant_input: String::new(),
            assistant_busy: None,
            assistant_choice: None,
            assistant_key: String::new(),
            assistant_models: Vec::new(),
            assistant_error: None,
            held: Vec::new(),
            resting: None,
            ghosted: (Vec::new(), Arc::new(Vec::new())),
            focus: false,
            revealed: None,
            drawn_panels: Vec::new(),
            panel_drag: None,
        }
    }

    /// Rebuilds the drawing only when the model or selection changed, as sampling is the slow part.
    fn refresh_scene(&mut self) {
        let selection = self.doc.selection();
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|(state, chosen)| state == self.doc.state() && chosen == &selection);
        if !current {
            self.outlines = Outlines::of_cached(self.doc.state(), &mut self.meshes)
                .inked(theme::current().viewport.ink);
            self.scene = Arc::new(object_lines(&self.outlines, &selection));
            self.shaded_scene = Arc::new(shaded_object_lines(&self.outlines, &selection));
            self.faces = Arc::new(face_vertices(&self.outlines, &selection));
            self.drawn = Some((self.doc.state().clone(), selection));
            self.faces_without.0.clear();
        }
        // The filter can change without the model changing, so it is passed on every frame.
        self.outlines.set_filter(self.pick_filter);
    }

    /// The commands to offer for what has been typed. None while a command is asking for input.
    fn listed_matches(&self) -> Vec<&'static str> {
        if self.processor.is_running() {
            Vec::new()
        } else {
            self.processor.completions(&self.typed)
        }
    }

    /// Moves the highlight with the arrow keys and fills in with Tab. Returns whether it filled.
    fn completion_keys(&mut self, ui: &egui::Ui) -> bool {
        let matches = self.listed_matches();
        let listed = !matches.is_empty();
        let (tab, down, up) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                listed && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                listed && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            )
        });
        if down {
            self.highlight = (self.highlight + 1) % matches.len();
        }
        if up {
            self.highlight = (self.highlight + matches.len() - 1) % matches.len();
        }
        if tab && let Some(name) = matches.get(self.highlight) {
            (*name).clone_into(&mut self.typed);
            self.highlight = 0;
            return true;
        }
        false
    }

    /// Greys in the rest of the highlighted command after what has been typed.
    fn paint_remainder(&self, ui: &egui::Ui, output: &egui::text_edit::TextEditOutput) {
        let Some(name) = self.listed_matches().get(self.highlight).copied() else {
            return;
        };
        let typed = self.typed.len();
        let continues = name
            .get(..typed)
            .is_some_and(|start| start.eq_ignore_ascii_case(&self.typed));
        if continues {
            ui.painter().text(
                output.galley_pos + egui::vec2(output.galley.rect.right(), 0.0),
                egui::Align2::LEFT_TOP,
                &name[typed..],
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
    }

    /// The list of possible commands, floating above the command line. Returns any clicked.
    fn match_list(&self, ui: &egui::Ui, above: egui::Pos2) -> Option<&'static str> {
        let matches = self.listed_matches();
        if matches.is_empty() {
            return None;
        }
        let mut clicked = None;
        egui::Area::new(ui.scope_id().with("command matches"))
            .order(egui::Order::Foreground)
            .fixed_pos(above)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                // As wide as the longest name, one per row, never cut; ten rows, then it scrolls.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let widest = matches
                        .iter()
                        .map(|name| {
                            ui.painter()
                                .layout_no_wrap(
                                    (*name).to_owned(),
                                    font.clone(),
                                    egui::Color32::WHITE,
                                )
                                .size()
                                .x
                        })
                        .fold(0.0, f32::max);
                    ui.set_min_width(widest + 2.0 * ui.spacing().button_padding.x);
                    let row = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                    egui::ScrollArea::vertical()
                        .max_height(row * MOST_MATCH_ROWS)
                        .show(ui, |ui| {
                            for (index, name) in matches.iter().enumerate() {
                                let shown = ui.selectable_label(index == self.highlight, *name);
                                if index == self.highlight {
                                    shown.scroll_to_me(None);
                                }
                                if shown.clicked() {
                                    clicked = Some(*name);
                                }
                            }
                        });
                });
            });
        clicked
    }

    /// The row of snap checkboxes, with the keys for ortho and grid snap.
    fn drawing_aids_bar(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F8)) {
            self.ortho = !self.ortho;
        }
        if ui.input(|input| input.key_pressed(egui::Key::F9)) {
            self.grid_snap = !self.grid_snap;
        }
        let text = &self.text;
        let tick = |ui: &mut egui::Ui, value: &mut bool, key: &'static str| {
            ui.checkbox(value, text.render(&Message::new(key)));
        };
        let snaps = &mut self.snaps;
        let (ortho, grid_snap) = (&mut self.ortho, &mut self.grid_snap);
        let gumball_on = &mut self.gumball_on;
        let (filter, filter_row) = (&mut self.pick_filter, self.filter_row);
        ui.horizontal(|ui| {
            tick(ui, &mut snaps.end, snap_label(SnapKind::End));
            tick(ui, &mut snaps.mid, snap_label(SnapKind::Mid));
            tick(ui, &mut snaps.center, snap_label(SnapKind::Center));
            tick(
                ui,
                &mut snaps.intersection,
                snap_label(SnapKind::Intersection),
            );
            tick(
                ui,
                &mut snaps.perpendicular,
                snap_label(SnapKind::Perpendicular),
            );
            tick(ui, &mut snaps.near, snap_label(SnapKind::Near));
            ui.separator();
            tick(ui, ortho, "ui.ortho");
            tick(ui, grid_snap, "ui.grid_snap");
            tick(ui, gumball_on, "ui.gumball");
            if filter_row {
                ui.separator();
                ui.label(text.render(&Message::new("ui.filter")));
                tick(ui, &mut filter.points, "ui.filter.points");
                tick(ui, &mut filter.curves, "ui.filter.curves");
                tick(ui, &mut filter.surfaces, "ui.filter.surfaces");
                tick(ui, &mut filter.solids, "ui.filter.solids");
                let mut disabled = !filter.on;
                tick(ui, &mut disabled, "ui.filter.disable");
                filter.on = !disabled;
            }
        });
    }

    fn command_line(&mut self, ui: &mut egui::Ui) {
        // The command bar's own lines come first, then the history; together they fill the rows,
        // the oldest going as new ones come.
        let log = self.processor.log().to_vec();
        let (blanks, skip, shown) = history_rows(&log, self.notices.len());
        for _ in 0..blanks {
            ui.label(" ");
        }
        self.notices(ui, skip);
        for row in shown {
            ui.add(egui::Label::new(row.as_str()).truncate());
        }
        let prompt = self
            .processor
            .prompt()
            .unwrap_or_else(|| self.text.render(&Message::new("ui.command")));
        // These keys must be taken before the text box, which would otherwise act on them.
        let filled = self.completion_keys(ui);
        let mut clicked_option = None;
        let mut submit = false;
        let mut clicked = None;
        let mut changed = false;
        ui.horizontal(|ui| {
            // The question, then its options as buttons, which answer as typing them would.
            if let Some(question) = self.processor.question() {
                ui.label(question);
                for (name, label) in self.processor.options() {
                    let chip = egui::Button::new(egui::RichText::new(label).color(theme::ACCENT))
                        .small()
                        .frame(true);
                    if ui.add(chip).on_hover_text(name).clicked() {
                        clicked_option = Some(name);
                    }
                }
                ui.label(":");
            } else {
                ui.label(format!("{prompt}:"));
            }
            let output = egui::TextEdit::singleline(&mut self.typed)
                .desired_width(f32::INFINITY)
                .show(ui);
            self.command_field = Some(output.response.id);
            if !self.naming_layer {
                output.response.request_focus();
            }
            changed = output.response.changed();
            if filled {
                // Without this the cursor stays mid-word and the next key lands there.
                let mut state = output.state.clone();
                let end = egui::text::CCursor::new(self.typed.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), output.response.id);
            }
            self.paint_remainder(ui, &output);
            clicked = self.match_list(ui, output.response.rect.left_top());
            submit = !self.naming_layer && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
        if let Some(name) = clicked_option {
            self.typed.clear();
            self.processor.submit(&mut self.doc, name);
        }
        self.drawing_aids_bar(ui);
        let wants_path = self
            .processor
            .current_request()
            .is_some_and(|request| request.kind == Kind::Path);
        // Space runs the command as in Rhino, except where a file name may contain one.
        if !wants_path && self.typed.ends_with(' ') {
            self.typed.pop();
            submit = true;
        }
        if changed && !submit {
            self.highlight = 0;
        }
        if let Some(name) = clicked {
            self.typed.clear();
            self.highlight = 0;
            self.processor.submit(&mut self.doc, name);
        } else if submit {
            let line = self.processor.autofilled(&self.typed, self.highlight);
            self.typed.clear();
            self.highlight = 0;
            // With a distance or angle held, Enter places the point where the pointer aims.
            match self.last_aim {
                Some(at) if line.is_empty() && self.processor.lock().is_set() => {
                    self.processor.click(&mut self.doc, at);
                }
                _ => self.processor.submit(&mut self.doc, &line),
            }
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            // With nothing running and nothing typed, Escape clears the selection.
            if !self.processor.is_running() && self.typed.is_empty() {
                self.doc.select(&[]);
                self.doc.select_grips(&[]);
                self.doc.select_edges(&[]);
                self.doc.select_faces(&[]);
            }
            self.typed.clear();
            self.highlight = 0;
            self.processor.cancel(&mut self.doc);
        }
        let delete = !self.naming_layer && ui.input(|input| input.key_pressed(egui::Key::Delete));
        let idle = self.typed.is_empty() && !self.processor.is_running();
        // F10 and F11 show and hide control points, as in Rhino.
        let (points_on, points_off) = ui.input(|input| {
            (
                input.key_pressed(egui::Key::F10),
                input.key_pressed(egui::Key::F11),
            )
        });
        if idle && points_on {
            self.processor.submit(&mut self.doc, "pointson");
        } else if idle && points_off {
            self.processor.submit(&mut self.doc, "pointsoff");
        }
        // With control points picked, Delete takes them away rather than whole curves.
        if delete && idle && !self.doc.grip_selection().is_empty() {
            self.processor.submit(&mut self.doc, "removecontrolpoint");
        } else if delete && idle {
            self.processor.submit(&mut self.doc, "delete");
        }
    }

    /// The held points and their guidelines on a viewport's plane: along its X and Y, and at
    /// 45° unless ortho is on (Shift flipping it for the moment).
    fn guides(&self, pane: usize, shift: bool) -> curva_viewport::Guides {
        let plane = self.panes[pane].plane;
        curva_viewport::Guides {
            held: self.held.clone(),
            axes: [plane.x, plane.y],
            diagonals: self.ortho == shift,
        }
    }

    /// Holds a snap point the pointer has rested on for a moment, or lets it go if it was held,
    /// as Rhino's SmartTrack does. Everything held is let go when no command is asking for a
    /// point.
    fn rest_on(&mut self, ctx: &egui::Context, hit: Option<SnapHit>) {
        if !self.takes_position() {
            self.held.clear();
            self.resting = None;
            return;
        }
        let holdable = hit.filter(|hit| {
            matches!(
                hit.kind,
                SnapKind::End | SnapKind::Mid | SnapKind::Center | SnapKind::Intersection
            )
        });
        let Some(hit) = holdable else {
            self.resting = None;
            return;
        };
        let now = ctx.input(|input| input.time);
        match &mut self.resting {
            Some((point, since, acted)) if *point == hit.point => {
                if !*acted && now - *since >= HOLD_AFTER {
                    *acted = true;
                    let mut guides = curva_viewport::Guides {
                        held: std::mem::take(&mut self.held),
                        ..curva_viewport::Guides::default()
                    };
                    guides.toggle(hit.point);
                    self.held = guides.held;
                } else if !*acted {
                    // Come back when the rest is long enough, even if the mouse stays still.
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD_AFTER));
                }
            }
            _ => {
                self.resting = Some((hit.point, now, false));
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD_AFTER));
            }
        }
    }

    /// The held points and their guidelines, dashed and see-through in the accent orange.
    fn paint_guides(&self, ui: &egui::Ui, pane: usize, viewport: egui::Rect, size: [f64; 2]) {
        if self.held.is_empty() {
            return;
        }
        let camera = &self.panes[pane].camera;
        let painter = ui.painter_at(viewport);
        let on_screen = |p: Point| {
            camera
                .project(p, size)
                .map(|at| viewport.min + egui::vec2(at[0] as f32, at[1] as f32))
        };
        let faint = theme::ACCENT.gamma_multiply(0.45);
        let reach = camera.distance * 20.0;
        for (point, direction) in self.guides(pane, false).lines() {
            let end = |sign: f64| std::array::from_fn(|i| point[i] + direction[i] * reach * sign);
            if let (Some(a), Some(b)) = (on_screen(end(-1.0)), on_screen(end(1.0))) {
                painter.extend(egui::Shape::dashed_line(
                    &[a, b],
                    egui::Stroke::new(1.0, faint),
                    6.0,
                    4.0,
                ));
            }
        }
        for point in &self.held {
            if let Some(at) = on_screen(*point) {
                painter.circle_filled(at, 3.5, theme::ACCENT);
            }
        }
    }

    fn takes_position(&self) -> bool {
        self.processor.current_request().is_some_and(|request| {
            matches!(
                request.kind,
                Kind::Point | Kind::PointOrNumber | Kind::PickPart
            ) || (request.kind == Kind::Number && request.from.is_some())
        })
    }

    /// Where a cursor position in a viewport lands for the point being asked for: an object snap
    /// if one is in reach, otherwise the viewport's plane with ortho and grid snap applied. Shift
    /// flips ortho for the moment and Alt turns object snaps off for the moment, as in Rhino.
    fn aim(
        &self,
        ui: &egui::Ui,
        pane: usize,
        screen: [f64; 2],
        size: [f64; 2],
    ) -> Option<(Point, Option<SnapHit>)> {
        let (shift, alt) = ui.input(|input| (input.modifiers.shift, input.modifiers.alt));
        let request = self.processor.current_request()?;
        let view = &self.panes[pane];
        aim_at(&Aiming {
            outlines: &self.outlines,
            request,
            camera: &view.camera,
            plane: &view.plane,
            display: view.display,
            snaps: self.snaps,
            guides: self.guides(pane, shift),
            ortho: self.ortho != shift,
            grid_snap: self.grid_snap,
            snaps_off: alt,
            screen,
            size,
        })
    }

    /// The gumball for the current selection, if it should show.
    fn gumball(&self) -> Option<Gumball> {
        if !self.gumball_on || self.processor.is_running() {
            return None;
        }
        // Picked control points take the gumball, as in Rhino; it sits at their middle.
        let grips = self.picked_grip_points();
        if !grips.is_empty() {
            let count = grips.len() as f64;
            let sum = grips.iter().fold([0.0; 3], |total, p| {
                [total[0] + p[0], total[1] + p[1], total[2] + p[2]]
            });
            return Some(Gumball {
                centre: sum.map(|value| value / count),
            });
        }
        let centre = self.outlines.centre_of(&self.doc.selection())?;
        Some(Gumball { centre })
    }

    /// Where the picked control points are.
    fn picked_grip_points(&self) -> Vec<Point> {
        let picked = self.doc.grip_selection();
        if picked.is_empty() {
            return Vec::new();
        }
        self.doc
            .shown_grips()
            .into_iter()
            .filter(|(grip, _)| picked.contains(grip))
            .map(|(_, at)| at)
            .collect()
    }

    /// What the drag under way would do.
    fn gumball_change(&self, drag: &GumballDrag) -> Option<GumballChange> {
        let gumball = self.gumball()?;
        let camera = &self.panes[drag.pane].camera;
        // The circle extrudes curves and open surfaces and stretches closed solids; Shift
        // stretches anything, and Shift with Ctrl scales evenly.
        let handle = match drag.handle {
            Handle::Extrude(index) if drag.even || self.selection_all_solid() => {
                Handle::Scale(index)
            }
            other => other,
        };
        let even_scale = drag.even && drag.ctrl;
        let change =
            gumball.change_with(camera, drag.size, handle, drag.from, drag.now, even_scale)?;
        // Object snaps apply to the point dragged: the centre lines up with the snapped point
        // along the axis, or moves to it in the plane, as in Rhino. Alt turns them off.
        let alt = self.alt_held;
        let Some(hit) = self
            .outlines
            .snap(camera, drag.now, drag.size, &self.snaps, None)
            .filter(|_| !alt)
        else {
            return Some(change);
        };
        let axis_of =
            |index: usize| -> Point { std::array::from_fn(|i| f64::from(u8::from(i == index))) };
        let from_centre = |p: Point| -> Point { std::array::from_fn(|i| p[i] - gumball.centre[i]) };
        let to_hit = from_centre(hit.point);
        let along = |v: Point, axis: Point| (0..3).map(|i| v[i] * axis[i]).sum::<f64>();
        Some(match (handle, change) {
            (Handle::Move(index), _) => {
                let axis = axis_of(index);
                GumballChange::Move(axis.map(|v| v * along(to_hit, axis)))
            }
            (Handle::Extrude(index), _) => GumballChange::Extrude {
                axis: axis_of(index),
                distance: along(to_hit, axis_of(index)),
            },
            // A stretch takes the snapped point to where the circle was.
            (Handle::Scale(index), GumballChange::Stretch { origin, axis, .. }) => {
                let held = gumball
                    .handle_point(camera, drag.size, Handle::Scale(index))
                    .map(|p| along(from_centre(p), axis))
                    .unwrap_or(1.0);
                let factor = along(to_hit, axis) / held;
                if factor.is_finite() && factor > 1e-9 {
                    GumballChange::Stretch {
                        origin,
                        axis,
                        factor,
                    }
                } else {
                    change
                }
            }
            (Handle::Plane(_) | Handle::Centre, _) => {
                let index = match handle {
                    Handle::Plane(index) => index,
                    _ => 2,
                };
                let normal = axis_of(index);
                let off = along(to_hit, normal);
                GumballChange::Move(std::array::from_fn(|i| to_hit[i] - normal[i] * off))
            }
            (_, change) => change,
        })
    }

    /// Whether everything selected is a closed solid, which the gumball's circle stretches
    /// rather than extrudes.
    fn selection_all_solid(&self) -> bool {
        let shapes = self.selected_shapes();
        !shapes.is_empty()
            && shapes
                .iter()
                .all(|(_, shape)| shape.body().is_some_and(curva_kernel::Body::is_solid))
    }

    /// Sketch draws while the mouse button is held: a point each few pixels the pointer moves,
    /// and letting go ends the curve. Returns whether it took the mouse.
    fn sketch_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) -> bool {
        let sketching = self
            .processor
            .current_request()
            .is_some_and(|request| request.prompt.key == "prompt.sketch.point");
        if !sketching {
            self.sketch_stroke = None;
            return false;
        }
        let (down, released, at) = ui.input(|input| {
            (
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.pointer.latest_pos(),
            )
        });
        if released {
            if let Some((_, fed)) = self.sketch_stroke.take()
                && fed >= 2
            {
                self.processor.submit(&mut self.doc, "");
            }
            return true;
        }
        let Some(at) = at.filter(|p| response.rect.contains(*p)) else {
            return self.sketch_stroke.is_some();
        };
        if !down {
            return false;
        }
        let far_enough = self
            .sketch_stroke
            .is_none_or(|(last, _)| last.distance(at) >= SKETCH_STEP);
        if far_enough {
            let offset = at - response.rect.min;
            let local = [f64::from(offset.x), f64::from(offset.y)];
            if let Some((point, _)) = self.aim(ui, pane, local, size) {
                self.active = pane;
                self.processor.click(&mut self.doc, point);
                let fed = self.sketch_stroke.map_or(0, |(_, n)| n) + 1;
                self.sketch_stroke = Some((at, fed));
            }
        }
        true
    }

    /// Runs the gumball for one viewport. Returns whether it took the mouse.
    fn gumball_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
        events: &[egui::Event],
    ) -> bool {
        let origin = response.rect.min;
        let local = |position: egui::Pos2| {
            let offset = position - origin;
            [f64::from(offset.x), f64::from(offset.y)]
        };
        if let Some(mut drag) = self.gumball_drag {
            if drag.pane != pane {
                return false;
            }
            if let Some(now) = ui.input(|input| input.pointer.latest_pos()) {
                drag.now = local(now);
                drag.even = ui.input(|input| input.modifiers.shift);
                drag.ctrl = ui.input(|input| input.modifiers.command);
                self.gumball_drag = Some(drag);
            }
            let released = events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    }
                )
            });
            if released {
                self.finish_gumball(&drag);
                self.gumball_drag = None;
            }
            return true;
        }
        let Some(gumball) = self.gumball() else {
            return false;
        };
        for event in events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } = event
                && response.rect.contains(*pos)
                && let Some(handle) = gumball.handle_at(&self.panes[pane].camera, local(*pos), size)
            {
                self.active = pane;
                // Ctrl-dragging an arrow extrudes along it, as in Rhino.
                let handle = match handle {
                    Handle::Move(axis) if ui.input(|input| input.modifiers.command) => {
                        Handle::Extrude(axis)
                    }
                    other => other,
                };
                self.gumball_drag = Some(GumballDrag {
                    pane,
                    handle,
                    size,
                    from: local(*pos),
                    now: local(*pos),
                    even: false,
                    ctrl: false,
                });
                return true;
            }
        }
        false
    }

    /// Applies a finished drag as one undo step.
    fn finish_gumball(&mut self, drag: &GumballDrag) {
        // A click without a drag asks for an exact value, as in Rhino.
        if drag.from == drag.now {
            self.gumball_typing = Some((drag.handle, String::new()));
            return;
        }
        let Some(change) = self.gumball_change(drag) else {
            return;
        };
        self.apply_gumball(change);
    }

    /// Applies a gumball change to the selection, or its picked control points, as one undo step.
    fn apply_gumball(&mut self, change: GumballChange) {
        let before = self.doc.state().clone();
        let grips = self.doc.grip_selection();
        if !grips.is_empty() {
            match self.doc.grips_changed(&grips, |p| change.moved_point(p)) {
                Ok(made) => {
                    for (id, curve) in made {
                        self.doc.replace(id, curve);
                    }
                    self.doc.refresh_history();
                    self.doc.record_change(before);
                }
                Err(error) => self.processor.note(error.to_string()),
            }
            return;
        }
        let selection = self.doc.selection();
        match change {
            GumballChange::Move(by) => self.doc.translate(&selection, by),
            GumballChange::Turn(turn) => self.doc.transform(&selection, &turn),
            GumballChange::Stretch {
                origin,
                axis,
                factor,
            } => {
                for (id, shape) in self.selected_shapes() {
                    let stretched = match &shape {
                        Shape::Curve(curve) => {
                            curve.stretched(origin, axis, factor).map(Shape::Curve)
                        }
                        Shape::Body(body) => body.stretched(origin, axis, factor).map(Shape::Body),
                    };
                    match stretched {
                        Ok(made) => self.doc.replace_shape(id, made),
                        Err(error) => self.processor.note(error.to_string()),
                    }
                }
            }
            GumballChange::Extrude { axis, distance } => self.gumball_extrude(axis, distance),
        }
        self.doc.refresh_history();
        self.doc.record_change(before);
    }

    /// The selected objects and their shapes.
    fn selected_shapes(&self) -> Vec<(u64, Shape)> {
        let selection = self.doc.selection();
        self.doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.contains(&object.id))
            .map(|object| (object.id, object.shape.clone()))
            .collect()
    }

    /// The gumball's extrude circle, as `ExtrudeCrv` and `ExtrudeSrf` do: curves become surfaces
    /// (closed ones solids), and flat surfaces facing that way become solids. The originals
    /// stay, as in Rhino.
    fn gumball_extrude(&mut self, axis: Point, distance: f64) {
        if distance.abs() < 1e-9 {
            return;
        }
        for (id, shape) in self.selected_shapes() {
            let layer = self.doc.layer_of(id);
            let Some(made) = gumball_extruded(&shape, axis, distance) else {
                continue;
            };
            match made {
                Ok(body) => {
                    self.doc.add_shape_on(Shape::Body(body), layer);
                }
                Err(error) => self.processor.note(error.to_string()),
            }
        }
    }

    /// The small box at the cursor for an exact gumball value, after a click on a handle
    /// without a drag: a distance, an angle or a factor. Enter applies it; Escape closes it.
    fn gumball_value_box(&mut self, ctx: &egui::Context) {
        let Some((handle, mut typed)) = self.gumball_typing.take() else {
            return;
        };
        // The circle asks for a factor where it stretches, a distance where it extrudes.
        let asked = match handle {
            Handle::Extrude(index) if self.selection_all_solid() => Handle::Scale(index),
            other => other,
        };
        let Some(at) = ctx.pointer_latest_pos() else {
            return;
        };
        let key = match asked {
            Handle::Turn(_) => "ui.gumball.angle",
            Handle::Scale(_) => "ui.gumball.factor",
            _ => "ui.gumball.distance",
        };
        let mut done = false;
        let mut keep = true;
        egui::Area::new(egui::Id::NULL.with("gumball value"))
            .fixed_pos(at + egui::vec2(14.0, 14.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(self.text.render(&Message::new(key)));
                    let field = ui.add(egui::TextEdit::singleline(&mut typed).desired_width(90.0));
                    field.request_focus();
                    if ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    }) {
                        done = true;
                    }
                    if ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        keep = false;
                    }
                });
            });
        if done {
            let even = ctx.input(|input| input.modifiers.shift);
            if let (Ok(value), Some(gumball)) = (typed.trim().parse::<f64>(), self.gumball())
                && let Some(change) = gumball.typed(asked, value, even)
            {
                self.apply_gumball(change);
            }
        } else if keep {
            self.gumball_typing = Some((handle, typed));
        }
    }

    /// The selected objects as the drag under way would leave them.
    fn gumball_preview(&self) -> Vec<Curve> {
        let Some(change) = self
            .gumball_drag
            .and_then(|drag| self.gumball_change(&drag))
        else {
            return Vec::new();
        };
        let grips = self.doc.grip_selection();
        if !grips.is_empty() {
            return self
                .doc
                .grips_changed(&grips, |p| change.moved_point(p))
                .map(|made| made.into_iter().map(|(_, curve)| curve).collect())
                .unwrap_or_default();
        }
        let selection = self.doc.selection();
        self.doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.binary_search(&object.id).is_ok())
            .take(MOST_GUMBALL_PREVIEW)
            .flat_map(|object| object.shape.outlines())
            .map(|curve| change.applied(&curve))
            .collect()
    }

    fn paint_gumball(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let Some(gumball) = self.gumball() else {
            return;
        };
        let camera = &self.panes[pane].camera;
        let hovered = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|position| rect.contains(*position))
            .and_then(|position| {
                let offset = position - rect.min;
                gumball.handle_at(camera, [f64::from(offset.x), f64::from(offset.y)], size)
            });
        let dragged = self
            .gumball_drag
            .filter(|drag| drag.pane == pane)
            .map(|d| d.handle);
        let painter = ui.painter_at(rect);
        if hovered.is_some() || dragged.is_some() {
            ui.ctx().set_cursor_icon(if dragged.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        for (handle, run) in gumball.outlines(camera, size) {
            let lit = Some(handle) == hovered || Some(handle) == dragged;
            let colour = if lit {
                theme::ACCENT
            } else {
                handle
                    .axis()
                    .map_or(theme::current().text, |axis| GUMBALL_AXES[axis])
            };
            let points: Vec<egui::Pos2> = run
                .iter()
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                .collect();
            let stroke = egui::Stroke::new(2.0, colour);
            match handle {
                Handle::Move(_) => {
                    // An arrowhead at the tip.
                    if let (Some(start), Some(tip)) = (points.first(), points.last()) {
                        let along = (*tip - *start).normalized();
                        let side = egui::vec2(-along.y, along.x);
                        let base = *tip - along * GUMBALL_TIP * 2.2;
                        painter.add(egui::Shape::convex_polygon(
                            vec![*tip, base + side * GUMBALL_TIP, base - side * GUMBALL_TIP],
                            colour,
                            egui::Stroke::NONE,
                        ));
                    }
                    painter.add(egui::Shape::line(points, stroke));
                }
                Handle::Turn(_) => {
                    painter.add(egui::Shape::line(points, stroke));
                }
                Handle::Extrude(_) => {
                    painter.add(egui::Shape::closed_line(points, stroke));
                }
                Handle::Plane(_) => {
                    let fill = colour.gamma_multiply(if lit { 0.8 } else { 0.45 });
                    painter.add(egui::Shape::convex_polygon(points, fill, stroke));
                }
                Handle::Scale(_) | Handle::Centre => {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        colour,
                        egui::Stroke::NONE,
                    ));
                }
            }
        }
        // While dragging, what the drag amounts to beside the pointer.
        if let Some(drag) = self.gumball_drag.filter(|drag| drag.pane == pane)
            && let Some(change) = self.gumball_change(&drag)
            && let Some(pointer) = ui.input(|input| input.pointer.latest_pos())
        {
            let (value, amount) = change.amount();
            let text = match amount {
                curva_viewport::Amount::Distance => {
                    format!("{value:.3} {}", self.doc.state().unit.symbol())
                }
                curva_viewport::Amount::Angle => format!("{value:.2}°"),
                curva_viewport::Amount::Factor => format!("× {value:.4}"),
            };
            paint_cursor_tag(ui, pointer, &text);
        }
    }

    /// Runs an edit of the model from a panel as one undo step, if it changed anything.
    fn undoable(&mut self, edit: impl FnOnce(&mut Document)) {
        let before = self.doc.state().clone();
        edit(&mut self.doc);
        if self.doc.state() != &before {
            self.doc.record_change(before);
        }
    }

    /// A drop-down of print widths (spec `line-weights.md` §5): `By layer` when `by_layer`,
    /// `Default`, Rhino's common widths, and a field for any other. `shown` is what it shows;
    /// returns the width chosen, if one was.
    fn width_choice(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        shown: Option<curva_document::PrintWidth>,
        by_layer: bool,
    ) -> Option<curva_document::PrintWidth> {
        use curva_document::PrintWidth;
        let text = |key: &'static str| self.text.render(&Message::new(key));
        let named = |width: PrintWidth| match width {
            PrintWidth::ByLayer => text("ui.width.by_layer"),
            PrintWidth::Default => text("ui.width.default"),
            PrintWidth::Millimetres(mm) => self
                .text
                .render(&Message::with("ui.width.mm", vec![format!("{mm:.2}")])),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(shown.map_or_else(|| text("ui.width.varies"), named))
            .width(84.0)
            .show_ui(ui, |ui| {
                let mut choices = Vec::new();
                if by_layer {
                    choices.push(PrintWidth::ByLayer);
                }
                choices.push(PrintWidth::Default);
                choices.extend(PrintWidth::COMMON.map(PrintWidth::Millimetres));
                for choice in choices {
                    if ui
                        .selectable_label(shown == Some(choice), named(choice))
                        .clicked()
                    {
                        chosen = Some(choice);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(text("ui.width.custom"));
                    let mut mm = match shown {
                        Some(PrintWidth::Millimetres(mm)) => mm,
                        _ => 0.25,
                    };
                    let field = ui.add(
                        egui::DragValue::new(&mut mm)
                            .range(0.01..=PrintWidth::MOST)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix(" mm"),
                    );
                    if field.changed() {
                        chosen = Some(PrintWidth::Millimetres(mm));
                    }
                });
            });
        chosen
    }
    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let layers = self.doc.layers().to_vec();
        let current = self.doc.current_layer();
        egui::Grid::new(ui.scope_id().with("layers")).show(ui, |ui| {
            for layer in &layers {
                if ui.radio(layer.id == current, "").clicked() {
                    self.undoable(|doc| doc.set_current_layer(layer.id));
                }
                let mut name = layer.name.clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(90.0));
                if edit.changed() {
                    self.doc.rename_layer(layer.id, name);
                }
                let mut colour = layer.colour;
                if ui.color_edit_button_rgb(&mut colour).changed() {
                    self.doc.set_layer_colour(layer.id, colour);
                }
                let shown = Some(layer.print_width.map_or(
                    curva_document::PrintWidth::Default,
                    curva_document::PrintWidth::Millimetres,
                ));
                let id = ui.scope_id().with(("layer width", layer.id));
                if let Some(width) = self.width_choice(ui, id, shown, false) {
                    let mm = match width {
                        curva_document::PrintWidth::Millimetres(mm) => Some(mm),
                        _ => None,
                    };
                    self.undoable(|doc| doc.set_layer_print_width(layer.id, mm));
                }
                let mut visible = layer.visible;
                let shown =
                    ui.checkbox(&mut visible, self.text.render(&Message::new("ui.layer.on")));
                if shown.changed() {
                    self.undoable(|doc| doc.set_layer_visible(layer.id, visible));
                }
                let mut locked = layer.locked;
                let lock = ui.checkbox(
                    &mut locked,
                    self.text.render(&Message::new("ui.layer.lock")),
                );
                if lock.changed() {
                    self.undoable(|doc| doc.set_layer_locked(layer.id, locked));
                }
                if ui
                    .button(self.text.render(&Message::new("ui.layer.delete")))
                    .clicked()
                {
                    self.undoable(|doc| {
                        doc.delete_layer(layer.id);
                    });
                }
                ui.end_row();
            }
        });
        if ui
            .button(self.text.render(&Message::new("ui.layer.new")))
            .clicked()
        {
            let number = layers.iter().map(|layer| layer.id).max().unwrap_or(0) + 1;
            let name = self.text.render(&Message::with(
                "ui.layer.new_name",
                vec![number.to_string()],
            ));
            self.undoable(|doc| {
                doc.add_layer(name);
            });
        }
    }

    fn properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text.render(&Message::new("ui.properties")));
        let selection = self.doc.selection();
        if selection.is_empty() {
            ui.label(self.text.render(&Message::new("ui.properties.none")));
            return;
        }
        ui.label(self.text.render(&Message::with(
            "ui.properties.count",
            vec![selection.len().to_string()],
        )));
        if let [only] = selection.as_slice()
            && let Some(object) = self.doc.state().objects.iter().find(|o| o.id == *only)
        {
            match &object.shape {
                Shape::Curve(curve) => {
                    ui.label(self.text.render(&Message::new(curve_kind(curve))));
                    if let Ok(length) = curve.length() {
                        ui.label(self.text.render(&Message::with(
                            "ui.properties.length",
                            vec![format!("{length:.3}")],
                        )));
                    }
                }
                Shape::Body(body) => {
                    // Measuring a curved solid takes a moment, so it is done once per change.
                    let fresh = self
                        .measured
                        .as_ref()
                        .is_some_and(|(id, held, _)| *id == object.id && held == body);
                    if !fresh {
                        let volume = body.volume().ok().filter(|v| *v > 0.0);
                        let reading = match volume {
                            Some(volume) => ("ui.kind.solid", "ui.properties.volume", volume),
                            None => (
                                "ui.kind.surface",
                                "ui.properties.area",
                                body.area().unwrap_or(0.0),
                            ),
                        };
                        self.measured = Some((object.id, body.clone(), reading));
                    }
                    if let Some((_, _, (kind, key, value))) = &self.measured {
                        ui.label(self.text.render(&Message::new(kind)));
                        ui.label(
                            self.text
                                .render(&Message::with(key, vec![format!("{value:.3}")])),
                        );
                    }
                }
            }
        }
        let layers = self.doc.layers().to_vec();
        let shared = self.doc.layer_of(selection[0]).filter(|first| {
            selection
                .iter()
                .all(|id| self.doc.layer_of(*id) == Some(*first))
        });
        let shown = shared
            .and_then(|id| layers.iter().find(|layer| layer.id == id))
            .map_or_else(
                || self.text.render(&Message::new("ui.properties.mixed")),
                |layer| layer.name.clone(),
            );
        let mut chosen = None;
        egui::ComboBox::from_label(self.text.render(&Message::new("ui.properties.layer")))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for layer in &layers {
                    if ui
                        .selectable_label(shared == Some(layer.id), &layer.name)
                        .clicked()
                    {
                        chosen = Some(layer.id);
                    }
                }
            });
        if let Some(layer) = chosen {
            self.undoable(|doc| doc.move_to_layer(&selection, layer));
        }
        let widths: Vec<curva_document::PrintWidth> = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.contains(&object.id))
            .map(|object| object.print_width)
            .collect();
        let shown = widths
            .first()
            .copied()
            .filter(|first| widths.iter().all(|width| width == first));
        ui.horizontal(|ui| {
            ui.label(self.text.render(&Message::new("ui.properties.width")));
            let id = ui.scope_id().with("object width");
            if let Some(width) = self.width_choice(ui, id, shown, true) {
                self.undoable(|doc| doc.set_print_width(&selection, width));
            }
        });
    }

    /// Point objects and point previews, which have no lines to draw.
    fn paint_points(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
        previewed: &[Curve],
    ) {
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let mark = |at: Point, colour: [f32; 3]| {
            if let Some(p) = camera.project(at, size) {
                let [r, g, b] = colour.map(|c| (c * 255.0) as u8);
                painter.rect_filled(
                    egui::Rect::from_center_size(
                        rect.min + egui::vec2(p[0] as f32, p[1] as f32),
                        egui::Vec2::splat(POINT_MARKER_SIZE),
                    ),
                    0.0,
                    egui::Color32::from_rgb(r, g, b),
                );
            }
        };
        for (at, colour) in self.outlines.point_markers(&self.doc.selection()) {
            mark(at, colour);
        }
        for curve in previewed {
            if let Curve::Point { at } = curve {
                mark(*at, curva_viewport::PREVIEW_COLOUR);
            }
        }
    }

    /// The faces of a solid picked for a command, tinted in the selection colour.
    fn paint_picked_faces(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let picked = self.doc.face_selection();
        if picked.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let tint = egui::Color32::from_rgba_unmultiplied(255, 220, 60, 90);
        for pick in picked {
            for triangle in self.outlines.face_triangles(pick) {
                let corners: Vec<egui::Pos2> = triangle
                    .iter()
                    .filter_map(|p| camera.project(*p, size))
                    .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                    .collect();
                if corners.len() == 3 {
                    painter.add(egui::Shape::convex_polygon(
                        corners,
                        tint,
                        egui::Stroke::NONE,
                    ));
                }
            }
        }
    }

    /// The edges of a solid picked for a command, drawn thick in the selection colour.
    fn paint_picked_edges(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let picked = self.doc.edge_selection();
        if picked.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        for pick in picked {
            let Some(points) = self.outlines.edge_points(pick) else {
                continue;
            };
            let screen: Vec<egui::Pos2> = points
                .iter()
                .filter_map(|p| camera.project(*p, size))
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                .collect();
            for pair in screen.windows(2) {
                painter.line_segment([pair[0], pair[1]], egui::Stroke::new(3.0, GRIP_PICKED));
            }
        }
    }

    /// Control points on show: the lines between a smooth curve's points, then the points,
    /// picked ones in yellow.
    fn paint_grips(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let shown = self.doc.shown_grips();
        if shown.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let screen = |at: Point| {
            camera
                .project(at, size)
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
        };
        for object in &self.doc.state().objects {
            let straight = matches!(
                object.curve(),
                Some(Curve::Line { .. } | Curve::Polyline { .. })
            );
            if straight || !shown.iter().any(|((id, _), _)| *id == object.id) {
                continue;
            }
            let corners: Vec<egui::Pos2> = shown
                .iter()
                .filter(|((id, _), _)| *id == object.id)
                .filter_map(|(_, at)| screen(*at))
                .collect();
            for pair in corners.windows(2) {
                painter.line_segment([pair[0], pair[1]], egui::Stroke::new(1.0, CONTROL_POLYGON));
            }
        }
        let picked = self.doc.grip_selection();
        for (grip, at) in shown {
            if let Some(centre) = screen(at) {
                let square = egui::Rect::from_center_size(centre, egui::Vec2::splat(GRIP_SIZE));
                let colour = if picked.contains(&grip) {
                    GRIP_PICKED
                } else {
                    GRIP_COLOUR
                };
                painter.rect_filled(square, 0.0, colour);
                painter.rect_stroke(
                    square,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// The control points a click or box hits, nearest first for a click.
    fn grips_hit(
        &self,
        camera: &Camera,
        (from, to): ([f64; 2], [f64; 2]),
        boxed: bool,
        size: [f64; 2],
    ) -> Vec<Grip> {
        let on_screen = self
            .doc
            .shown_grips()
            .into_iter()
            .filter_map(|(grip, at)| camera.project(at, size).map(|p| (grip, p)));
        if boxed {
            let (low, high) = (
                [from[0].min(to[0]), from[1].min(to[1])],
                [from[0].max(to[0]), from[1].max(to[1])],
            );
            return on_screen
                .filter(|(_, p)| {
                    (low[0]..=high[0]).contains(&p[0]) && (low[1]..=high[1]).contains(&p[1])
                })
                .map(|(grip, _)| grip)
                .collect();
        }
        on_screen
            .map(|(grip, p)| (grip, (p[0] - from[0]).hypot(p[1] - from[1])))
            .filter(|(_, reach)| *reach <= PICK_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(grip, _)| grip)
            .into_iter()
            .collect()
    }

    /// Starts or stops listening to match the checkbox.
    fn keep_agent_link(&mut self, ctx: &egui::Context) {
        if self.agent_on && self.agent.is_none() {
            let waker = ctx.clone();
            match curva_mcp::listen(move || waker.request_repaint()) {
                Ok(listener) => self.agent = Some(listener),
                Err(error) => {
                    self.agent_on = false;
                    self.processor.note(error.to_string());
                }
            }
        } else if !self.agent_on {
            self.agent = None;
            self.waiting_approval = None;
        }
    }

    fn answer_agent(&mut self, request: &curva_mcp::AgentRequest) {
        // Earlier requests may have changed the view, and a picture shows what the window does.
        self.apply_view_request();
        let camera = self.panes[self.active].camera;
        let ghosted = self.panes[self.active].display == DisplayMode::Ghosted;
        let draw = move |doc: &Document| picture_from(doc, &camera, ghosted);
        let answer =
            Session::new(&mut self.doc, &mut self.processor, &draw).handle(&request.message);
        self.apply_view_request();
        // A client that has gone away no longer needs its answer.
        let _ = request.answer.send(answer);
    }

    /// Answers what agents have asked, stopping at anything the user must approve first.
    fn serve_agents(&mut self) {
        while self.waiting_approval.is_none() {
            let Some(request) = self
                .agent
                .as_ref()
                .and_then(|agent| agent.requests.try_recv().ok())
            else {
                return;
            };
            self.agent_seen = Some(std::time::Instant::now());
            if let Some(line) = curva_mcp::needs_approval(&request.message) {
                self.waiting_approval = Some((request, line));
            } else {
                self.answer_agent(&request);
            }
        }
    }

    /// Asks the user about an action an agent wants to take.
    fn approval_window(&mut self, ui: &egui::Ui) {
        let Some((_, line)) = &self.waiting_approval else {
            return;
        };
        let question = self
            .text
            .render(&Message::with("ui.agent.approve", vec![line.clone()]));
        let (allow_text, decline_text) = (
            self.text.render(&Message::new("ui.agent.allow")),
            self.text.render(&Message::new("ui.agent.decline")),
        );
        let (mut allow, mut decline) = (false, false);
        egui::Window::new(self.text.render(&Message::new("ui.agent")))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(question);
                ui.horizontal(|ui| {
                    allow = ui.button(allow_text).clicked();
                    decline = ui.button(decline_text).clicked();
                });
            });
        if (allow || decline)
            && let Some((request, _)) = self.waiting_approval.take()
        {
            if allow {
                self.answer_agent(&request);
            } else {
                // A client that has gone away no longer needs its answer.
                let _ = request
                    .answer
                    .send(Some(curva_mcp::declined(&request.message)));
            }
        }
    }

    /// Puts the chosen theme in use (or the system's), when it changes: the interface's look,
    /// the viewports' grids and the colours objects are drawn in.
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let light = self.settings.light(ctx.system_theme());
        if self.applied_light == Some(light) {
            return;
        }
        self.applied_light = Some(light);
        theme::use_light(light);
        ctx.set_visuals(theme::visuals(theme::current()));
        // One set of sizes for every panel, the top bar included.
        ctx.all_styles_mut(|style| {
            use egui::{FontFamily, FontId, TextStyle};
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(18.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            );
        });
        for pane in &mut self.panes {
            let plane = pane.plane;
            pane.set_plane(plane);
        }
        self.drawn = None;
    }

    /// The undo keys, taken before the command line can see them: Ctrl+Z undoes and Ctrl+Y or
    /// Ctrl+Shift+Z redoes the last change to the model, even while the command line has focus,
    /// unless it holds typed text, which keeps its own undo. Alt+Z and Alt+Shift+Z step through
    /// the last-used viewport's views. Also keeps each viewport's view history.
    fn history_keys(&mut self, ctx: &egui::Context) {
        self.alt_held = ctx.input(|input| input.modifiers.alt);
        use egui::{Key, KeyboardShortcut, Modifiers};
        let moving = ctx.input(|input| {
            input.pointer.any_down() || input.smooth_scroll_delta != egui::Vec2::ZERO
        });
        for pane in &mut self.panes {
            pane.track_view(moving);
        }
        if !self.typed.is_empty() || self.naming_layer {
            return;
        }
        let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        let (redo, undo, view_ahead, view_back) = ctx.input_mut(|input| {
            // The fuller shortcuts are taken first, as a plain one also matches them.
            let view_ahead =
                input.consume_shortcut(&shortcut(Modifiers::ALT | Modifiers::SHIFT, Key::Z));
            let view_back = input.consume_shortcut(&shortcut(Modifiers::ALT, Key::Z));
            // Rhino's own keys for them too, Home and End.
            let view_back = view_back || input.consume_key(Modifiers::NONE, Key::Home);
            let view_ahead = view_ahead || input.consume_key(Modifiers::NONE, Key::End);
            let redo = input
                .consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                || input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Y));
            let undo = input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Z));
            (redo, undo, view_ahead, view_back)
        });
        // The File tab's keys, also taken before the command line sees them.
        let file_command = ctx.input_mut(|input| {
            if input.consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
                Some("SaveAs")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S)) {
                Some("Save")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O)) {
                Some("Open")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N)) {
                Some("New")
            } else {
                None
            }
        });
        if let Some(name) = file_command {
            self.run_from_button(name);
        }
        // The clipboard keys every program has, on the selected objects.
        let (copy, cut, paste, all) = ctx.input_mut(|input| {
            (
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::C)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::X)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::V)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::A)),
            )
        });
        if copy || cut {
            self.clipboard = self
                .selected_shapes()
                .into_iter()
                .map(|(id, shape)| (shape, self.doc.layer_of(id)))
                .collect();
        }
        if cut && !self.clipboard.is_empty() {
            self.run_from_button("delete");
        }
        if paste && !self.clipboard.is_empty() {
            // Pasted in place, as in Rhino, and selected.
            let shapes = self.clipboard.clone();
            self.undoable(|doc| {
                let added: Vec<u64> = shapes
                    .into_iter()
                    .map(|(shape, layer)| doc.add_shape_on(shape, layer))
                    .collect();
                doc.select(&added);
            });
        }
        if all {
            self.run_from_button("selall");
        }
        if view_back || view_ahead {
            self.panes[self.active].step_view(view_ahead);
        }
        if undo || redo {
            // A command in progress is ended first, as its changes are not yet a step.
            if self.processor.is_running() {
                self.processor.cancel(&mut self.doc);
            }
            self.processor
                .submit(&mut self.doc, if redo { "redo" } else { "undo" });
        }
    }

    /// Lays out and draws the panels, sliding hidden ones to their edges (all of them in focus
    /// mode). Returns the area left for the viewports, and how far they may reach to be seen
    /// through glass panels.
    fn panels(&mut self, ctx: &egui::Context, window: egui::Rect) -> (egui::Rect, egui::Rect) {
        if ctx.input(|input| input.key_pressed(egui::Key::F12)) {
            self.toggle_focus();
        }
        self.history_keys(ctx);
        self.window_rect = window;
        // The top bar sits fixed along the top, sliding up out of view in focus mode; the other
        // panels are laid out in the window below it.
        let full_window = window;
        let bar = egui::Rect::from_min_size(
            window.min + egui::Vec2::splat(layout::GAP),
            egui::vec2(window.width() - 2.0 * layout::GAP, TOP_BAR_HEIGHT),
        );
        let lift = ctx.animate_value_with_time(
            egui::Id::NULL.with("top bar slide"),
            if self.focus {
                bar.max.y - window.min.y
            } else {
                0.0
            },
            SLIDE_TIME,
        );
        self.top_bar(ctx, bar.translate(egui::vec2(0.0, -lift)));
        let window = egui::Rect::from_min_max(egui::pos2(window.min.x, bar.max.y), window.max);
        let (placed, free) = self.layout.arrange(window);
        // Typing brings the command line's panel out until the command ends.
        let typing = !self.typed.is_empty() || self.processor.is_running();
        let pointer = ctx.pointer_hover_pos();
        let released = !ctx.input(|input| input.pointer.primary_down());
        // While a panel is dragged over another, that one lights up to show they would merge.
        let merge_into = self.panel_drag.and_then(|drag| {
            let at = ctx.pointer_latest_pos()?;
            self.drawn_panels
                .iter()
                .find(|(p, rect)| *p != drag.panel && rect.contains(at))
                .map(|(p, _)| *p)
        });
        let floating_rects: Vec<egui::Rect> = placed
            .iter()
            .filter(|(panel, _)| self.panel_drag.is_none_or(|drag| drag.panel != *panel))
            .map(|(_, rect)| *rect)
            .collect();
        for (panel, rect) in placed {
            let Some(place) = self.layout.place(panel).cloned() else {
                continue;
            };
            let shown = (!self.focus && !place.hidden)
                || self.revealed == Some(panel)
                || (place.pieces.contains(&layout::Piece::Command) && typing && !place.hidden);
            let edge = self.layout.edge_of(panel, rect, window);
            let (tab, upright) = layout::edge_tab(rect, edge, window);
            let hovered = pointer.is_some_and(|at| {
                tab.contains(at)
                    || self
                        .drawn_panels
                        .iter()
                        .any(|(p, drawn)| *p == panel && drawn.contains(at))
            });
            let target = if shown {
                rect
            } else {
                let visible = if hovered { layout::PEEK } else { 0.0 };
                layout::slid(rect, edge, window, visible)
            };
            let id = egui::Id::NULL.with(("panel slide", panel));
            let offset = egui::vec2(
                ctx.animate_value_with_time(id.with(0), target.min.x - rect.min.x, SLIDE_TIME),
                ctx.animate_value_with_time(id.with(1), target.min.y - rect.min.y, SLIDE_TIME),
            );
            let mut drawn = rect.translate(offset);
            let mut size = rect.size();
            // A panel being dragged follows the pointer, and shows where it would land.
            if let (Some(drag), Some(at)) = (self.panel_drag, ctx.pointer_latest_pos())
                && drag.panel == panel
            {
                drawn = egui::Rect::from_min_size(at - drag.grab, drag.size);
                size = drag.size;
                let landing = self.landing(drag, at, drawn, window, merge_into, &floating_rects);
                if merge_into.is_none() {
                    let shown_piece = drag.piece.unwrap_or(panel);
                    if let Some(id) = landing.panel_of(shown_piece)
                        && let Some((_, spot)) = landing
                            .arrange(window)
                            .0
                            .into_iter()
                            .find(|(p, _)| *p == id)
                    {
                        ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::NULL.with("dock preview"),
                        ))
                        .rect(
                            spot,
                            PANEL_RADIUS,
                            DOCK_PREVIEW_FILL,
                            egui::Stroke::new(1.5, theme::ACCENT.gamma_multiply(0.6)),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                if released {
                    self.layout = landing;
                    self.panel_drag = None;
                    self.save_layout();
                }
            }
            self.drawn_panels.retain(|(p, _)| *p != panel);
            if shown || hovered || self.panel_drag.is_some_and(|d| d.panel == panel) {
                self.drawn_panels.push((panel, drawn));
            }
            let mut events = Vec::new();
            if !shown {
                events.push(self.edge_tab(ctx, &place, tab, upright));
            }
            if shown || hovered || offset != target.min - rect.min || drawn != target {
                events.push(self.panel_area(ctx, &place, size, drawn, merge_into == Some(panel)));
            }
            for event in events {
                self.panel_event(event, panel, &place, drawn, window);
            }
        }
        self.focus_tab(ctx, full_window);
        self.version_label(ctx, full_window);
        // In focus mode the viewports take the window, short of the tabs left at its edges.
        if self.focus {
            let shrunk = full_window.shrink(layout::STRIP);
            return (shrunk, shrunk);
        }
        // The viewports reach under the glass panels, so there is something to see through
        // them; their titles and frames stay in the free area.
        (free, full_window)
    }

    /// Where a dragged panel would go if let go with the pointer at `at`: merged into the panel
    /// under the pointer, docked to a near edge, or floating, settled on the grid.
    fn landing(
        &self,
        drag: PanelDrag,
        at: egui::Pos2,
        drawn: egui::Rect,
        window: egui::Rect,
        merge_into: Option<layout::PanelId>,
        others: &[egui::Rect],
    ) -> layout::Layout {
        if let Some(onto) = merge_into {
            return match drag.piece {
                Some(piece) => {
                    let alone = self.layout.pulled(
                        piece,
                        layout::Dock::Floating { x: 0.0, y: 0.0 },
                        drag.size,
                    );
                    alone.merged(piece, onto)
                }
                None => self.layout.merged(drag.panel, onto),
            };
        }
        let mut size = drag.size;
        let mut dock = layout::drop_dock(at, drawn.min, window);
        if matches!(dock, layout::Dock::Floating { .. }) {
            let settled = layout::settle(drawn, window, others);
            size = settled.size();
            dock = layout::Dock::Floating {
                x: settled.min.x - window.min.x,
                y: settled.min.y - window.min.y,
            };
        }
        match drag.piece {
            Some(piece) => self.layout.pulled(piece, dock, size),
            None => self.layout.moved(drag.panel, dock, size),
        }
    }

    /// Carries out what happened on one panel.
    fn panel_event(
        &mut self,
        event: PanelEvent,
        panel: layout::PanelId,
        place: &layout::Place,
        drawn: egui::Rect,
        window: egui::Rect,
    ) {
        match event {
            PanelEvent::Nothing => {}
            PanelEvent::StripClicked => {
                if place.hidden {
                    self.layout = self.layout.with_hidden(panel, false);
                    self.save_layout();
                } else {
                    self.revealed = Some(panel);
                }
            }
            PanelEvent::DragStarted(grab, piece) => {
                self.panel_drag = Some(PanelDrag {
                    panel,
                    piece: piece.filter(|_| place.pieces.len() > 1),
                    grab,
                    size: drawn.size().min(window.size() * 0.6).max(layout::MIN_PANEL),
                });
            }
            PanelEvent::Resized(sides, by) => {
                self.layout = self.layout.resized_from(panel, sides, by, window);
                self.save_layout();
            }
            PanelEvent::Reset => {
                self.layout = layout::Layout::default();
                self.save_layout();
            }
            PanelEvent::Hide => {
                if self.revealed == Some(panel) {
                    self.revealed = None;
                } else {
                    self.layout = self.layout.with_hidden(panel, true);
                    self.save_layout();
                }
            }
            PanelEvent::TabMoved(from, to) => {
                self.layout = self.layout.reordered(panel, from, to);
                self.save_layout();
            }
            PanelEvent::Close => self.closing = Some(panel),
            PanelEvent::TabChosen(index) => {
                self.layout = self.layout.with_active(panel, index);
                self.save_layout();
            }
        }
    }

    /// The tab a hidden panel leaves at its edge, with its pieces' names written along it:
    /// hovering it peeks the panel out, clicking brings it back, dragging pulls it out.
    fn edge_tab(
        &self,
        ctx: &egui::Context,
        place: &layout::Place,
        tab: egui::Rect,
        upright: bool,
    ) -> PanelEvent {
        let mut event = PanelEvent::Nothing;
        let names: Vec<String> = place
            .pieces
            .iter()
            .map(|piece| self.text.render(&Message::new(piece.name_key())))
            .collect();
        egui::Area::new(egui::Id::NULL.with(("edge tab", place.id())))
            .fixed_pos(tab.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(tab.size(), egui::Sense::click_and_drag());
                let fill = if response.hovered() {
                    theme::current().panel_edge
                } else {
                    theme::current().panel
                };
                ui.painter().rect_filled(rect, 6.0, fill);
                let font = egui::FontId::proportional(12.0);
                let galley =
                    ui.painter()
                        .layout_no_wrap(names.join(" · "), font, theme::current().quiet);
                let text_size = galley.size();
                let (position, angle) = if upright {
                    // Running upwards along a side.
                    (
                        egui::pos2(
                            rect.center().x - text_size.y / 2.0,
                            rect.center().y + text_size.x / 2.0,
                        ),
                        -std::f32::consts::FRAC_PI_2,
                    )
                } else {
                    (rect.center() - text_size / 2.0, 0.0)
                };
                ui.painter().add(
                    egui::epaint::TextShape::new(position, galley, theme::current().quiet)
                        .with_angle(angle),
                );
                if response.clicked() {
                    event = PanelEvent::StripClicked;
                } else if response.drag_started() {
                    event = PanelEvent::DragStarted(egui::vec2(24.0, 12.0), None);
                }
            });
        event
    }

    /// Draws one panel at `drawn`, with rounded corners and a soft shadow: its name centred
    /// along its top, or a tab per piece, which are its grips; a control to hide it; and on a
    /// floating one a corner to resize it by. `glowing` lights it in the accent while another
    /// panel dragged over it would merge into it.
    fn panel_area(
        &mut self,
        ctx: &egui::Context,
        place: &layout::Place,
        size: egui::Vec2,
        drawn: egui::Rect,
        glowing: bool,
    ) -> PanelEvent {
        let rect = drawn;
        let panel = place.id();
        let mut event = PanelEvent::Nothing;
        let glass = self.settings.glass;
        let frame = panel_frame(glowing).fill(if glass {
            theme::current().glass
        } else {
            theme::current().panel
        });
        egui::Area::new(egui::Id::NULL.with(("panel", panel)))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                // Nothing a panel holds may spill outside its place.
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                // The blurred view behind, under the tint the frame paints over it.
                if glass {
                    ui.painter().add(egui::Shape::from(
                        eframe::egui_wgpu::Callback::new_paint_callback(
                            rect,
                            curva_viewport::Glass {
                                rect,
                                radius: f32::from(PANEL_RADIUS),
                            },
                        ),
                    ));
                }
                frame.show(ui, |ui| {
                    let inner = size - egui::Vec2::splat(2.0 * PANEL_MARGIN);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    ui.add_space(TITLE_ROW);
                    match place.showing() {
                        // Inside a scroll area, so nothing a piece holds grows the panel past its place.
                        layout::Piece::CommandTabs => {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| self.command_tabs(ui));
                        }
                        layout::Piece::Layers => {
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| self.layers_panel(ui));
                        }
                        layout::Piece::Properties => {
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| self.properties_panel(ui));
                        }
                        layout::Piece::Command => {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .stick_to_bottom(true)
                                .show(ui, |ui| self.command_line(ui));
                        }
                        layout::Piece::Agent => {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| self.assistant_panel(ui));
                        }
                        layout::Piece::Display => self.display_piece(ui),
                        layout::Piece::Settings => self.settings_piece(ui),
                    }
                });
                // Added after the contents, so these sit above the panel's own controls.
                event = self.panel_title(ui, place, rect);
                if let Some(resize) = resize_handles(ui, place, rect) {
                    event = resize;
                }
            });
        event
    }

    /// A panel's title row: its name centred, or one tab per piece with the showing one in the
    /// accent. The row is the grip: dragging a tab of a panel with several pulls that piece
    /// out, dragging elsewhere moves the panel. A V at its end hides the panel.
    fn panel_title(&self, ui: &egui::Ui, place: &layout::Place, rect: egui::Rect) -> PanelEvent {
        let mut event = PanelEvent::Nothing;
        let row = egui::Rect::from_min_size(
            rect.min + egui::vec2(PANEL_MARGIN, 4.0),
            egui::vec2(rect.width() - 2.0 * PANEL_MARGIN, TITLE_ROW),
        );
        let font = egui::FontId::proportional(TITLE_FONT);
        let galleys: Vec<_> = place
            .pieces
            .iter()
            .map(|piece| {
                ui.painter().layout_no_wrap(
                    self.text.render(&Message::new(piece.name_key())),
                    font.clone(),
                    theme::current().quiet,
                )
            })
            .collect();
        let spacing = 18.0;
        let total: f32 = galleys.iter().map(|g| g.size().x).sum::<f32>()
            + spacing * (galleys.len().saturating_sub(1)) as f32;
        let mut x = row.center().x - total / 2.0;
        // The row as a whole moves the panel.
        let grip = ui
            .interact(
                row,
                ui.scope_id().with("title"),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        if grip.drag_started()
            && let Some(at) = grip.interact_pointer_pos()
        {
            event = PanelEvent::DragStarted(at - rect.min, None);
        }
        grip.context_menu(|ui| {
            if ui
                .button(self.text.render(&Message::new("ui.layout.reset")))
                .clicked()
            {
                event = PanelEvent::Reset;
                ui.close();
            }
        });
        let tabs: Vec<egui::Rect> = galleys
            .iter()
            .scan(x, |left, galley| {
                let tab = egui::Rect::from_min_size(
                    egui::pos2(*left, row.min.y),
                    egui::vec2(galley.size().x, TITLE_ROW),
                );
                *left += galley.size().x + spacing;
                Some(tab)
            })
            .collect();
        for (index, galley) in galleys.into_iter().enumerate() {
            let width = galley.size().x;
            let tab = tabs[index];
            let active =
                place.pieces.len() > 1 && index == place.active.min(place.pieces.len() - 1);
            let colour = if active {
                theme::ACCENT
            } else {
                theme::current().quiet
            };
            if place.pieces.len() > 1 {
                let response = ui
                    .interact(
                        tab.expand(4.0),
                        ui.scope_id().with(("tab", index)),
                        egui::Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                let pointer = ui.input(|input| input.pointer.latest_pos());
                let in_row =
                    pointer.is_some_and(|at| row.expand2(egui::vec2(0.0, 10.0)).contains(at));
                if response.clicked() {
                    event = PanelEvent::TabChosen(index);
                } else if response.dragged()
                    && !in_row
                    && let Some(at) = pointer
                {
                    // Dragged out of the row, the piece comes out as a panel of its own.
                    event = PanelEvent::DragStarted(at - rect.min, Some(place.pieces[index]));
                } else if response.drag_stopped()
                    && in_row
                    && let Some(at) = pointer
                {
                    // Let go along the row, the tab moves there.
                    event = PanelEvent::TabMoved(index, tab_slot(&tabs, at.x));
                }
                if active {
                    ui.painter().line_segment(
                        [
                            egui::pos2(tab.min.x, tab.max.y),
                            egui::pos2(tab.max.x, tab.max.y),
                        ],
                        egui::Stroke::new(2.0, theme::ACCENT),
                    );
                }
            }
            ui.painter().galley(
                egui::pos2(tab.min.x, row.center().y - galley.size().y / 2.0),
                galley,
                colour,
            );
            x += width + spacing;
        }
        let _ = x;
        // At the row's end: the hide V, pointing to the edge the panel hides to, then the cross.
        let edge = self.layout.edge_of(place.id(), rect, self.window_rect);
        let close = egui::Rect::from_center_size(
            egui::pos2(row.max.x - 8.0, row.center().y),
            egui::vec2(16.0, TITLE_ROW),
        );
        let hide = close.translate(egui::vec2(-20.0, 0.0));
        let quiet = theme::current().quiet;
        let lit = |r: &egui::Response| {
            egui::Stroke::new(1.5, if r.hovered() { theme::ACCENT } else { quiet })
        };
        let hidden = ui
            .interact(hide, ui.scope_id().with("hide"), egui::Sense::click())
            .on_hover_text(self.text.render(&Message::new("ui.panel.hide")));
        let towards = match edge {
            layout::Edge::Top => egui::vec2(0.0, -1.0),
            layout::Edge::Bottom => egui::vec2(0.0, 1.0),
            layout::Edge::Left => egui::vec2(-1.0, 0.0),
            layout::Edge::Right => egui::vec2(1.0, 0.0),
        };
        paint_chevron(ui.painter(), hide.center(), towards, lit(&hidden));
        if hidden.clicked() {
            event = PanelEvent::Hide;
        }
        let closed = ui
            .interact(close, ui.scope_id().with("close"), egui::Sense::click())
            .on_hover_text(self.text.render(&Message::new("ui.panel.close")));
        let (mid, arm) = (close.center(), 3.5);
        for (a, b) in [((-arm, -arm), (arm, arm)), ((arm, -arm), (-arm, arm))] {
            ui.painter().line_segment(
                [mid + egui::vec2(a.0, a.1), mid + egui::vec2(b.0, b.1)],
                lit(&closed),
            );
        }
        if closed.clicked() {
            event = PanelEvent::Close;
        }
        event
    }

    /// The software's name and version, small and quiet, at the window's bottom right.
    fn version_label(&self, ctx: &egui::Context, window: egui::Rect) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::NULL.with("version"),
        ));
        painter.text(
            window.max - egui::vec2(8.0, 4.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("Curva {}", env!("CARGO_PKG_VERSION")),
            egui::FontId::proportional(11.0),
            theme::current().quiet,
        );
    }

    /// Hides every panel, or brings them all back.
    fn toggle_focus(&mut self) {
        self.focus = !self.focus;
        self.revealed = None;
    }

    /// The small tab with an arrow at the top centre that turns focus mode on and off. With one
    /// panel brought out of focus mode, it hides that panel again.
    fn focus_tab(&mut self, ctx: &egui::Context, window: egui::Rect) {
        let size = egui::vec2(44.0, 14.0);
        let corner = egui::pos2(window.center().x - size.x / 2.0, window.min.y);
        egui::Area::new(egui::Id::NULL.with("focus tab"))
            .fixed_pos(corner)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                let fill = if response.hovered() {
                    theme::current().panel_edge
                } else {
                    theme::current().panel
                };
                let radius = egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 7,
                    se: 7,
                };
                ui.painter().rect_filled(rect, radius, fill);
                // A V while the panels are hidden, to bring them down; turned over to hide them.
                let (mid, half) = (rect.center(), egui::vec2(6.0, 3.0));
                let tip = if self.focus { half.y } else { -half.y };
                let stroke = egui::Stroke::new(1.5, theme::current().quiet);
                ui.painter().line_segment(
                    [mid + egui::vec2(-half.x, -tip), mid + egui::vec2(0.0, tip)],
                    stroke,
                );
                ui.painter().line_segment(
                    [mid + egui::vec2(0.0, tip), mid + egui::vec2(half.x, -tip)],
                    stroke,
                );
                let tip_text = if self.focus {
                    "ui.focus.off"
                } else {
                    "ui.focus.on"
                };
                if response
                    .on_hover_text(self.text.render(&Message::new(tip_text)))
                    .clicked()
                {
                    if self.revealed.is_some() {
                        self.revealed = None;
                    } else {
                        self.toggle_focus();
                    }
                }
            });
    }

    /// Keeps the panel arrangement for the next session. Losing it only loses the arrangement.
    fn save_layout(&self) {
        if self.benchmark.is_none()
            && let Some(file) = layout::file()
        {
            let _ = self.layout.save(&file);
        }
    }

    /// The command buttons, in tabs. A button runs its command as typing it would, ending any
    /// command already running, as a Rhino toolbar button does.
    fn command_tabs(&mut self, ui: &mut egui::Ui) {
        // Only Freeform has commands yet; the other specializations say when they come.
        if self.specialization != 0 {
            ui.label(self.text.render(&Message::new("ui.specialization.later")));
            return;
        }
        ui.horizontal(|ui| {
            for (index, (key, _)) in COMMAND_TABS.iter().enumerate() {
                let label = self.text.render(&Message::new(key));
                if ui
                    .selectable_label(self.command_tab == index, label)
                    .clicked()
                {
                    self.command_tab = index;
                }
            }
        });
        let mut chosen = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for name in COMMAND_TABS[self.command_tab.min(COMMAND_TABS.len() - 1)].1 {
                    if command_button(ui, name).clicked() {
                        chosen = Some(*name);
                    }
                }
            });
        });
        if let Some(name) = chosen {
            self.run_from_button(name);
        }
    }

    /// The top bar, named Curva: always there and fixed (only focus mode hides it). The file's
    /// name and Curva centred on top; under them File and Settings at the left, and the
    /// specializations as a dial in the middle, the selected one centred.
    fn top_bar(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let glass = self.settings.glass;
        let frame = panel_frame(false).fill(if glass {
            theme::current().glass
        } else {
            theme::current().panel
        });
        let mut chosen = None;
        egui::Area::new(egui::Id::NULL.with("top bar"))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                if glass {
                    ui.painter().add(egui::Shape::from(
                        eframe::egui_wgpu::Callback::new_paint_callback(
                            rect,
                            curva_viewport::Glass {
                                rect,
                                radius: f32::from(PANEL_RADIUS),
                            },
                        ),
                    ));
                }
                frame.show(ui, |ui| {
                    let inner = rect.size() - egui::Vec2::splat(2.0 * PANEL_MARGIN);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    let file = self
                        .doc
                        .path()
                        .and_then(|path| path.file_stem())
                        .map_or_else(
                            || self.text.render(&Message::new("ui.untitled")),
                            |stem| stem.to_string_lossy().into_owned(),
                        );
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{file}  ·  Curva"))
                                .color(theme::current().quiet)
                                .size(13.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.menu_button(self.text.render(&Message::new("ui.tab.file")), |ui| {
                            chosen = self.file_items(ui);
                        });
                        if ui
                            .button(self.text.render(&Message::new("ui.file.settings")))
                            .clicked()
                        {
                            self.open_settings(SettingsTab::General);
                        }
                    });
                    let dial = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, rect.max.y - PANEL_MARGIN - 12.0),
                        egui::vec2(DIAL_WIDTH, 22.0),
                    );
                    self.specialization_dial(ui, dial);
                });
            });
        if let Some(name) = chosen {
            self.run_from_button(name);
        }
    }

    /// The specializations as a dial that loops: the selected one in the centre, its neighbours
    /// either side and fading. Clicking a neighbour or scrolling over the dial turns it.
    fn specialization_dial(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let count = SPECIALIZATIONS.len();
        let response = ui.interact(rect, ui.scope_id().with("dial"), egui::Sense::click());
        if response.hovered() {
            let scroll =
                ui.input(|input| input.smooth_scroll_delta.x + input.smooth_scroll_delta.y);
            if scroll.abs() > 20.0 {
                self.specialization =
                    (self.specialization + if scroll < 0.0 { 1 } else { count - 1 }) % count;
            }
        }
        let spacing = rect.width() / 3.0;
        for offset in [-1_i32, 0, 1] {
            let index = (self.specialization as i32 + offset).rem_euclid(count as i32) as usize;
            let centre = egui::pos2(rect.center().x + spacing * offset as f32, rect.center().y);
            let fade = match offset.abs() {
                0 => 1.0,
                1 => 0.55,
                _ => 0.22,
            };
            let colour = if offset == 0 {
                theme::current().text
            } else {
                theme::current().quiet.gamma_multiply(fade)
            };
            let name = self.text.render(&Message::new(SPECIALIZATIONS[index]));
            let galley = ui.painter().layout_no_wrap(
                name,
                egui::FontId::new(14.0, egui::FontFamily::Name("Bold".into())),
                colour,
            );
            let at = centre - galley.size() / 2.0;
            let hit = egui::Rect::from_min_size(at, galley.size()).expand(4.0);
            ui.painter().galley(at, galley, colour);
            if offset != 0
                && ui
                    .interact(
                        hit,
                        ui.scope_id().with(("dial item", offset)),
                        egui::Sense::click(),
                    )
                    .clicked()
            {
                self.specialization = index;
            }
            if offset == 0 {
                ui.painter().line_segment(
                    [
                        egui::pos2(hit.min.x + 4.0, hit.max.y),
                        egui::pos2(hit.max.x - 4.0, hit.max.y),
                    ],
                    egui::Stroke::new(2.0, theme::ACCENT),
                );
            }
        }
    }

    /// Runs a command as typing it would, ending any command already running, as a Rhino
    /// toolbar button does.
    fn run_from_button(&mut self, name: &str) {
        if self.processor.is_running() {
            self.processor.cancel(&mut self.doc);
        }
        self.typed.clear();
        self.highlight = 0;
        self.processor.submit(&mut self.doc, name);
    }

    /// The File menu: New, Open, Save and Save As run their commands; Import and Export wait for
    /// a format to offer; About opens Settings at its About tab. Returns a command to run.
    fn file_items(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let mut chosen = None;
        for (name, key) in [
            ("New", "ui.file.new"),
            ("Open", "ui.file.open"),
            ("Save", "ui.file.save"),
            ("SaveAs", "ui.file.save_as"),
        ] {
            if ui.button(self.text.render(&Message::new(key))).clicked() {
                chosen = Some(name);
                ui.close();
            }
        }
        ui.separator();
        for key in ["ui.file.import", "ui.file.export"] {
            ui.add_enabled(
                false,
                egui::Button::new(self.text.render(&Message::new(key))),
            )
            .on_disabled_hover_text(self.text.render(&Message::new("ui.file.later")));
        }
        ui.separator();
        if ui
            .button(self.text.render(&Message::new("ui.file.about")))
            .clicked()
        {
            self.open_settings(SettingsTab::About);
            ui.close();
        }
        chosen
    }

    /// Brings the Settings panel out at a tab: back from closed, floating in the centre of the
    /// window, or to the front of the panel holding it.
    fn open_settings(&mut self, tab: SettingsTab) {
        self.settings_tab = tab;
        let window = self.window_rect;
        self.layout = self.layout.restored(layout::Piece::Settings, window);
        if let Some(panel) = self.layout.panel_of(layout::Piece::Settings)
            && let Some(index) = self
                .layout
                .place(panel)
                .and_then(|p| p.pieces.iter().position(|p| *p == layout::Piece::Settings))
        {
            self.layout = self
                .layout
                .with_active(panel, index)
                .with_hidden(panel, false);
        }
        self.save_layout();
    }

    /// The Settings panel, in tabs: General, Interface, AI Assistant and About.
    fn settings_piece(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, key) in [
                (SettingsTab::General, "ui.settings.general"),
                (SettingsTab::Interface, "ui.settings.interface"),
                (SettingsTab::Assistant, "ui.piece.assistant"),
                (SettingsTab::About, "ui.file.about"),
            ] {
                let label = self.text.render(&Message::new(key));
                if ui
                    .selectable_label(self.settings_tab == tab, label)
                    .clicked()
                {
                    self.settings_tab = tab;
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.settings_tab {
                SettingsTab::General => self.general_settings(ui),
                SettingsTab::Interface => self.interface_settings(ui),
                SettingsTab::Assistant => self.assistant_settings(ui),
                SettingsTab::About => self.about_settings(ui),
            });
    }

    /// Settings → General: theme, glass or solid panels, the model's unit, the grid, updates.
    fn general_settings(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.clone();
        let mut unit = self.doc.state().unit;
        let text = &self.text;
        let settings = &mut self.settings;
        let say = |key: &'static str| text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.settings.theme")).strong());
        ui.horizontal(|ui| {
            for (choice, key) in [
                (theme::ThemeChoice::System, "ui.settings.system"),
                (theme::ThemeChoice::Light, "ui.settings.light"),
                (theme::ThemeChoice::Dark, "ui.settings.dark"),
            ] {
                ui.radio_value(&mut settings.theme, choice, say(key));
            }
        });
        ui.label(egui::RichText::new(say("ui.settings.panels")).strong());
        ui.horizontal(|ui| {
            ui.radio_value(&mut settings.glass, true, say("ui.settings.glass"));
            ui.radio_value(&mut settings.glass, false, say("ui.settings.solid"));
        });
        ui.label(egui::RichText::new(say("ui.settings.units")).strong());
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut unit,
                curva_document::Unit::Millimetres,
                say("ui.settings.mm"),
            );
            ui.radio_value(
                &mut unit,
                curva_document::Unit::Metres,
                say("ui.settings.m"),
            );
        });
        ui.checkbox(&mut settings.grid, say("ui.settings.grid"));
        ui.checkbox(&mut settings.check_updates, say("ui.settings.updates"));
        if unit != self.doc.state().unit {
            self.unit_asked = Some(unit);
        }
        if self.settings != before {
            self.applied_light = None;
            if self.benchmark.is_none()
                && let Some(file) = theme::file()
            {
                let _ = self.settings.save(&file);
            }
        }
    }

    /// Settings → Interface: every panel with a switch to show or close it, and a way back to the
    /// standard arrangement.
    fn interface_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            self.text
                .render(&Message::new("ui.settings.interface_note")),
        );
        let window = self.window_rect;
        for piece in layout::Piece::ALL {
            let mut shown = self.layout.is_shown(piece);
            let name = self.text.render(&Message::new(piece.name_key()));
            if ui.checkbox(&mut shown, name).changed() {
                self.layout = if shown {
                    self.layout.restored(piece, window)
                } else {
                    // Only this piece goes, even from a panel with others.
                    let alone = self.layout.pulled(
                        piece,
                        layout::Dock::Floating { x: 0.0, y: 0.0 },
                        layout::FLOATING_STANDARD,
                    );
                    alone.closing(piece)
                };
                self.save_layout();
            }
        }
        ui.add_space(8.0);
        if ui
            .button(self.text.render(&Message::new("ui.layout.reset")))
            .clicked()
        {
            self.layout = layout::Layout::default().restored(layout::Piece::Settings, window);
            self.save_layout();
        }
    }

    /// Settings → About: the version, the licence, and the credits.
    fn about_settings(&self, ui: &mut egui::Ui) {
        ui.heading(format!("Curva {}", env!("CARGO_PKG_VERSION")));
        ui.label(self.text.render(&Message::new("ui.about.licence")));
        ui.separator();
        ui.label(include_str!("../../../ATTRIBUTION.md"));
    }

    /// As Rhino does, asks whether changing the unit should scale the model to keep its real
    /// size or keep the numbers as they are.
    fn unit_question(&mut self, ctx: &egui::Context) {
        let Some(unit) = self.unit_asked else {
            return;
        };
        let mut answer = None;
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let question = self.text.render(&Message::with(
            "ui.units.question",
            vec![unit.symbol().to_owned()],
        ));
        centred_dialog(ctx, "unit question", |ui| {
            ui.label(question);
            ui.horizontal(|ui| {
                if ui.button(say("ui.units.scale")).clicked() {
                    answer = Some(Some(true));
                }
                if ui.button(say("ui.units.keep")).clicked() {
                    answer = Some(Some(false));
                }
                if ui.button(say("ui.units.cancel")).clicked() {
                    answer = Some(None);
                }
            });
        });
        if let Some(choice) = answer {
            self.unit_asked = None;
            if let Some(scale) = choice {
                let before = self.doc.state().clone();
                self.doc.set_unit(unit, scale);
                self.doc.record_change(before);
            }
        }
    }

    /// Asks whether to close a panel, saying where to bring it back, before closing it.
    fn close_question(&mut self, ctx: &egui::Context) {
        let Some(panel) = self.closing else {
            return;
        };
        let names: Vec<String> = self
            .layout
            .place(panel)
            .map(|p| {
                p.pieces
                    .iter()
                    .map(|piece| self.text.render(&Message::new(piece.name_key())))
                    .collect()
            })
            .unwrap_or_default();
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let question = self
            .text
            .render(&Message::with("ui.close.question", vec![names.join(", ")]));
        let mut answer = None;
        centred_dialog(ctx, "close question", |ui| {
            ui.label(egui::RichText::new(question).strong());
            ui.label(say("ui.close.where"));
            ui.horizontal(|ui| {
                if ui.button(say("ui.close.close")).clicked() {
                    answer = Some(true);
                }
                if ui.button(say("ui.units.cancel")).clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(close) = answer {
            self.closing = None;
            if close {
                self.layout = self.layout.closing(panel);
                self.save_layout();
            }
        }
    }

    /// The Display panel: the active viewport's display mode, as a button with a V pointing down
    /// that opens the list of modes.
    fn display_piece(&mut self, ui: &mut egui::Ui) {
        let current = self.panes[self.active].display;
        let label = |mode: DisplayMode| match mode {
            DisplayMode::Wireframe => "ui.display.wireframe",
            DisplayMode::Shaded => "ui.display.shaded",
            DisplayMode::Ghosted => "ui.display.ghosted",
        };
        let title = self.text.render(&Message::new(label(current)));
        let mut chosen = None;
        egui::ComboBox::from_id_salt("display mode")
            .selected_text(title)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for mode in [
                    DisplayMode::Wireframe,
                    DisplayMode::Shaded,
                    DisplayMode::Ghosted,
                ] {
                    let name = self.text.render(&Message::new(label(mode)));
                    if ui.selectable_label(mode == current, name).clicked() {
                        chosen = Some(mode);
                    }
                }
            });
        if let Some(mode) = chosen {
            self.panes[self.active].display = mode;
        }
    }

    /// Settings → AI Assistant.
    fn assistant_settings(&mut self, ui: &mut egui::Ui) {
        self.assistant_connections(ui);
    }

    /// What happens once Curva has opened: the model given on the command line opens, the check
    /// for updates starts in the background, and the command bar says what an update brought and
    /// welcomes the user.
    fn opened(mut self, ctx: &egui::Context, file: Option<&std::path::Path>) -> Self {
        if let Some(path) = file
            && let Ok(doc) = Document::open(path)
        {
            self.doc = doc;
        }
        if self.benchmark.is_some() {
            return self;
        }
        if let Some(version) = updates::just_updated() {
            let (a, b, c) = version;
            let text = format!("{a}.{b}.{c}");
            let line = self
                .text
                .render(&Message::with("ui.update.done", vec![text.clone()]));
            self.notices.push(Notice::new(
                line,
                NoticeAction::Open(format!("{}{text}", updates::NOTES)),
            ));
        }
        let welcome = self.text.render(&Message::new("ui.welcome"));
        self.notices.push(Notice::new(welcome, NoticeAction::None));
        if self.settings.check_updates {
            let (sender, receiver) = std::sync::mpsc::channel();
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(updates::check());
                repaint.request_repaint();
            });
            self.update_check = Some(receiver);
        }
        self
    }

    /// The command bar's own lines above the command history: an update on offer, what an update
    /// brought, and the welcome. They glow softly in the accent for their first two seconds.
    fn notices(&mut self, ui: &mut egui::Ui, skip: usize) {
        self.receive_update(ui.ctx());
        let now = ui.input(|input| input.time);
        let mut clicked = None;
        for (index, notice) in self.notices.iter_mut().enumerate().skip(skip) {
            let born = *notice.born.get_or_insert(now);
            let age = (now - born) as f32;
            let glow = (1.0 - age / NOTICE_GLOW).clamp(0.0, 1.0);
            if glow > 0.0 {
                ui.ctx().request_repaint();
            }
            let colour = lerp_colour(theme::current().text, theme::ACCENT, glow);
            let text = egui::RichText::new(&notice.text).color(colour);
            match notice.action {
                NoticeAction::None => {
                    ui.label(text);
                }
                NoticeAction::Update | NoticeAction::Open(_) => {
                    if ui.link(text).clicked() {
                        clicked = Some(index);
                    }
                }
            }
        }
        if let Some(index) = clicked {
            match self.notices[index].action.clone() {
                NoticeAction::Update => self.start_update(ui.ctx()),
                NoticeAction::Open(url) => open_link(&url),
                NoticeAction::None => {}
            }
        }
    }

    /// Takes the answer of the check for updates, and the download, when they come.
    fn receive_update(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.update_check
            && let Ok(found) = receiver.try_recv()
        {
            self.update_check = None;
            if let Some(release) = found {
                let line = self.text.render(&Message::with(
                    "ui.update.available",
                    vec![release.version_text()],
                ));
                // The update line comes first, before the welcome.
                self.notices
                    .insert(0, Notice::new(line, NoticeAction::Update));
                self.update = Some(release);
            }
        }
        if let Some(receiver) = &self.update_download
            && let Ok(result) = receiver.try_recv()
        {
            self.update_download = None;
            let version = self
                .update
                .as_ref()
                .map(updates::Release::version_text)
                .unwrap_or_default();
            match result
                .and_then(|file| updates::install(&file, &version, self.doc.path()).map(|()| file))
            {
                Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => {
                    let line = self
                        .text
                        .render(&Message::with("ui.update.failed", vec![error]));
                    self.notices
                        .insert(0, Notice::new(line, NoticeAction::None));
                }
            }
        }
    }

    /// Downloads the update in the background, saving the model first if it has a file.
    fn start_update(&mut self, ctx: &egui::Context) {
        let Some(release) = self.update.clone() else {
            return;
        };
        if self.update_download.is_some() {
            return;
        }
        if let Some(path) = self.doc.path().map(std::path::Path::to_path_buf) {
            let _ = self.doc.save(&path);
        }
        let line = self.text.render(&Message::with(
            "ui.update.downloading",
            vec![release.version_text()],
        ));
        self.notices
            .insert(0, Notice::new(line, NoticeAction::None));
        let (sender, receiver) = std::sync::mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(updates::download(&release));
            repaint.request_repaint();
        });
        self.update_download = Some(receiver);
    }

    /// Agent access, and whether an agent is talking to Curva.
    fn agent_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.agent_on,
                self.text.render(&Message::new("ui.agent")),
            );
            let recent = self
                .agent_seen
                .is_some_and(|seen| seen.elapsed() < AGENT_RECENT);
            let (key, colour) = match (self.agent.is_some(), recent) {
                (false, _) => ("ui.agent.off", theme::current().quiet),
                (true, false) => ("ui.agent.waiting", theme::current().quiet),
                (true, true) => ("ui.agent.active", AGENT_ACTIVE),
            };
            ui.colored_label(colour, self.text.render(&Message::new(key)));
        });
    }

    /// The AI Assistant panel: the choice of model service until one is connected, then the chat.
    fn assistant_panel(&mut self, ui: &mut egui::Ui) {
        self.receive_assistant();
        if self.settings.assistant.connected {
            self.assistant_chat(ui);
        } else {
            self.assistant_connections(ui);
        }
        ui.separator();
        self.agent_status(ui);
    }

    /// The ways to connect a model, as Serpentine3D offers them, and how to connect an outside
    /// assistant through MCP.
    fn assistant_connections(&mut self, ui: &mut egui::Ui) {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(say("ui.assistant.connect"));
        ui.horizontal_wrapped(|ui| {
            for (provider, key) in [
                (assistant::Provider::ChatGpt, "ui.assistant.chatgpt"),
                (assistant::Provider::LmStudio, "ui.assistant.local"),
                (assistant::Provider::OpenAi, "ui.assistant.key"),
            ] {
                let chosen = self.assistant_choice == Some(provider)
                    || (provider == assistant::Provider::OpenAi
                        && self.assistant_choice == Some(assistant::Provider::Claude));
                if ui.selectable_label(chosen, say(key)).clicked() {
                    self.assistant_choice = Some(provider);
                    self.assistant_error = None;
                    if provider == assistant::Provider::LmStudio {
                        match assistant::local_models() {
                            Ok(models) => self.assistant_models = models,
                            Err(error) => self.assistant_error = Some(error),
                        }
                    }
                }
            }
        });
        let mut connect = None;
        match self.assistant_choice {
            Some(assistant::Provider::ChatGpt) => {
                ui.label(say("ui.assistant.chatgpt_note"));
                ui.horizontal(|ui| {
                    if ui.button(say("ui.assistant.sign_in")).clicked()
                        && let Err(error) = assistant::sign_in()
                    {
                        self.assistant_error = Some(error);
                    }
                    if ui.button(say("ui.assistant.use")).clicked() {
                        connect = Some((
                            assistant::Provider::ChatGpt,
                            assistant::Provider::ChatGpt.standard_model().to_owned(),
                        ));
                    }
                });
            }
            Some(assistant::Provider::LmStudio) => {
                if self.assistant_models.is_empty() {
                    ui.label(say("ui.assistant.no_models"));
                }
                for model in self.assistant_models.clone() {
                    if ui.button(&model).clicked() {
                        connect = Some((assistant::Provider::LmStudio, model));
                    }
                }
            }
            Some(provider @ (assistant::Provider::OpenAi | assistant::Provider::Claude)) => {
                ui.horizontal(|ui| {
                    let mut choice = provider;
                    ui.radio_value(&mut choice, assistant::Provider::OpenAi, "OpenAI");
                    ui.radio_value(&mut choice, assistant::Provider::Claude, "Claude");
                    self.assistant_choice = Some(choice);
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.assistant_key)
                        .password(true)
                        .hint_text(say("ui.assistant.key_hint")),
                );
                if ui.button(say("ui.assistant.use")).clicked() {
                    let provider = self.assistant_choice.unwrap_or(provider);
                    if !self.assistant_key.trim().is_empty() {
                        if let Err(error) = assistant::set_key(provider, self.assistant_key.trim())
                        {
                            self.assistant_error = Some(error);
                        }
                        self.assistant_key.clear();
                    }
                    connect = Some((provider, provider.standard_model().to_owned()));
                }
            }
            None => {}
        }
        if let Some(error) = &self.assistant_error {
            ui.colored_label(theme::ACCENT, error);
        }
        // An outside assistant, through MCP.
        egui::CollapsingHeader::new(say("ui.assistant.mcp")).show(ui, |ui| {
            ui.label(say("ui.assistant.mcp_steps"));
            let mut settings = assistant::mcp_settings();
            ui.add(
                egui::TextEdit::multiline(&mut settings)
                    .code_editor()
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
            if ui.button(say("ui.assistant.copy")).clicked() {
                ui.ctx().copy_text(assistant::mcp_settings());
            }
        });
        if let Some((provider, model)) = connect {
            self.settings.assistant = assistant::Connection {
                provider,
                model,
                connected: true,
            };
            self.save_settings();
        }
    }

    /// The conversation, and the box to ask in.
    fn assistant_chat(&mut self, ui: &mut egui::Ui) {
        let connection = self.settings.assistant.clone();
        ui.horizontal(|ui| {
            ui.colored_label(
                theme::ACCENT,
                format!("{} · {}", connection.provider.label(), connection.model),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("⟲")
                    .on_hover_text(self.text.render(&Message::new("ui.assistant.change")))
                    .clicked()
                {
                    self.settings.assistant.connected = false;
                    self.save_settings();
                }
                if ui
                    .small_button("+")
                    .on_hover_text(self.text.render(&Message::new("ui.assistant.new")))
                    .clicked()
                {
                    self.assistant_lines.clear();
                    self.assistant_history.clear();
                }
            });
        });
        let height = (ui.available_height() - 64.0).max(60.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.assistant_lines {
                    match line {
                        assistant::Line::User(text) => {
                            ui.label(egui::RichText::new(text).strong());
                        }
                        assistant::Line::Assistant(text) => {
                            ui.label(text);
                        }
                        assistant::Line::Tool(name) => {
                            ui.colored_label(theme::current().quiet, format!("· {name}"));
                        }
                        assistant::Line::Error(text) => {
                            ui.colored_label(theme::ACCENT, text);
                        }
                    }
                }
                if self.assistant_busy.is_some() {
                    ui.colored_label(
                        theme::current().quiet,
                        self.text.render(&Message::new("ui.assistant.thinking")),
                    );
                }
            });
        let mut send = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.assistant_input)
                    .hint_text(self.text.render(&Message::new("ui.assistant.ask")))
                    .desired_width(ui.available_width() - 60.0),
            );
            send |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            send |= ui
                .add_enabled(
                    self.assistant_busy.is_none(),
                    egui::Button::new(self.text.render(&Message::new("ui.assistant.send"))),
                )
                .clicked();
        });
        if send && self.assistant_busy.is_none() && !self.assistant_input.trim().is_empty() {
            let request = std::mem::take(&mut self.assistant_input);
            self.assistant_lines
                .push(assistant::Line::User(request.clone()));
            // The assistant works on the model through Agent access.
            self.agent_on = true;
            let history = self.assistant_history.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let repaint = ui.ctx().clone();
            std::thread::spawn(move || {
                let lines = assistant::answer(&connection, &history, &request);
                let _ = sender.send((request, lines));
                repaint.request_repaint();
            });
            self.assistant_busy = Some(receiver);
        }
    }

    /// Takes the assistant's answer when it comes.
    fn receive_assistant(&mut self) {
        if let Some(receiver) = &self.assistant_busy
            && let Ok((request, lines)) = receiver.try_recv()
        {
            self.assistant_busy = None;
            let reply = lines
                .iter()
                .filter_map(|line| match line {
                    assistant::Line::Assistant(text) => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.assistant_history.push((request, reply));
            self.assistant_lines.extend(lines);
        }
    }

    /// Keeps the settings for the next session.
    fn save_settings(&self) {
        if self.benchmark.is_none()
            && let Some(file) = theme::file()
        {
            let _ = self.settings.save(&file);
        }
    }

    /// The surfaces and solids the running command would make, as ghosted triangles. The meshes
    /// are kept while the previewed bodies stay the same, so a still preview costs nothing.
    fn ghosted_preview(&mut self, cursor: Option<Point>) -> Arc<Vec<MeshVertex>> {
        let bodies = self
            .processor
            .preview_bodies(&self.doc, cursor, &self.typed);
        if bodies != self.ghosted.0 {
            let meshes: Vec<curva_kernel::BodyMesh> =
                bodies.iter().filter_map(|body| body.mesh().ok()).collect();
            self.ghosted = (bodies, Arc::new(ghost_faces(&meshes)));
        }
        Arc::clone(&self.ghosted.1)
    }

    /// Offers back the model a Curva that did not close normally left behind.
    fn recovery_window(&mut self, ui: &mut egui::Ui) {
        let Some(file) = self.left_behind.clone() else {
            return;
        };
        let (mut restore, mut discard) = (false, false);
        egui::Window::new(self.text.render(&Message::new("ui.recovery")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(self.text.render(&Message::new("ui.recovery.question")));
                ui.horizontal(|ui| {
                    restore = ui
                        .button(self.text.render(&Message::new("ui.recovery.restore")))
                        .clicked();
                    discard = ui
                        .button(self.text.render(&Message::new("ui.recovery.discard")))
                        .clicked();
                });
            });
        if restore {
            self.left_behind = None;
            match recovery::restore(&file) {
                Ok(doc) => {
                    self.doc = doc;
                    self.drawn = None;
                    let line = self.text.render(&Message::new("done.recovered"));
                    self.processor.note(line);
                }
                Err(error) => {
                    let line = self
                        .text
                        .render(&Message::with("error.recovery", vec![error]));
                    self.processor.note(line);
                }
            }
        } else if discard {
            self.left_behind = None;
            recovery::discard(&file);
        }
    }

    /// Tells the document which plane flat objects go on: the last-used viewport's.
    fn share_work_plane(&mut self) {
        let plane = self.panes[self.active].plane;
        self.doc.set_work_plane([plane.x, plane.y]);
        self.doc.set_work_origin(plane.origin);
    }

    /// Carries out the changes of view commands asked for, oldest first.
    fn apply_view_request(&mut self) {
        while let Some(request) = self.processor.take_view_request() {
            self.apply_view(request);
        }
    }

    /// Carries out one change of view.
    fn apply_view(&mut self, request: ViewRequest) {
        match request {
            ViewRequest::UndoView => self.panes[self.active].step_view(false),
            ViewRequest::RedoView => self.panes[self.active].step_view(true),
            ViewRequest::ZoomBy(factor) => {
                self.panes[self.active].camera.zoom(1.0 / factor);
            }
            ViewRequest::ZoomWindow(a, b) => {
                let low = std::array::from_fn(|axis| a[axis].min(b[axis]));
                let high = std::array::from_fn(|axis| a[axis].max(b[axis]));
                self.panes[self.active].camera.frame(low, high);
            }
            ViewRequest::ZoomTarget(at) => {
                self.panes[self.active].camera.target = at;
            }
            ViewRequest::SaveView(name) => {
                let pane = &self.panes[self.active];
                let view = pane.camera.saved(name.clone(), pane.view);
                self.doc.save_view(view);
                let said = self
                    .text
                    .render(&Message::with("done.namedview.save", vec![name]));
                self.processor.note(said);
            }
            ViewRequest::RestoreView(name) => {
                if let Some(saved) = self.doc.saved_view(&name).cloned() {
                    let pane = &mut self.panes[self.active];
                    if let Some(kind) = StandardView::from_key(&saved.kind)
                        && kind != pane.view
                    {
                        *pane = Pane::new(kind);
                    }
                    pane.camera = Camera::from_saved(&saved);
                    pane.named = Some(saved.name);
                }
            }
            ViewRequest::ZoomExtents => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds() {
                    for pane in &mut self.panes {
                        pane.camera.frame(low, high);
                    }
                }
            }
            ViewRequest::ZoomSelected => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds_of(&self.doc.selection()) {
                    for pane in &mut self.panes {
                        pane.camera.frame(low, high);
                    }
                }
            }
            ViewRequest::FourViews => {
                self.panes = standard_panes();
                self.maximised = None;
            }
            ViewRequest::SetCPlane { origin, x, y } => {
                let normal = curva_kernel::cross(x, y);
                self.panes[self.active].set_plane(CPlane {
                    origin,
                    x,
                    y,
                    normal,
                });
            }
            ViewRequest::ResetCPlane => {
                let pane = &mut self.panes[self.active];
                pane.set_plane(pane.view.plane());
            }
            ViewRequest::Show(name) => {
                let display = self.panes[self.active].display;
                self.panes[self.active] = Pane::new(standard_view(name));
                self.panes[self.active].display = display;
            }
            ViewRequest::Plan => {
                let pane = &mut self.panes[self.active];
                let plane = pane.plane;
                pane.camera.plan(&plane);
            }
            ViewRequest::Display(mode) => self.panes[self.active].display = mode,
            ViewRequest::ShowSelectionFilter => self.filter_row = !self.filter_row,
            ViewRequest::ToggleSelectionFilter => self.pick_filter.on = !self.pick_filter.on,
        }
    }

    fn handle_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) {
        // Not `drag_delta`: egui holds that back for the first 6 points to tell a click from a
        // drag, and only the left button clicks here. The view must move from the first pixel.
        let (delta, shift, ctrl, middle, right, pressed) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.modifiers.shift,
                input.modifiers.command,
                input.pointer.middle_down(),
                input.pointer.secondary_down(),
                input.pointer.press_origin(),
            )
        });
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        if response.is_pointer_button_down_on() {
            self.active = pane;
            // Ctrl + right-drag zooms about where the drag began; dragging up comes closer.
            let plane = self.panes[pane].plane;
            let origin = response.rect.min;
            let about = pressed.and_then(|at| {
                let offset = at - origin;
                let screen = [f64::from(offset.x), f64::from(offset.y)];
                self.panes[pane].camera.plane_point(screen, size, &plane)
            });
            let camera = &mut self.panes[pane].camera;
            if right && ctrl {
                if let Some(about) = about {
                    camera.zoom_about((dy * ZOOM_DRAG_SPEED).exp(), about);
                } else {
                    camera.zoom((dy * ZOOM_DRAG_SPEED).exp());
                }
            } else if middle || (right && (shift || camera.is_parallel())) {
                camera.pan(dx, dy, size[1]);
            } else if right {
                camera.orbit(dx, dy);
            }
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|input| input.smooth_scroll_delta.y));
            if scroll != 0.0 {
                self.panes[pane].camera.zoom((-scroll * 0.002).exp());
            }
        }
        let in_title = |position: egui::Pos2| position.y < response.rect.top() + TITLE_HEIGHT;
        if response.double_clicked() && response.interact_pointer_pos().is_some_and(in_title) {
            self.maximised = if self.maximised.is_some() {
                None
            } else {
                Some(pane)
            };
        }
        let events = ui.input(|input| input.events.clone());
        let on_viewport = |position: egui::Pos2| {
            response.rect.contains(position)
                && !in_title(position)
                && ui.ctx().layer_id_at(position) == Some(response.layer_id)
        };
        if self.gumball_mouse(ui, pane, response, size, &events) {
            return;
        }
        if self.sketch_mouse(ui, pane, response, size) {
            return;
        }
        let gestures = self.panes[pane].gestures.take(&events, on_viewport);
        let remove = ui.input(|input| input.modifiers.command);
        let origin = response.rect.min;
        let local = |position: egui::Pos2| {
            let offset = position - origin;
            [f64::from(offset.x), f64::from(offset.y)]
        };
        // Checked once, so the click that ends a command does not also change the selection.
        if self.takes_position() {
            for (pressed, _) in gestures {
                if !self.takes_position() {
                    break;
                }
                // Where it was pressed, as that is where the user aimed.
                if let Some((at, _)) = self.aim(ui, pane, local(pressed), size) {
                    self.active = pane;
                    self.share_work_plane();
                    self.processor.click(&mut self.doc, at);
                }
            }
            return;
        }
        let camera = self.panes[pane].camera;
        let wants = |kind: Kind| {
            self.processor
                .current_request()
                .is_some_and(|request| request.kind == kind)
        };
        let (wants_edges, wants_faces) = (wants(Kind::Edges), wants(Kind::Faces));
        for (from, to) in gestures {
            let boxed = is_box_drag(from, to);
            if wants_faces {
                // A command is asking for faces, so clicks and boxes pick faces, not objects.
                let hits = if boxed {
                    self.outlines
                        .faces_in_box(&camera, local(from), local(to), size)
                } else {
                    self.outlines
                        .pick_face(&camera, local(from), size)
                        .into_iter()
                        .collect()
                };
                let picked = combined_selection(self.doc.face_selection(), hits, shift, remove);
                self.doc.select_faces(&picked);
                continue;
            }
            if wants_edges {
                // A command is asking for edges, so clicks and boxes pick edges, not objects.
                let hits = if boxed {
                    self.outlines
                        .edges_in_box(&camera, local(from), local(to), size)
                } else {
                    self.outlines
                        .pick_edge(&camera, local(from), size, PICK_REACH)
                        .into_iter()
                        .collect()
                };
                let picked = combined_selection(self.doc.edge_selection(), hits, shift, remove);
                self.doc.select_edges(&picked);
                continue;
            }
            let grips = self.grips_hit(&camera, (local(from), local(to)), boxed, size);
            if !grips.is_empty() {
                // A control point was hit, so the points picked change and the objects do not.
                let picked = combined_selection(self.doc.grip_selection(), grips, shift, remove);
                self.doc.select_grips(&picked);
                continue;
            }
            if !shift && !remove {
                self.doc.select_grips(&[]);
            }
            let hits = if boxed {
                self.outlines.in_box(&camera, local(from), local(to), size)
            } else {
                let hit = self.outlines.pick(&camera, local(from), size, PICK_REACH);
                hit.into_iter().collect()
            };
            // Picking one member of a group picks the whole group, as in Rhino.
            let hits = self.doc.with_groups(&hits);
            let chosen = combined_selection(self.doc.selection(), hits, shift, remove);
            self.doc.select(&chosen);
        }
    }

    fn paint_snap_marker(
        &self,
        ui: &egui::Ui,
        pane: usize,
        viewport: egui::Rect,
        size: [f64; 2],
        hit: SnapHit,
    ) {
        let Some(at) = self.panes[pane].camera.project(hit.point, size) else {
            return;
        };
        let centre = viewport.min + egui::vec2(at[0] as f32, at[1] as f32);
        let painter = ui.painter_at(viewport);
        // ArchiCAD's symbols: a cross for an intersection, a triangle for a midpoint, a circle
        // for a centre, a square for an end and the rest.
        let stroke = egui::Stroke::new(2.0, theme::ACCENT);
        let half = SNAP_MARKER_SIZE / 2.0;
        match hit.kind {
            SnapKind::Intersection => {
                for (a, b) in [(-1.0, -1.0), (-1.0, 1.0)] {
                    let offset = egui::vec2(a * half, b * half);
                    painter.line_segment([centre + offset, centre - offset], stroke);
                }
            }
            SnapKind::Mid => {
                let corners = vec![
                    centre + egui::vec2(0.0, -half),
                    centre + egui::vec2(half, half * 0.8),
                    centre + egui::vec2(-half, half * 0.8),
                ];
                painter.add(egui::Shape::closed_line(corners, stroke));
            }
            SnapKind::Center => {
                painter.circle_stroke(centre, half, stroke);
            }
            _ => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(centre, egui::Vec2::splat(SNAP_MARKER_SIZE)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        painter.text(
            centre + egui::vec2(SNAP_MARKER_SIZE, -SNAP_MARKER_SIZE),
            egui::Align2::LEFT_BOTTOM,
            self.text.render(&Message::new(snap_label(hit.kind))),
            egui::TextStyle::Body.resolve(ui.style()),
            theme::ACCENT,
        );
    }

    /// One viewport's title, and an outline if it is the one view commands act on.
    fn paint_frame(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.text(
            rect.left_top() + egui::vec2(6.0, 3.0),
            egui::Align2::LEFT_TOP,
            self.panes[pane].named.clone().unwrap_or_else(|| {
                self.text
                    .render(&Message::new(view_label(self.panes[pane].view)))
            }),
            egui::TextStyle::Body.resolve(ui.style()),
            theme::current().quiet,
        );
        if pane == self.active && self.maximised.is_none() {
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// Clicking a viewport's title opens a menu of display modes, as in Rhino.
    fn display_menu(&mut self, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
        let title = egui::Rect::from_min_size(rect.min, TITLE_SIZE);
        let response = ui.interact(
            title,
            ui.scope_id().with(("display menu", pane)),
            egui::Sense::click(),
        );
        let mut chosen = None;
        egui::Popup::menu(&response).show(|ui| {
            for (mode, key) in [
                (DisplayMode::Wireframe, "ui.display.wireframe"),
                (DisplayMode::Shaded, "ui.display.shaded"),
                (DisplayMode::Ghosted, "ui.display.ghosted"),
            ] {
                let current = self.panes[pane].display == mode;
                if ui
                    .selectable_label(current, self.text.render(&Message::new(key)))
                    .clicked()
                {
                    chosen = Some(mode);
                }
            }
        });
        if let Some(mode) = chosen {
            self.panes[pane].display = mode;
            self.active = pane;
        }
    }

    fn viewports(&mut self, ui: &mut egui::Ui, area: egui::Rect, reach: egui::Rect) {
        // Picked edges and faces only mean something while a command asks for them.
        let wants = |kind: Kind| {
            self.processor
                .current_request()
                .is_some_and(|request| request.kind == kind)
        };
        if !wants(Kind::Edges) && !self.doc.edge_selection().is_empty() {
            self.doc.select_edges(&[]);
        }
        if !wants(Kind::Faces) && !self.doc.face_selection().is_empty() {
            self.doc.select_faces(&[]);
        }
        let mut shown = Vec::new();
        for (pane, rect) in pane_rects(area, self.maximised) {
            let rect = reaching(rect, area, reach);
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            self.display_menu(ui, pane, rect);
            self.handle_mouse(ui, pane, &response, pane_size(rect));
            shown.push((pane, rect, response));
        }
        self.refresh_scene();
        let hovered = shown
            .iter()
            .find_map(|(pane, rect, response)| response.hover_pos().map(|at| (*pane, *rect, at)));
        // While the view is being turned or slid, the aim holds where it was, so a height or a
        // preview does not jump with the view.
        let navigating =
            ui.input(|input| input.pointer.secondary_down() || input.pointer.middle_down());
        let held = self.held_aim;
        let aimed = hovered.and_then(|(pane, rect, position)| {
            let local = position - rect.min;
            let screen = [f64::from(local.x), f64::from(local.y)];
            let size = pane_size(rect);
            if self.takes_position() {
                self.aim(ui, pane, screen, size)
            } else {
                let plane = self.panes[pane].plane;
                let camera = &self.panes[pane].camera;
                camera
                    .plane_point(screen, size, &plane)
                    .map(|at| (at, None))
            }
        });
        let aimed = if navigating { held } else { aimed };
        self.held_aim = aimed;
        let cursor = aimed.map(|(at, _)| at);
        self.last_aim = cursor;
        self.rest_on(ui.ctx(), aimed.and_then(|(_, hit)| hit));
        // Picking a point shows a crosshair, and what the step measures beside it.
        if let Some((.., position)) = hovered
            && self.takes_position()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            // A rectangle's far corner shows width × depth.
            let unit = self.doc.state().unit.symbol();
            let corner = cursor.and_then(|at| self.processor.corner_size(&self.doc, at));
            if let Some((wide, deep)) = corner {
                paint_cursor_tag(
                    ui,
                    position,
                    &format!("{wide:.3} {unit} × {deep:.3} {unit}"),
                );
            }
            // A number being typed for a size shows in the tag, as it drives the command.
            let typed = self.typed.trim().parse::<f64>().ok();
            if corner.is_none()
                && let Some(value) =
                    typed.or_else(|| cursor.and_then(|at| self.processor.measure(&self.doc, at)))
            {
                let unit = self.doc.state().unit.symbol();
                paint_cursor_tag(ui, position, &format!("{value:.3} {unit}"));
            }
        }
        // Picking objects, edges or faces shows the pointer with a small box beside it.
        let picking = self.processor.current_request().is_some_and(|request| {
            matches!(request.kind, Kind::Objects | Kind::Edges | Kind::Faces)
        });
        if let Some((.., position)) = hovered
            && picking
        {
            let corner = position + egui::vec2(10.0, 10.0);
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(corner, egui::Vec2::splat(8.0)),
                1.0,
                egui::Stroke::new(1.5, theme::current().text),
                egui::StrokeKind::Middle,
            );
        }
        let mut previewed = self.processor.preview(&self.doc, cursor, &self.typed);
        previewed.extend(self.gumball_preview());
        let preview = ghost_lines(&previewed);
        let overlay = Arc::new(preview);
        let preview_faces = self.ghosted_preview(cursor);
        // A solid the preview changes in place is hidden, so the changed one shows in its place.
        let replaced = self
            .processor
            .preview_replaces(&self.doc, cursor, &self.typed);
        let faces = if replaced.is_empty() {
            Arc::clone(&self.faces)
        } else {
            if self.faces_without.0 != replaced {
                let selection = self.doc.selection();
                let made =
                    curva_viewport::face_vertices_without(&self.outlines, &selection, &replaced);
                self.faces_without = (replaced, Arc::new(made));
            }
            Arc::clone(&self.faces_without.1)
        };
        for (pane, rect, _) in &shown {
            let size = pane_size(*rect);
            let camera = &self.panes[*pane].camera;
            let display = self.panes[*pane].display;
            let drawing = Drawing {
                slot: *pane,
                grid: if self.settings.grid {
                    Arc::clone(&self.panes[*pane].grid)
                } else {
                    Arc::default()
                },
                // Dark edges suit solid faces; ghosted views keep the light wireframe edges.
                scene: Arc::clone(if display == DisplayMode::Shaded {
                    &self.shaded_scene
                } else {
                    &self.scene
                }),
                faces: Arc::clone(&faces),
                overlay: Arc::clone(&overlay),
                preview_faces: Arc::clone(&preview_faces),
                matrix: camera.matrix(size[0] / size[1]),
                light: camera.forward().map(|value| value as f32),
                shaded: display != DisplayMode::Wireframe,
                ghosted: display == DisplayMode::Ghosted,
                area: *rect,
                frame: ui.ctx().cumulative_frame_nr(),
                background: theme::current().window,
                background_low: theme::current().window_low,
            };
            ui.painter().add(egui_wgpu_callback(*rect, drawing));
            self.paint_frame(ui, *pane, *rect);
            self.paint_points(ui, *pane, *rect, size, &previewed);
            self.paint_grips(ui, *pane, *rect, size);
            self.paint_picked_edges(ui, *pane, *rect, size);
            self.paint_picked_faces(ui, *pane, *rect, size);
            self.paint_gumball(ui, *pane, *rect, size);
            self.paint_guides(ui, *pane, *rect, size);
            if let Some((hover_pane, ..)) = hovered
                && hover_pane == *pane
                && let Some(hit) = aimed.and_then(|(_, hit)| hit)
            {
                self.paint_snap_marker(ui, *pane, *rect, size, hit);
            }
            let dragging = self.panes[*pane]
                .gestures
                .pressed_at
                .zip(ui.input(|input| input.pointer.latest_pos()))
                .filter(|(from, to)| !self.takes_position() && is_box_drag(*from, *to));
            if let Some((from, to)) = dragging {
                paint_selection_box(ui, *rect, from, to);
            }
        }
    }
}

/// Curva's icon, for the window's title bar and the task bar.
const ICON_PNG: &[u8] = include_bytes!("../../../packaging/icons/curva-256.png");

/// The icon as the window wants it, or `None` if it cannot be read, in which case the system's
/// default icon shows.
fn window_icon() -> Option<egui::IconData> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgba).ok()?;
    (info.color_type == png::ColorType::Rgba).then(|| egui::IconData {
        rgba: rgba[..info.buffer_size()].to_vec(),
        width: info.width,
        height: info.height,
    })
}

fn egui_wgpu_callback(rect: egui::Rect, drawing: Drawing) -> egui::Shape {
    eframe::egui_wgpu::Callback::new_paint_callback(rect, drawing).into()
}

impl eframe::App for Curva {
    /// A normal close: the recovery copy is no longer needed.
    fn on_exit(&mut self) {
        if let Some(autosave) = &self.autosave {
            autosave.finish();
        }
        // The arrangement is only a convenience; failing to keep it is not worth stopping for.
        if self.benchmark.is_none()
            && let Some(file) = layout::file()
        {
            let _ = self.layout.save(&file);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.apply_theme(ui.ctx());
        // Keys go to whichever field has focus; the command line takes them when no other does.
        let focused = ui.ctx().memory(|memory| memory.focused());
        self.naming_layer = focused.is_some() && focused != self.command_field;
        // Before the panels, so its Enter is not also taken by the command line.
        self.gumball_value_box(ui.ctx());
        self.share_work_plane();
        // The panels float over the viewports, which take the area they leave free.
        let (free, reach) = self.panels(ui.ctx(), ui.max_rect());
        self.keep_agent_link(ui.ctx());
        self.serve_agents();
        self.approval_window(ui);
        self.recovery_window(ui);
        self.unit_question(ui.ctx());
        self.close_question(ui.ctx());
        if let Some(autosave) = &mut self.autosave {
            autosave.tick(&self.doc, std::time::Instant::now());
        }
        self.apply_view_request();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::current().window))
            .show(ui, |ui| self.viewports(ui, free, reach));
        if let Some(benchmark) = &mut self.benchmark {
            self.panes[PERSPECTIVE].camera.orbit(BENCHMARK_DRAG, 0.0);
            // Snapping is measured with every snap on, the slowest case.
            let every_snap = SnapSet {
                near: true,
                ..self.snaps
            };
            let started = std::time::Instant::now();
            let _ = self.outlines.snap(
                &self.panes[PERSPECTIVE].camera,
                [400.0, 300.0],
                [800.0, 600.0],
                &every_snap,
                Some([0.0; 3]),
            );
            benchmark
                .snapping
                .push(started.elapsed().as_secs_f64() * 1000.0);
            if benchmark.record(ui, frame) {
                benchmark.report(frame);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                self.benchmark = None;
            }
        }
    }
}

/// What a picture of the model shows: everything, shaded or `ghosted`, seen by `camera`.
fn picture_of(
    doc: &Document,
    outlines: &Outlines,
    camera: &Camera,
    aspect: f64,
    ghosted: bool,
) -> Picture {
    let selection = doc.selection();
    let faces = face_vertices(outlines, &selection);
    Picture {
        lines: if faces.is_empty() || ghosted {
            scene_lines(outlines, &selection)
        } else {
            shaded_scene_lines(outlines, &selection)
        },
        faces,
        matrix: camera.matrix(aspect),
        light: camera.forward().map(|value| value as f32),
        ghosted,
    }
}

/// A picture of the model for an MCP agent, as the bytes of a PNG file.
fn mcp_picture(doc: &Document) -> Result<Vec<u8>, String> {
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    picture_from(doc, &camera, false)
}

/// A picture of the model seen by `camera`, as the bytes of a PNG file.
fn picture_from(doc: &Document, camera: &Camera, ghosted: bool) -> Result<Vec<u8>, String> {
    let outlines = Outlines::of(doc.state());
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    let path = std::env::temp_dir().join(format!("curva-mcp-{}.png", std::process::id()));
    save_picture(
        picture_of(doc, &outlines, camera, aspect, ghosted),
        PICTURE_SIZE,
        &path,
    )?;
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    // The picture has been read; a leftover temporary file would do no harm.
    let _ = std::fs::remove_file(&path);
    Ok(bytes)
}

/// Runs semicolon-separated command lines and saves a picture of the result.
fn picture(path: &str, script: &str) -> Result<(), String> {
    let mut doc = Document::default();
    let mut processor = Processor::default();
    // A `SetDisplayMode Ghosted` in the script makes the picture ghosted.
    let mut ghosted = false;
    for line in script.split(';') {
        processor.submit(&mut doc, line);
        while let Some(request) = processor.take_view_request() {
            if let ViewRequest::Display(mode) = request {
                ghosted = mode == DisplayMode::Ghosted;
            }
        }
    }
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    save_picture(
        picture_of(&doc, &outlines, &camera, aspect, ghosted),
        PICTURE_SIZE,
        std::path::Path::new(path),
    )?;
    for line in processor.log() {
        println!("{line}");
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let result = run();
    if let Err(error) = &result {
        // Without a console the error would be lost, so it is kept where a user can find it.
        if let Some(folder) = recovery::app_folder() {
            let _ = std::fs::create_dir_all(&folder);
            let _ = std::fs::write(
                folder.join("curva.log"),
                format!("Curva could not run: {error}\n"),
            );
        }
    }
    result
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        arguments
            .iter()
            .position(|argument| argument == flag)
            .and_then(|index| arguments.get(index + 1))
    };
    if arguments.iter().any(|argument| argument == "--mcp") {
        if arguments.iter().any(|argument| argument == "--connect") {
            return curva_mcp::bridge_to_window();
        }
        let mut server = curva_mcp::Server::new(mcp_picture);
        return server
            .serve(std::io::stdin().lock(), std::io::stdout().lock())
            .map_err(|error| error.to_string());
    }
    if let Some(path) = value("--picture") {
        let script = value("--run").map_or("", String::as_str);
        return picture(path, script);
    }
    // A model given on the command line opens, as after an update.
    let file = arguments
        .iter()
        .find(|argument| argument.to_lowercase().ends_with(".curva"))
        .map(std::path::PathBuf::from);
    let benchmark = arguments
        .iter()
        .any(|argument| argument == "--orbit-benchmark")
        .then(|| {
            let count = |flag: &str| value(flag).and_then(|n| n.parse().ok()).unwrap_or(0);
            BenchmarkScene {
                curves: count("--curves"),
                solids: count("--solids"),
            }
        });
    eframe::run_native(
        "Curva",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            // Waiting for the display adds a frame or more of lag: over Remote Desktop the display runs
            // at 32 Hz, and measured frames took 31 ms this way against 2 ms without waiting.
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                surface: eframe::egui_wgpu::SurfaceConfig {
                    present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
                    desired_maximum_frame_latency: Some(1),
                },
                ..eframe::egui_wgpu::WgpuConfiguration::default()
            },
            // Shaded views need depth, so nearer faces hide farther ones.
            depth_buffer: DEPTH_BITS,
            viewport: match window_icon() {
                Some(icon) => egui::ViewportBuilder::default()
                    .with_icon(icon)
                    .with_maximized(true),
                None => egui::ViewportBuilder::default().with_maximized(true),
            },
            ..eframe::NativeOptions::default()
        },
        Box::new(move |context| {
            let app = Curva::new(context, benchmark).opened(&context.egui_ctx, file.as_deref());
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())
}

/// What the gumball's extrude circle makes of one object: a curve becomes a surface (a closed
/// one a solid), a surface is pulled straight along the axis into a solid, as ExtrudeSrf does;
/// a solid makes nothing.
fn gumball_extruded(
    shape: &Shape,
    axis: Point,
    distance: f64,
) -> Option<Result<curva_kernel::Body, curva_kernel::KernelError>> {
    match shape {
        Shape::Curve(curve) => {
            let closed = curve.is_closed().unwrap_or(false);
            Some(curva_kernel::Body::extruded(
                std::slice::from_ref(curve),
                axis,
                distance,
                closed,
            ))
        }
        Shape::Body(body) if !body.is_solid() => Some(body.extruded_straight(axis, distance)),
        Shape::Body(_) => None,
    }
}

/// What decides where a cursor position lands for the point a command asks for.
struct Aiming<'a> {
    outlines: &'a Outlines,
    request: &'a curva_commands::Request,
    camera: &'a Camera,
    plane: &'a CPlane,
    display: DisplayMode,
    snaps: SnapSet,
    guides: curva_viewport::Guides,
    ortho: bool,
    grid_snap: bool,
    /// Alt held: no object snaps for the moment.
    snaps_off: bool,
    screen: [f64; 2],
    size: [f64; 2],
}

/// Where a cursor position lands for the point being asked for: an object snap if one is in reach
/// (in a shaded view only one that can be seen), then the held points' guidelines, then a height
/// along its line, otherwise the viewport's plane with ortho and grid snap applied.
fn aim_at(aiming: &Aiming<'_>) -> Option<(Point, Option<SnapHit>)> {
    let Aiming {
        outlines,
        request,
        camera,
        plane,
        screen,
        size,
        ..
    } = *aiming;
    if request.kind == Kind::PickPart {
        let on = outlines.pick_point(camera, screen, size, PICK_REACH)?;
        return Some((on, None));
    }
    let base = request.preview.last().copied().or(request.from);
    if !aiming.snaps_off {
        let set = SnapSet {
            // Only a shaded view hides what is behind faces; ghosted and wireframe show it all.
            visible_only: aiming.display == DisplayMode::Shaded,
            ..aiming.snaps
        };
        if let Some(hit) =
            outlines.snap_while_drawing(camera, screen, size, &set, base, &request.preview)
        {
            return Some((hit.point, Some(hit)));
        }
        if let Some(hit) = outlines.guide_snap(camera, screen, size, &aiming.guides) {
            return Some((hit.point, Some(hit)));
        }
    }
    // A height or extrusion follows the pointer along its line, as in Rhino.
    if let Some((origin, direction)) = request.along {
        return camera
            .line_point(screen, size, origin, direction)
            .map(|at| (at, None));
    }
    let cursor = camera.plane_point(screen, size, plane)?;
    Some((
        constrained(plane, cursor, base, aiming.ortho, aiming.grid_snap),
        None,
    ))
}

/// The Settings panel's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsTab {
    General,
    Interface,
    Assistant,
    About,
}

/// A small question in the centre of the window, drawn like a panel.
fn centred_dialog(ctx: &egui::Context, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(egui::Id::NULL.with(("dialog", id)))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            panel_frame(false).show(ui, |ui| {
                ui.set_max_width(420.0);
                contents(ui);
            });
        });
}

/// The frame every panel is drawn in: rounded, edged and shadowed, glass or solid; glowing in the
/// accent while another panel dragged over it would merge into it.
fn panel_frame(glowing: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::current().glass)
        .stroke(egui::Stroke::new(
            if glowing { 2.0 } else { 1.0 },
            if glowing {
                theme::ACCENT
            } else {
                theme::current().panel_edge
            },
        ))
        .corner_radius(PANEL_RADIUS)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: if glowing { 24 } else { 16 },
            spread: 0,
            color: if glowing {
                theme::ACCENT.gamma_multiply(0.5)
            } else {
                egui::Color32::from_black_alpha(90)
            },
        })
        .inner_margin(PANEL_MARGIN)
}

/// A V pointing along `towards` (a unit direction on screen), as the hide control draws it.
fn paint_chevron(
    painter: &egui::Painter,
    centre: egui::Pos2,
    towards: egui::Vec2,
    stroke: egui::Stroke,
) {
    let side = egui::vec2(-towards.y, towards.x);
    let tip = centre + towards * 2.5;
    let back = centre - towards * 2.5;
    painter.line_segment([back + side * 4.5, tip], stroke);
    painter.line_segment([tip, back - side * 4.5], stroke);
}

/// Where along a row of tabs a tab let go at `x` lands, as an index among them.
fn tab_slot(tabs: &[egui::Rect], x: f32) -> usize {
    tabs.iter()
        .position(|tab| x < tab.center().x)
        .unwrap_or(tabs.len().saturating_sub(1))
}

/// What clicking one of the command bar's own lines does.
#[derive(Clone, Debug, PartialEq)]
enum NoticeAction {
    /// Nothing; it is only said.
    None,
    /// Download and install the update on offer.
    Update,
    /// Open a page in the browser.
    Open(String),
}

/// One of the command bar's own lines, above the command history.
#[derive(Clone, Debug)]
struct Notice {
    text: String,
    action: NoticeAction,
    /// When it was first shown, for its glow.
    born: Option<f64>,
}

impl Notice {
    fn new(text: String, action: NoticeAction) -> Self {
        Self {
            text,
            action,
            born: None,
        }
    }
}

/// `from` moved towards `to` by `amount` (0 to 1).
fn lerp_colour(from: egui::Color32, to: egui::Color32, amount: f32) -> egui::Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    egui::Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

/// Opens a page in the system's browser.
fn open_link(url: &str) {
    let result = if cfg!(target_os = "windows") {
        quiet_command("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    let _ = result;
}

/// A command button: its icon, with its name on hover, or its name where it has no icon yet.
fn command_button(ui: &mut egui::Ui, name: &str) -> egui::Response {
    if !icons::has(name) {
        return ui.button(name).on_hover_text(name);
    }
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(COMMAND_BUTTON), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme::current().hover);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), name);
    response.on_hover_text(name)
}

/// The edges and corners a panel can be resized from: near one, the pointer becomes a resize
/// arrow and the edge pulses gently in the accent. The handles lie just inside the panel, so a drag
/// from one never reaches the viewport behind. Returns the drag, if one is under way.
fn resize_handles(ui: &egui::Ui, place: &layout::Place, rect: egui::Rect) -> Option<PanelEvent> {
    let movable = layout::Sides::movable(place);
    let side = |left, right, top, bottom| layout::Sides {
        left,
        right,
        top,
        bottom,
    };
    let reach = RESIZE_REACH;
    let (l, r, t, b) = (rect.min.x, rect.max.x, rect.min.y, rect.max.y);
    let zone = |x0: f32, y0: f32, x1: f32, y1: f32| {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    };
    let handles = [
        (
            side(true, false, true, false),
            zone(l, t, l + 2.0 * reach, t + 2.0 * reach),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, false, true),
            zone(r - 2.0 * reach, b - 2.0 * reach, r, b),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, true, false),
            zone(r - 2.0 * reach, t, r, t + 2.0 * reach),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, true),
            zone(l, b - 2.0 * reach, l + 2.0 * reach, b),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, false),
            zone(l, t + 2.0 * reach, l + reach, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, true, false, false),
            zone(r - reach, t + 2.0 * reach, r, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, false, true, false),
            zone(l + 2.0 * reach, t, r - 2.0 * reach, t + reach),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            side(false, false, false, true),
            zone(l + 2.0 * reach, b - reach, r - 2.0 * reach, b),
            egui::CursorIcon::ResizeVertical,
        ),
    ];
    let mut event = None;
    for (sides, area, cursor) in handles {
        // A corner needs both its sides to be movable, an edge its one.
        if sides.within(movable) != sides {
            continue;
        }
        let key = (sides.left, sides.right, sides.top, sides.bottom);
        let response = ui
            .interact(
                area,
                ui.scope_id().with(("resize", key)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.hovered() || response.dragged() {
            // A gentle pulse along the edges being held.
            let time = ui.input(|input| input.time) as f32;
            let strength = 0.55 + 0.45 * (time * 5.0).sin();
            let stroke = egui::Stroke::new(2.0, theme::ACCENT.gamma_multiply(strength));
            let painter = ui.painter();
            if sides.left {
                painter.line_segment([rect.left_top(), rect.left_bottom()], stroke);
            }
            if sides.right {
                painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
            }
            if sides.top {
                painter.line_segment([rect.left_top(), rect.right_top()], stroke);
            }
            if sides.bottom {
                painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
            }
            ui.ctx().request_repaint();
        }
        if response.dragged() {
            event = Some(PanelEvent::Resized(sides, response.drag_delta()));
        }
    }
    event
}

/// A program to run without a console window of its own: on Windows, a console program started
/// from Curva (a GUI program) would otherwise flash one up.
pub(crate) fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW.
        command.creation_flags(0x0800_0000);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Sense, pos2, vec2};

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn anywhere(_: Pos2) -> bool {
        true
    }

    #[test]
    fn a_quick_click_while_moving_is_kept_though_egui_drops_it() {
        let context = egui::Context::default();
        let mut clicks = LeftGestures::default();
        let (start, end) = (pos2(100.0, 100.0), pos2(130.0, 100.0));
        let frames = [
            vec![],
            vec![Event::PointerMoved(start)],
            vec![button(start, true)],
            vec![Event::PointerMoved(end)],
            vec![button(end, false)],
        ];
        let (mut egui_clicked, mut found) = (false, Vec::new());
        for events in frames {
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let output = context.run_ui(input, |ui| {
                let (_, response) =
                    ui.allocate_exact_size(vec2(400.0, 400.0), Sense::click_and_drag());
                egui_clicked |= response.clicked();
                let events = ui.input(|input| input.events.clone());
                found.extend(clicks.take(&events, anywhere));
            });
            output.drop_without_applying_deltas();
        }
        assert!(!egui_clicked, "this is the behaviour that lost the points");
        assert_eq!(found, [(start, end)], "both ends of the press are kept");
    }

    #[test]
    fn several_clicks_in_one_frame_all_count() {
        let (a, b) = (pos2(10.0, 10.0), pos2(50.0, 50.0));
        let events = [
            button(a, true),
            button(a, false),
            button(b, true),
            button(b, false),
        ];
        assert_eq!(
            LeftGestures::default().take(&events, anywhere),
            [(a, a), (b, b)]
        );
    }

    #[test]
    fn a_press_outside_the_viewport_is_not_a_click() {
        let at = pos2(10.0, 10.0);
        let events = [button(at, true), button(at, false)];
        assert!(LeftGestures::default().take(&events, |_| false).is_empty());
    }

    #[test]
    fn a_press_and_release_in_separate_frames_is_one_click() {
        let at = pos2(10.0, 10.0);
        let mut clicks = LeftGestures::default();
        assert!(clicks.take(&[button(at, true)], anywhere).is_empty());
        assert_eq!(clicks.take(&[button(at, false)], anywhere), [(at, at)]);
        assert!(clicks.take(&[button(at, false)], anywhere).is_empty());
    }

    #[test]
    fn with_no_key_a_selection_is_replaced() {
        assert_eq!(combined_selection(vec![1, 2], vec![3], false, false), [3]);
        assert!(combined_selection(vec![1, 2], vec![], false, false).is_empty());
    }

    #[test]
    fn shift_adds_without_repeating() {
        assert_eq!(
            combined_selection(vec![1, 2], vec![2, 3], true, false),
            [1, 2, 3]
        );
    }

    #[test]
    fn ctrl_removes_only_what_was_hit() {
        assert_eq!(
            combined_selection(vec![1, 2, 3], vec![2, 9], false, true),
            [1, 3]
        );
    }

    #[test]
    fn ctrl_on_nothing_keeps_the_selection() {
        assert_eq!(combined_selection(vec![1, 2], vec![], false, true), [1, 2]);
    }

    #[test]
    fn ctrl_wins_over_shift() {
        assert_eq!(combined_selection(vec![1, 2], vec![2], true, true), [1]);
    }

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0))
    }

    #[test]
    fn four_viewports_fill_the_area_without_overlapping() {
        let rects = pane_rects(area(), None);
        assert_eq!(rects.len(), VIEW_SLOTS);
        for (index, (_, one)) in rects.iter().enumerate() {
            assert!(area().contains_rect(*one));
            for (_, other) in &rects[index + 1..] {
                assert!(!one.intersects(*other));
            }
        }
    }

    #[test]
    fn the_window_icon_reads_as_a_square_with_see_through_corners() {
        let icon = window_icon().expect("the icon reads");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // The corner is outside the rounded square; the middle is inside it.
        assert_eq!(icon.rgba[3], 0);
        let middle = (128 * 256 + 128) * 4;
        assert_eq!(icon.rgba[middle + 3], 255);
    }

    #[test]
    fn every_command_button_names_a_command_and_each_tab_has_some() {
        for (tab, names) in COMMAND_TABS {
            assert!(!names.is_empty(), "{tab} is empty");
            for name in names {
                let found = curva_commands::commands::resolve(name).map(|def| def.name);
                assert_eq!(found, Some(*name), "{name} in {tab}");
            }
        }
    }

    #[test]
    fn viewports_are_arranged_as_in_rhino() {
        let panes = standard_panes();
        let rects = pane_rects(area(), None);
        let at = |view: StandardView| {
            let index = panes.iter().position(|pane| pane.view == view).unwrap();
            rects
                .iter()
                .find(|(pane, _)| *pane == index)
                .unwrap()
                .1
                .center()
        };
        let (top, perspective) = (at(StandardView::Top), at(StandardView::Perspective));
        let (front, right) = (at(StandardView::Front), at(StandardView::Right));
        assert!(top.x < perspective.x && top.y < front.y);
        assert!(front.x < right.x && perspective.y < right.y);
        assert_eq!(panes[PERSPECTIVE].view, StandardView::Perspective);
    }

    #[test]
    fn a_maximised_viewport_fills_the_area_alone() {
        assert_eq!(pane_rects(area(), Some(2)), [(2, area())]);
    }

    #[test]
    fn a_viewport_size_is_never_zero() {
        let squashed = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
        assert_eq!(pane_size(squashed), [1.0, 1.0]);
    }

    #[test]
    fn a_viewport_can_take_a_plane_of_its_own_and_lose_it_again() {
        let mut pane = Pane::new(StandardView::Top);
        let raised = CPlane {
            origin: [0.0, 0.0, 5.0],
            ..CPlane::WORLD_XY
        };
        pane.set_plane(raised);
        assert_eq!(pane.plane, raised);
        assert_eq!(
            pane.grid.len(),
            grid_lines_in(&raised, &theme::current().viewport).len()
        );
        assert!(pane.grid.iter().all(|v| v.position[2] == 5.0));
        pane.set_plane(pane.view.plane());
        assert_eq!(pane.plane, CPlane::WORLD_XY);
    }
    #[test]
    fn curva_opens_on_the_perspective_viewport_alone() {
        let rects = pane_rects(area(), STARTING_LAYOUT);
        assert_eq!(rects.len(), 1);
        assert_eq!(standard_panes()[rects[0].0].view, StandardView::Perspective);
        assert_eq!(rects[0].1, area());
    }

    fn messages(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("message {index}")).collect()
    }

    #[test]
    fn the_log_takes_the_same_space_however_long_it_is() {
        for count in [0, 1, LOG_LINES, LOG_LINES + 7] {
            assert_eq!(log_rows(&messages(count)).len(), LOG_LINES);
        }
    }

    #[test]
    fn the_log_shows_the_newest_messages_last() {
        let log = messages(LOG_LINES + 2);
        let rows = log_rows(&log);
        assert_eq!(rows[LOG_LINES - 1], log[LOG_LINES + 1]);
        assert_eq!(rows[0], log[2]);
    }

    #[test]
    fn a_short_log_is_padded_above_its_messages() {
        let log = messages(1);
        let rows = log_rows(&log);
        assert_eq!(rows[LOG_LINES - 1], "message 0");
        assert_eq!(rows[0], " ");
    }

    #[test]
    fn a_small_movement_is_a_click_and_a_larger_one_a_box() {
        let at = pos2(100.0, 100.0);
        assert!(!is_box_drag(at, pos2(103.0, 104.0)));
        assert!(is_box_drag(at, pos2(120.0, 101.0)));
        assert!(is_box_drag(at, pos2(100.0, 80.0)));
    }
    #[test]
    fn other_buttons_are_not_left_clicks() {
        let at = pos2(10.0, 10.0);
        let events = [Event::PointerButton {
            pos: at,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }];
        let mut clicks = LeftGestures::default();
        assert!(clicks.take(&events, anywhere).is_empty());
        assert!(clicks.pressed_at.is_none());
    }

    #[test]
    fn the_gumball_extrudes_curves_into_surfaces_and_flat_surfaces_into_solids() {
        let up = [0.0, 0.0, 1.0];
        let line = Shape::Curve(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let wall = gumball_extruded(&line, up, 5.0).unwrap().unwrap();
        assert!(!wall.is_solid());
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let block = gumball_extruded(&Shape::Curve(square.clone()), up, 5.0)
            .unwrap()
            .unwrap();
        assert!(block.is_solid());
        let sheet = curva_kernel::Body::planar(&[square]).unwrap();
        let slab = gumball_extruded(&Shape::Body(sheet), up, 5.0)
            .unwrap()
            .unwrap();
        assert!(slab.is_solid());
        assert!((slab.volume().unwrap() - 500.0).abs() < 1e-3);
        assert!(gumball_extruded(&Shape::Body(slab), up, 5.0).is_none());
    }

    #[test]
    fn clicks_snap_to_curves_and_polylines_in_every_display_mode() {
        for display in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut doc = Document::default();
            doc.add(Curve::line([0.0; 3], [40.0, 0.0, 0.0]).unwrap());
            doc.add(
                Curve::polyline(
                    vec![[0.0, 30.0, 0.0], [30.0, 30.0, 0.0], [30.0, 60.0, 0.0]],
                    false,
                )
                .unwrap(),
            );
            doc.add_body(curva_kernel::Body::cuboid([60.0, 0.0, 0.0], [10.0, 10.0, 10.0]).unwrap());
            let outlines = Outlines::of(doc.state());
            let mut processor = Processor::default();
            processor.submit(&mut doc, "line");
            processor.submit(&mut doc, "0,-20");
            let pane = Pane::new(StandardView::Perspective);
            let size = [800.0, 600.0];
            for target in [[40.0, 0.0, 0.0], [30.0, 30.0, 0.0], [20.0, 0.0, 0.0]] {
                let screen = pane.camera.project(target, size).unwrap();
                let aimed = aim_at(&Aiming {
                    outlines: &outlines,
                    request: processor.current_request().unwrap(),
                    camera: &pane.camera,
                    plane: &pane.plane,
                    display,
                    snaps: SnapSet::default(),
                    guides: curva_viewport::Guides::default(),
                    ortho: false,
                    grid_snap: false,
                    snaps_off: false,
                    screen: [screen[0] + 3.0, screen[1] - 2.0],
                    size,
                });
                let (at, hit) = aimed.unwrap();
                assert!(hit.is_some(), "no snap at {target:?} in {display:?}");
                assert!(
                    curva_kernel::gap(at, target) < 1e-6,
                    "{at:?} for {target:?} in {display:?}"
                );
            }
            // A click there ends the line exactly on the snapped point.
            let end = pane.camera.project([30.0, 30.0, 0.0], size).unwrap();
            let (at, _) = aim_at(&Aiming {
                outlines: &outlines,
                request: processor.current_request().unwrap(),
                camera: &pane.camera,
                plane: &pane.plane,
                display,
                snaps: SnapSet::default(),
                guides: curva_viewport::Guides::default(),
                ortho: false,
                grid_snap: false,
                snaps_off: false,
                screen: end,
                size,
            })
            .unwrap();
            processor.click(&mut doc, at);
            let made = doc
                .state()
                .objects
                .last()
                .unwrap()
                .curve()
                .unwrap()
                .sample()
                .unwrap();
            assert!(
                curva_kernel::gap(*made.last().unwrap(), [30.0, 30.0, 0.0]) < 1e-6,
                "{display:?}"
            );
        }
    }
}
