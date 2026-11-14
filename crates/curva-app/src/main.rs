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
//!
//! This file only starts the program and runs the frame loop. Each part of the window is a module
//! that adds its own methods to [`Curva`] and keeps its own state in a field of it
//! (`docs/reviews/code-health-2026-11-14.md`, fix 4): `viewports` (panes, views, drawing),
//! `mouse` (clicks, boxes, sketching, snaps), `gumball`, `grips`, `command_line`, `top_bar`,
//! `panels` (`frames`, `layers`, `properties`, `blocks`), `settings`, `assistant_panel`, `agent`
//! (MCP), `notices` (opening, updates, recovery) and `benchmark`. A new part gets a module of its
//! own rather than growing this file.

// No console window beside Curva on Windows (spec `windows-no-console.md`).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod agent;
mod assistant;
mod assistant_panel;
mod benchmark;
mod command_line;
mod desktop;
mod file_dialogs;
mod grips;
mod gumball;
mod icons;
mod layout;
mod mouse;
mod notices;
mod panels;
mod recovery;
mod settings;
mod theme;
mod top_bar;
mod updates;
mod viewports;

#[allow(clippy::wildcard_imports)]
use {
    agent::*, assistant_panel::*, benchmark::*, command_line::*, file_dialogs::*, gumball::*,
    mouse::*, notices::*, panels::frames::*, settings::*, top_bar::*, viewports::*,
};

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
/// What Curva puts on the system clipboard when it copies objects, so that the next Ctrl+V
/// (which the window only reports as a paste of text) is known to be theirs.
const CLIPBOARD_MARK: &str = "Curva objects (paste them into Curva with Ctrl+V)";
/// How long the pointer rests on a snap point to hold it, or let it go, in seconds.
const HOLD_AFTER: f64 = 0.5;
/// The Perspective viewport's place in the list of viewports.
const PERSPECTIVE: usize = 1;
/// The band at the top of a viewport that holds its title, in points.
const TITLE_HEIGHT: f32 = 22.0;
const PANE_GAP: f32 = 2.0;
/// The part of a viewport's corner that opens its display menu when clicked.
const TITLE_SIZE: egui::Vec2 = egui::vec2(120.0, 22.0);
/// Which viewport fills the window when Curva opens.
const STARTING_LAYOUT: Option<usize> = Some(PERSPECTIVE);
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
/// Room left around a panel for its shadow, beyond which nothing it holds is drawn.
const SHADOW_ROOM: f32 = 20.0;
/// How long a panel takes to slide in or out, in seconds.
const SLIDE_TIME: f32 = 0.2;
/// Where a dragged panel would land.
const DOCK_PREVIEW_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(56, 30, 12, 56);
/// How far the pointer moves, in points, between the points Sketch takes while drawing.
const SKETCH_STEP: f32 = 4.0;
/// How long the command bar's opening lines glow, in seconds.
const NOTICE_GLOW: f32 = 2.0;
/// Views kept for each viewport's `UndoView`.
const MOST_VIEWS_KEPT: usize = 50;
/// How fast Ctrl + right-drag zooms, per pixel dragged.
const ZOOM_DRAG_SPEED: f64 = 0.01;

struct Curva {
    doc: Document,
    processor: Processor,
    /// The system's file dialogs for commands that ask for a file.
    file_dialogs: FileDialogs,
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
    /// When each linked block's file was last read, to read it again when it changes.
    linked_seen: std::collections::HashMap<String, std::time::SystemTime>,
    /// Whether the window had focus last frame.
    window_had_focus: bool,
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
    /// The last solid or surface measured for the properties panel: its id, the body as it
    /// was, and what it is, which measure, and the value.
    measured: Option<(u64, curva_kernel::Body, (&'static str, &'static str, f64))>,
    drawn: Option<(State, Vec<u64>)>,
    benchmark: Option<Benchmark>,
    /// Keeps a recovery copy of the model; off for benchmarks.
    autosave: Option<recovery::Autosave>,
    /// Where the panels are.
    layout: layout::Layout,
    /// The user's settings: theme, glass or solid panels, grid.
    settings: theme::Settings,
    /// A panel whose cross was clicked, while asking whether to close it.
    closing: Option<layout::PanelId>,
    /// The window's area this frame, for panels that open in its centre.
    window_rect: egui::Rect,
    /// Where the pointer last aimed in a viewport, for Enter to place a point with a held distance.
    last_aim: Option<Point>,
    /// The last aim, with its snap, held while the view is turned or slid.
    held_aim: Option<(Point, Option<SnapHit>)>,
    /// Objects copied with Ctrl+C or Ctrl+X, with their layers, for Ctrl+V.
    clipboard: Vec<(Shape, Option<u64>)>,
    /// The command bar's own lines: an update on offer, the welcome.
    notices: Vec<Notice>,
    /// A newer release on offer.
    update: Option<updates::Release>,
    /// The bodies of the last solid preview, and their ghosted triangles.
    ghosted: (Vec<curva_kernel::Body>, Arc<Vec<MeshVertex>>),
    /// The agents' connection and what they asked for.
    agents: Agents,
    /// The AI Assistant panel's chat and connection choices.
    assistant: AssistantPanel,
    /// A gumball drag or typed value under way.
    gumball_state: GumballState,
    /// What the mouse is in the middle of in the viewports.
    mouse: MouseState,
    /// A model left behind by a crash, and the update check and download.
    startup: Startup,
    /// The panels' focus, sliding and dragging.
    frames: Frames,
    /// The Settings panel's tab and the questions it is asking.
    settings_ui: SettingsUi,
    /// The top bar's chosen command tab and specialization.
    bar: TopBar,
    /// What the viewports draw, kept between frames.
    drawing: ViewportCache,
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
            processor: {
                // Commands that ask for a file show the system's dialog here; scripts, agents
                // and benchmarks type locations instead.
                let mut processor = Processor::default();
                processor.use_file_dialogs(benchmark.is_none());
                processor
            },
            file_dialogs: FileDialogs::default(),
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
            linked_seen: std::collections::HashMap::new(),
            window_had_focus: false,
            alt_held: false,
            snaps: SnapSet::default(),
            ortho: false,
            grid_snap: false,
            pick_filter: PickFilter::default(),
            filter_row: false,
            outlines: Outlines::default(),
            scene: Arc::new(Vec::new()),
            measured: None,
            drawn: None,
            benchmark: benchmark.map(|_| Benchmark::default()),
            autosave,
            settings: theme::file()
                .filter(|_| benchmark.is_none())
                .map(|file| theme::Settings::load(&file))
                .unwrap_or_default(),
            closing: None,
            window_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
            layout: layout::file()
                .filter(|_| benchmark.is_none())
                .map(|file| layout::Layout::load(&file))
                .unwrap_or_default(),
            last_aim: None,
            held_aim: None,
            clipboard: Vec::new(),
            notices: Vec::new(),
            update: None,
            ghosted: (Vec::new(), Arc::new(Vec::new())),
            agents: Agents::default(),
            assistant: AssistantPanel::default(),
            gumball_state: GumballState::default(),
            mouse: MouseState::default(),
            startup: Startup {
                left_behind,
                ..Startup::default()
            },
            frames: Frames::default(),
            settings_ui: SettingsUi::default(),
            bar: TopBar::default(),
            drawing: ViewportCache::default(),
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
        self.serve_file_dialogs(frame);
        // Opening or starting a model makes a new document, so its undo limits are set here.
        let limits = self.settings.undo_limits();
        if self.doc.undo_limits() != limits {
            self.doc.set_undo_limits(limits);
        }
        // Linked blocks are read again when Curva gets focus back, in case their files changed.
        let has_focus = ui.ctx().input(|input| input.focused);
        if has_focus && !self.window_had_focus && !self.doc.definitions().is_empty() {
            self.reload_linked(false);
        }
        self.window_had_focus = has_focus;
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
                    .with_app_id(desktop::APP_ID)
                    .with_maximized(true),
                None => egui::ViewportBuilder::default()
                    .with_app_id(desktop::APP_ID)
                    .with_maximized(true),
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
}
