//! The Curva application: a window with a viewport and a command line.
//!
//! `curva --picture out.png --run "line;0,0;50,20"` runs commands without a window and saves a
//! picture of the viewport, which is how automatic runs check the drawing.
//! `curva --mcp` runs as an MCP server for AI agents, on standard input and output, on a model of
//! its own. `curva --mcp --connect` connects an agent to the window that is open instead.
//! `curva --orbit-benchmark --curves 5000` opens the window on a scene of that many circles, spins
//! the view and prints how fast it drew and snapped. `--solids 500` adds that many cylinders.

mod layout;
mod recovery;

use curva_commands::{DisplayMode, Kind, Message, Processor, Translations, ViewName, ViewRequest};
use curva_document::{Document, Grip, Shape, State};
use curva_kernel::{Curve, Point};
use curva_mcp::Session;
use curva_viewport::{
    CPlane, Camera, DEPTH_BITS, Drawing, Gumball, GumballChange, Handle, LineRenderer, MeshCache,
    MeshVertex, Outlines, PickFilter, Picture, SnapHit, SnapKind, SnapSet, StandardView,
    VIEW_SLOTS, constrained, face_vertices, ghost_lines, grid_lines, object_lines, save_picture,
    scene_lines, shaded_object_lines, shaded_scene_lines,
};
use eframe::egui;
use std::sync::Arc;

/// How close to a curve a click must be, in pixels.
const PICK_REACH: f64 = 6.0;
/// How far, in points, a left press must travel to draw a selection box and not be a click.
const BOX_MIN_DRAG: f32 = 6.0;
const BOX_EDGE: egui::Color32 = egui::Color32::from_rgb(230, 230, 235);
const BOX_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 40, 60, 40);
const SNAP_MARKER: egui::Color32 = egui::Color32::from_rgb(255, 170, 40);
const SNAP_MARKER_SIZE: f32 = 10.0;
/// Spacing and size of the circles in a benchmark scene, in millimetres.
const BENCHMARK_SPACING: f64 = 12.0;
const BENCHMARK_RADIUS: f64 = 5.0;
/// The Perspective viewport's place in the list of viewports.
const PERSPECTIVE: usize = 1;
/// The band at the top of a viewport that holds its title, in points.
const TITLE_HEIGHT: f32 = 22.0;
const PANE_GAP: f32 = 2.0;
const TITLE_COLOUR: egui::Color32 = egui::Color32::from_rgb(190, 190, 200);
/// The part of a viewport's corner that opens its display menu when clicked.
const TITLE_SIZE: egui::Vec2 = egui::vec2(120.0, 22.0);
const ACTIVE_EDGE: egui::Color32 = egui::Color32::from_rgb(120, 150, 210);
/// Which viewport fills the window when Curva opens.
const STARTING_LAYOUT: Option<usize> = Some(PERSPECTIVE);
/// Most objects shown moving during a gumball drag.
const MOST_GUMBALL_PREVIEW: usize = 2000;
const GUMBALL_TIP: f32 = 5.0;
const GUMBALL_ACTIVE: egui::Color32 = egui::Color32::from_rgb(255, 230, 90);
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
/// A picked control point, yellow like selected objects.
const GRIP_PICKED: egui::Color32 = egui::Color32::from_rgb(255, 220, 60);
/// The lines between a smooth curve's control points.
const CONTROL_POLYGON: egui::Color32 = egui::Color32::from_rgb(150, 150, 165);
const LOG_LINES: usize = 3;
/// Behind the viewports, showing in the gaps around the floating panels.
const WINDOW_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(22, 23, 27);
/// A solid panel.
const PANEL_FILL: egui::Color32 = egui::Color32::from_rgb(36, 38, 44);
/// The command bar, lighter and see-through, as glass.
const GLASS_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(52, 55, 63, 214);
const PANEL_EDGE: egui::Color32 = egui::Color32::from_rgb(60, 63, 72);
const PANEL_RADIUS: u8 = 12;
const PANEL_MARGIN: f32 = 8.0;
/// Room left around a panel for its shadow, beyond which nothing it holds is drawn.
const SHADOW_ROOM: f32 = 20.0;
/// How long a panel takes to slide in or out, in seconds.
const SLIDE_TIME: f32 = 0.2;
/// The bar a panel is moved by.
const GRIP: egui::Vec2 = egui::vec2(32.0, 4.0);
/// Where a dragged panel would land.
const DOCK_PREVIEW_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(24, 33, 51, 64);
/// Width of the agent connection, at the right of the command bar.
const AGENT_COLUMN: f32 = 170.0;
/// How long after an agent's last request it still counts as connected.
const AGENT_RECENT: std::time::Duration = std::time::Duration::from_secs(30);
const AGENT_ACTIVE: egui::Color32 = egui::Color32::from_rgb(110, 200, 120);
/// The command tabs at the top, each with its most used commands (spec
/// `docs/specs/ui-floating-panels.md`).
const COMMAND_TABS: [(&str, &[&str]); 4] = [
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
            "Point",
            "Offset",
            "Fillet",
            "Chamfer",
        ],
    ),
    (
        "ui.tab.surfaces",
        &[
            "PlanarSrf",
            "ExtrudeCrv",
            "Revolve",
            "Loft",
            "Sweep1",
            "OffsetSrf",
        ],
    ),
    (
        "ui.tab.solids",
        &[
            "Box",
            "Pipe",
            "FilletEdge",
            "ChamferEdge",
            "Shell",
            "Volume",
        ],
    ),
    (
        "ui.tab.operations",
        &[
            "BooleanUnion",
            "BooleanDifference",
            "BooleanIntersection",
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
        ],
    ),
];
const PICTURE_SIZE: [u32; 2] = [1200, 800];
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
    naming_layer: bool,
    agent_on: bool,
    agent: Option<curva_mcp::Listener>,
    waiting_approval: Option<(curva_mcp::AgentRequest, String)>,
    gumball_drag: Option<GumballDrag>,
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
    /// The command tab showing, as an index into [`COMMAND_TABS`].
    command_tab: usize,
    /// The side panel's tab showing: 0 for layers, 1 for properties.
    side_tab: usize,
    /// When an agent last asked something, to show whether one is connected.
    agent_seen: Option<std::time::Instant>,
    /// Where the pointer last aimed in a viewport, for Enter to place a point with a held distance.
    last_aim: Option<Point>,
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
    /// Its grip began to be dragged, held this far from its corner.
    DragStarted(egui::Vec2),
    /// A floating panel's corner was dragged this far.
    Resized(egui::Vec2),
    /// Reset layout was chosen from its grip's menu.
    Reset,
}

/// A panel being moved by its grip.
#[derive(Clone, Copy)]
struct PanelDrag {
    panel: layout::PanelId,
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
fn log_rows(log: &[String]) -> Vec<&str> {
    let shown = &log[log.len().saturating_sub(LOG_LINES)..];
    let blanks = std::iter::repeat_n(" ", LOG_LINES - shown.len());
    blanks.chain(shown.iter().map(String::as_str)).collect()
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
    let galley =
        painter.layout_no_wrap(text.to_owned(), egui::FontId::monospace(12.0), TITLE_COLOUR);
    let corner = pointer + egui::vec2(18.0, 14.0);
    let frame = egui::Rect::from_min_size(corner, galley.size()).expand2(egui::vec2(6.0, 3.0));
    painter.rect(
        frame,
        4.0,
        PANEL_FILL,
        egui::Stroke::new(1.0, PANEL_EDGE),
        egui::StrokeKind::Inside,
    );
    painter.galley(corner, galley, TITLE_COLOUR);
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
}

impl Pane {
    fn set_plane(&mut self, plane: CPlane) {
        self.plane = plane;
        self.grid = Arc::new(grid_lines(&plane));
    }

    fn new(view: StandardView) -> Self {
        Self {
            view,
            camera: view.camera(),
            plane: view.plane(),
            grid: Arc::new(grid_lines(&view.plane())),
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
            // A benchmark must not take over the connection of a window that is in use.
            agent_on: benchmark.is_none(),
            agent: None,
            waiting_approval: None,
            gumball_drag: None,
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
            layout: layout::file()
                .filter(|_| benchmark.is_none())
                .map(|file| layout::Layout::load(&file))
                .unwrap_or_default(),
            command_tab: 0,
            side_tab: 0,
            agent_seen: None,
            last_aim: None,
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
            self.outlines = Outlines::of_cached(self.doc.state(), &mut self.meshes);
            self.scene = Arc::new(object_lines(&self.outlines, &selection));
            self.shaded_scene = Arc::new(shaded_object_lines(&self.outlines, &selection));
            self.faces = Arc::new(face_vertices(&self.outlines, &selection));
            self.drawn = Some((self.doc.state().clone(), selection));
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
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (index, name) in matches.iter().enumerate() {
                        let shown = ui.selectable_label(index == self.highlight, *name);
                        if shown.clicked() {
                            clicked = Some(*name);
                        }
                    }
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
        for row in log_rows(self.processor.log()) {
            ui.add(egui::Label::new(row).truncate());
        }
        let prompt = self
            .processor
            .prompt()
            .unwrap_or_else(|| self.text.render(&Message::new("ui.command")));
        // These keys must be taken before the text box, which would otherwise act on them.
        let filled = self.completion_keys(ui);
        let mut submit = false;
        let mut clicked = None;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(format!("{prompt}:"));
            let output = egui::TextEdit::singleline(&mut self.typed)
                .desired_width(f32::INFINITY)
                .show(ui);
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
        if request.kind == Kind::PickPart {
            let camera = &self.panes[pane].camera;
            let on = self.outlines.pick_point(camera, screen, size, PICK_REACH)?;
            return Some((on, None));
        }
        let base = request.preview.last().copied().or(request.from);
        let camera = &self.panes[pane].camera;
        if !alt
            && let Some(hit) = self.outlines.snap_while_drawing(
                camera,
                screen,
                size,
                &self.snaps,
                base,
                &request.preview,
            )
        {
            return Some((hit.point, Some(hit)));
        }
        // A height or extrusion follows the pointer along its line, as in Rhino.
        if let Some((origin, direction)) = request.along {
            return camera
                .line_point(screen, size, origin, direction)
                .map(|at| (at, None));
        }
        let plane = self.panes[pane].plane;
        let cursor = camera.plane_point(screen, size, &plane)?;
        Some((
            constrained(&plane, cursor, base, self.ortho != shift, self.grid_snap),
            None,
        ))
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
        self.gumball()?.change(
            &self.panes[drag.pane].camera,
            drag.size,
            drag.handle,
            drag.from,
            drag.now,
        )
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
                self.gumball_drag = Some(GumballDrag {
                    pane,
                    handle,
                    size,
                    from: local(*pos),
                    now: local(*pos),
                });
                return true;
            }
        }
        false
    }

    /// Applies a finished drag as one undo step.
    fn finish_gumball(&mut self, drag: &GumballDrag) {
        let Some(change) = self.gumball_change(drag) else {
            return;
        };
        if drag.from == drag.now {
            return;
        }
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
        }
        self.doc.refresh_history();
        self.doc.record_change(before);
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
        for (handle, run) in gumball.outlines(camera, size) {
            let colour = if Some(handle) == hovered || Some(handle) == dragged {
                GUMBALL_ACTIVE
            } else {
                GUMBALL_AXES[handle.axis()]
            };
            let points: Vec<egui::Pos2> = run
                .iter()
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                .collect();
            if let (Handle::Move(_), Some(tip)) = (handle, points.last()) {
                painter.circle_filled(*tip, GUMBALL_TIP, colour);
            }
            painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, colour)));
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

    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text.render(&Message::new("ui.layers")));
        let layers = self.doc.layers().to_vec();
        let current = self.doc.current_layer();
        let mut naming = false;
        egui::Grid::new(ui.scope_id().with("layers")).show(ui, |ui| {
            for layer in &layers {
                if ui.radio(layer.id == current, "").clicked() {
                    self.undoable(|doc| doc.set_current_layer(layer.id));
                }
                let mut name = layer.name.clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(90.0));
                naming |= edit.has_focus();
                if edit.changed() {
                    self.doc.rename_layer(layer.id, name);
                }
                let mut colour = layer.colour;
                if ui.color_edit_button_rgb(&mut colour).changed() {
                    self.doc.set_layer_colour(layer.id, colour);
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
        self.naming_layer = naming;
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

    /// Lays out and draws the panels, sliding them out of view in focus mode. Returns the area
    /// left for the viewports, and how far they may reach to be seen through glass panels.
    fn panels(&mut self, ctx: &egui::Context, window: egui::Rect) -> (egui::Rect, egui::Rect) {
        if ctx.input(|input| input.key_pressed(egui::Key::F12)) {
            self.toggle_focus();
        }
        let (placed, free) = self.layout.arrange(window);
        let placed_bar = placed
            .iter()
            .copied()
            .find(|(panel, _)| *panel == layout::PanelId::CommandBar);
        // Typing brings the command bar out until the command ends.
        let typing = !self.typed.is_empty() || self.processor.is_running();
        let pointer = ctx.pointer_hover_pos();
        for (panel, rect) in placed {
            let shown = !self.focus
                || self.revealed == Some(panel)
                || (panel == layout::PanelId::CommandBar && typing);
            let hovered = pointer.is_some_and(|at| {
                self.drawn_panels
                    .iter()
                    .any(|(p, drawn)| *p == panel && drawn.contains(at))
            });
            let target = if shown {
                rect
            } else {
                let edge = self.layout.edge_of(panel, rect, window);
                let visible = if hovered { layout::PEEK } else { layout::STRIP };
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
                let landing =
                    self.layout
                        .moved(panel, layout::drop_dock(at, drawn.min, window), drag.size);
                if let Some((_, target)) = landing
                    .arrange(window)
                    .0
                    .into_iter()
                    .find(|(p, _)| *p == panel)
                {
                    ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::NULL.with("dock preview"),
                    ))
                    .rect(
                        target,
                        PANEL_RADIUS,
                        DOCK_PREVIEW_FILL,
                        egui::Stroke::new(1.5, ACTIVE_EDGE),
                        egui::StrokeKind::Inside,
                    );
                }
                if !ctx.input(|input| input.pointer.primary_down()) {
                    self.layout = landing;
                    self.panel_drag = None;
                    self.save_layout();
                }
            }
            self.drawn_panels.retain(|(p, _)| *p != panel);
            self.drawn_panels.push((panel, drawn));
            let floating = self
                .layout
                .places
                .iter()
                .any(|p| p.panel == panel && matches!(p.dock, layout::Dock::Floating { .. }));
            match self.panel_area(ctx, panel, size, drawn, !shown, floating) {
                PanelEvent::Nothing => {}
                PanelEvent::StripClicked => self.revealed = Some(panel),
                PanelEvent::DragStarted(grab) => {
                    self.panel_drag = Some(PanelDrag {
                        panel,
                        grab,
                        size: drawn.size().min(window.size() * 0.6).max(layout::MIN_PANEL),
                    });
                }
                PanelEvent::Resized(by) => {
                    self.layout = self.layout.resized(panel, size + by);
                    self.save_layout();
                }
                PanelEvent::Reset => {
                    self.layout = layout::Layout::default();
                    self.save_layout();
                }
            }
        }
        self.focus_tab(ctx, window);
        // In focus mode the viewports take the window, short of the strips left at its edges.
        if self.focus {
            return (window.shrink(layout::STRIP), window.shrink(layout::STRIP));
        }
        // The viewports reach under the glass command bar, so there is something to see
        // through it; their titles and frames stay in the free area.
        let mut reach = free;
        if let Some((_, bar)) = placed_bar
            && let Some(place) = self
                .layout
                .places
                .iter()
                .find(|p| p.panel == layout::PanelId::CommandBar)
        {
            match place.dock {
                layout::Dock::Bottom => reach.max.y = bar.max.y,
                layout::Dock::Top => reach.min.y = bar.min.y,
                layout::Dock::Left => reach.min.x = bar.min.x,
                layout::Dock::Right => reach.max.x = bar.max.x,
                layout::Dock::Floating { .. } => {}
            }
        }
        (free, reach)
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
                    PANEL_EDGE
                } else {
                    PANEL_FILL
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
                let stroke = egui::Stroke::new(1.5, TITLE_COLOUR);
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

    /// Draws one floating panel at `drawn`, with rounded corners and a soft shadow, a grip to
    /// move it by, and on a `floating` one a corner to resize it by. A `hidden` panel, slid away
    /// in focus mode, answers a click on what shows of it instead of passing it to what it holds.
    fn panel_area(
        &mut self,
        ctx: &egui::Context,
        panel: layout::PanelId,
        size: egui::Vec2,
        drawn: egui::Rect,
        hidden: bool,
        floating: bool,
    ) -> PanelEvent {
        let rect = drawn;
        let mut event = PanelEvent::Nothing;
        let glass = panel == layout::PanelId::CommandBar;
        let frame = egui::Frame::new()
            // The command bar is glass: lighter and see-through. Its blur comes in a later step.
            .fill(if glass { GLASS_FILL } else { PANEL_FILL })
            .stroke(egui::Stroke::new(1.0, PANEL_EDGE))
            .corner_radius(PANEL_RADIUS)
            .shadow(egui::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(90),
            })
            .inner_margin(PANEL_MARGIN);
        egui::Area::new(egui::Id::NULL.with(("panel", panel)))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                // Nothing a panel holds may spill outside its place.
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                if glass {
                    // The blurred view behind, under the tint the frame paints over it.
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
                    match panel {
                        layout::PanelId::CommandTabs => self.command_tabs(ui),
                        layout::PanelId::Side => self.side_panel(ui),
                        layout::PanelId::CommandBar => self.command_bar(ui),
                    }
                });
                // Added after the contents, so these sit above the panel's own controls.
                if hidden {
                    let strip =
                        ui.interact(rect, ui.scope_id().with("strip"), egui::Sense::click());
                    if strip.clicked() {
                        event = PanelEvent::StripClicked;
                    }
                    return;
                }
                // The grip: a small bar in the top-left corner's margin.
                let grip = egui::Rect::from_min_size(rect.min + egui::vec2(14.0, 2.5), GRIP);
                let held = ui
                    .interact(
                        grip.expand(4.0),
                        ui.scope_id().with("grip"),
                        egui::Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                let lit = held.hovered() || held.dragged();
                ui.painter().rect_filled(
                    grip,
                    GRIP.y / 2.0,
                    if lit { ACTIVE_EDGE } else { PANEL_EDGE },
                );
                if held.drag_started()
                    && let Some(at) = held.interact_pointer_pos()
                {
                    event = PanelEvent::DragStarted(at - rect.min);
                }
                held.context_menu(|ui| {
                    if ui
                        .button(self.text.render(&Message::new("ui.layout.reset")))
                        .clicked()
                    {
                        event = PanelEvent::Reset;
                        ui.close();
                    }
                });
                if floating {
                    let corner =
                        egui::Rect::from_min_max(rect.max - egui::Vec2::splat(14.0), rect.max);
                    let resize = ui
                        .interact(corner, ui.scope_id().with("resize"), egui::Sense::drag())
                        .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                    let stroke = egui::Stroke::new(
                        1.0,
                        if resize.hovered() {
                            ACTIVE_EDGE
                        } else {
                            PANEL_EDGE
                        },
                    );
                    for inset in [4.0, 8.0] {
                        ui.painter().line_segment(
                            [
                                egui::pos2(rect.max.x - inset, rect.max.y - 3.0),
                                egui::pos2(rect.max.x - 3.0, rect.max.y - inset),
                            ],
                            stroke,
                        );
                    }
                    if resize.dragged() {
                        event = PanelEvent::Resized(resize.drag_delta());
                    }
                }
            });
        event
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
                for name in COMMAND_TABS[self.command_tab].1 {
                    if ui.button(*name).on_hover_text(*name).clicked() {
                        chosen = Some(*name);
                    }
                }
            });
        });
        if let Some(name) = chosen {
            if self.processor.is_running() {
                self.processor.cancel(&mut self.doc);
            }
            self.typed.clear();
            self.highlight = 0;
            self.processor.submit(&mut self.doc, name);
        }
    }

    /// Layers and properties, in tabs.
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (index, key) in ["ui.tab.layers", "ui.tab.properties"]
                .into_iter()
                .enumerate()
            {
                let label = self.text.render(&Message::new(key));
                if ui.selectable_label(self.side_tab == index, label).clicked() {
                    self.side_tab = index;
                }
            }
        });
        ui.separator();
        // Both ways, so a wide row scrolls inside the panel instead of stretching it.
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.side_tab == 0 {
                    self.layers_panel(ui);
                } else {
                    self.properties_panel(ui);
                }
            });
    }

    /// The command line and drawing aids on the left, the agent connection on the right.
    fn command_bar(&mut self, ui: &mut egui::Ui) {
        let agent_width = AGENT_COLUMN;
        ui.horizontal_top(|ui| {
            let command_width = (ui.available_width() - agent_width - 16.0).max(100.0);
            ui.vertical(|ui| {
                ui.set_width(command_width);
                self.command_line(ui);
            });
            ui.separator();
            ui.vertical(|ui| self.agent_status(ui));
        });
    }

    /// Agent access, and whether an agent is talking to Curva.
    fn agent_status(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(self.text.render(&Message::new("ui.agent.title"))).strong());
        ui.checkbox(
            &mut self.agent_on,
            self.text.render(&Message::new("ui.agent")),
        );
        let recent = self
            .agent_seen
            .is_some_and(|seen| seen.elapsed() < AGENT_RECENT);
        let (key, colour) = match (self.agent.is_some(), recent) {
            (false, _) => ("ui.agent.off", TITLE_COLOUR),
            (true, false) => ("ui.agent.waiting", TITLE_COLOUR),
            (true, true) => ("ui.agent.active", AGENT_ACTIVE),
        };
        ui.colored_label(colour, self.text.render(&Message::new(key)));
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
        let (delta, shift, middle, right) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.modifiers.shift,
                input.pointer.middle_down(),
                input.pointer.secondary_down(),
            )
        });
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        if response.is_pointer_button_down_on() {
            self.active = pane;
            let camera = &mut self.panes[pane].camera;
            if middle || (right && (shift || camera.is_parallel())) {
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
        painter.rect_stroke(
            egui::Rect::from_center_size(centre, egui::Vec2::splat(SNAP_MARKER_SIZE)),
            0.0,
            egui::Stroke::new(2.0, SNAP_MARKER),
            egui::StrokeKind::Middle,
        );
        painter.text(
            centre + egui::vec2(SNAP_MARKER_SIZE, -SNAP_MARKER_SIZE),
            egui::Align2::LEFT_BOTTOM,
            self.text.render(&Message::new(snap_label(hit.kind))),
            egui::TextStyle::Body.resolve(ui.style()),
            SNAP_MARKER,
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
            TITLE_COLOUR,
        );
        if pane == self.active && self.maximised.is_none() {
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, ACTIVE_EDGE),
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
        let cursor = aimed.map(|(at, _)| at);
        self.last_aim = cursor;
        // Picking a point shows a crosshair, and what the step measures beside it.
        if let Some((.., position)) = hovered
            && self.takes_position()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if let Some(value) = cursor.and_then(|at| self.processor.measure(&self.doc, at)) {
                paint_cursor_tag(ui, position, &format!("{value:.3} mm"));
            }
        }
        let mut previewed = self.processor.preview(&self.doc, cursor, &self.typed);
        previewed.extend(self.gumball_preview());
        let preview = ghost_lines(&previewed);
        let overlay = Arc::new(preview);
        for (pane, rect, _) in &shown {
            let size = pane_size(*rect);
            let camera = &self.panes[*pane].camera;
            let display = self.panes[*pane].display;
            let drawing = Drawing {
                slot: *pane,
                grid: Arc::clone(&self.panes[*pane].grid),
                // Dark edges suit solid faces; ghosted views keep the light wireframe edges.
                scene: Arc::clone(if display == DisplayMode::Shaded {
                    &self.shaded_scene
                } else {
                    &self.scene
                }),
                faces: Arc::clone(&self.faces),
                overlay: Arc::clone(&overlay),
                matrix: camera.matrix(size[0] / size[1]),
                light: camera.forward().map(|value| value as f32),
                shaded: display != DisplayMode::Wireframe,
                ghosted: display == DisplayMode::Ghosted,
                area: *rect,
                frame: ui.ctx().cumulative_frame_nr(),
                background: WINDOW_BACKGROUND,
            };
            ui.painter().add(egui_wgpu_callback(*rect, drawing));
            self.paint_frame(ui, *pane, *rect);
            self.paint_points(ui, *pane, *rect, size, &previewed);
            self.paint_grips(ui, *pane, *rect, size);
            self.paint_picked_edges(ui, *pane, *rect, size);
            self.paint_picked_faces(ui, *pane, *rect, size);
            self.paint_gumball(ui, *pane, *rect, size);
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
        self.share_work_plane();
        // The panels float over the viewports, which take the area they leave free.
        let (free, reach) = self.panels(ui.ctx(), ui.max_rect());
        self.keep_agent_link(ui.ctx());
        self.serve_agents();
        self.approval_window(ui);
        self.recovery_window(ui);
        if let Some(autosave) = &mut self.autosave {
            autosave.tick(&self.doc, std::time::Instant::now());
        }
        self.apply_view_request();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(WINDOW_BACKGROUND))
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
                Some(icon) => egui::ViewportBuilder::default().with_icon(icon),
                None => egui::ViewportBuilder::default(),
            },
            ..eframe::NativeOptions::default()
        },
        Box::new(move |context| Ok(Box::new(Curva::new(context, benchmark)))),
    )
    .map_err(|error| error.to_string())
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
        assert_eq!(pane.grid.len(), grid_lines(&raised).len());
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
}
