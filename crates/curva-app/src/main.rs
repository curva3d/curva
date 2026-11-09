//! The Curva application: a window with a viewport and a command line.
//!
//! `curva --picture out.png --run "line;0,0;50,20"` runs commands without a window and saves a
//! picture of the viewport, which is how automatic runs check the drawing.
//! `curva --mcp` runs as an MCP server for AI agents, on standard input and output.
//! `curva --orbit-benchmark --curves 5000` opens the window on a scene of that many circles, spins
//! the view and prints how fast it drew and snapped.

use curva_commands::{Kind, Message, Processor, Translations, ViewName, ViewRequest};
use curva_document::{Document, State};
use curva_kernel::{Curve, Point};
use curva_viewport::{
    CPlane, Camera, Drawing, Gumball, GumballChange, Handle, LineRenderer, Outlines, SnapHit,
    SnapKind, SnapSet, StandardView, VIEW_SLOTS, constrained, ghost_lines, grid_lines,
    object_lines, save_picture, scene_lines,
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
const LOG_LINES: usize = 5;
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
    gumball_drag: Option<GumballDrag>,
    snaps: SnapSet,
    ortho: bool,
    grid_snap: bool,
    outlines: Outlines,
    scene: Arc<Vec<curva_viewport::Vertex>>,
    drawn: Option<(State, Vec<u64>)>,
    benchmark: Option<Benchmark>,
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
fn combined_selection(current: Vec<u64>, hits: Vec<u64>, add: bool, remove: bool) -> Vec<u64> {
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
fn benchmark_scene(doc: &mut Document, count: usize) -> Result<(), String> {
    let side = (count as f64).sqrt().ceil() as usize;
    for index in 0..count {
        let (column, row) = ((index % side.max(1)) as f64, (index / side.max(1)) as f64);
        let centre = [column * BENCHMARK_SPACING, row * BENCHMARK_SPACING, 0.0];
        doc.add(Curve::circle(centre, BENCHMARK_RADIUS).map_err(|error| error.to_string())?);
    }
    Ok(())
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
    fn new(context: &eframe::CreationContext<'_>, benchmark: Option<usize>) -> Self {
        if let Some(state) = context.wgpu_render_state.as_ref() {
            let renderer = LineRenderer::new(&state.device, state.target_format);
            state.renderer.write().callback_resources.insert(renderer);
        }
        let mut doc = Document::default();
        let mut panes = standard_panes();
        if let Some(count) = benchmark
            && benchmark_scene(&mut doc, count).is_ok()
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
            gumball_drag: None,
            snaps: SnapSet::default(),
            ortho: false,
            grid_snap: false,
            outlines: Outlines::default(),
            scene: Arc::new(Vec::new()),
            drawn: None,
            benchmark: benchmark.map(|_| Benchmark::default()),
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
            self.outlines = Outlines::of(self.doc.state());
            self.scene = Arc::new(object_lines(&self.outlines, &selection));
            self.drawn = Some((self.doc.state().clone(), selection));
        }
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
            self.processor.submit(&mut self.doc, &line);
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.typed.clear();
            self.highlight = 0;
            self.processor.cancel(&mut self.doc);
        }
        let delete = !self.naming_layer && ui.input(|input| input.key_pressed(egui::Key::Delete));
        if delete && self.typed.is_empty() && !self.processor.is_running() {
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
        let centre = self.outlines.centre_of(&self.doc.selection())?;
        Some(Gumball { centre })
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
        let selection = self.doc.selection();
        self.doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.binary_search(&object.id).is_ok())
            .take(MOST_GUMBALL_PREVIEW)
            .map(|object| change.applied(&object.curve))
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
            ui.label(self.text.render(&Message::new(curve_kind(&object.curve))));
            if let Ok(length) = object.curve.length() {
                ui.label(self.text.render(&Message::with(
                    "ui.properties.length",
                    vec![format!("{length:.3}")],
                )));
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

    /// Tells the document which plane flat objects go on: the last-used viewport's.
    fn share_work_plane(&mut self) {
        let plane = self.panes[self.active].plane;
        self.doc.set_work_plane([plane.x, plane.y]);
        self.doc.set_work_origin(plane.origin);
    }

    /// Carries out a change of view asked for by a command.
    fn apply_view_request(&mut self) {
        match self.processor.take_view_request() {
            Some(ViewRequest::ZoomExtents) => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds() {
                    for pane in &mut self.panes {
                        pane.camera.frame(low, high);
                    }
                }
            }
            Some(ViewRequest::FourViews) => {
                self.panes = standard_panes();
                self.maximised = None;
            }
            Some(ViewRequest::SetCPlane { origin, x, y }) => {
                let normal = curva_kernel::cross(x, y);
                self.panes[self.active].set_plane(CPlane {
                    origin,
                    x,
                    y,
                    normal,
                });
            }
            Some(ViewRequest::ResetCPlane) => {
                let pane = &mut self.panes[self.active];
                pane.set_plane(pane.view.plane());
            }
            Some(ViewRequest::Show(name)) => {
                self.panes[self.active] = Pane::new(standard_view(name));
            }
            None => {}
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
        for (from, to) in gestures {
            let hits = if is_box_drag(from, to) {
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
            self.text
                .render(&Message::new(view_label(self.panes[pane].view))),
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

    fn viewports(&mut self, ui: &mut egui::Ui) {
        let mut shown = Vec::new();
        for (pane, rect) in pane_rects(ui.available_rect_before_wrap(), self.maximised) {
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
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
        let mut previewed = self.processor.preview(&self.doc, cursor, &self.typed);
        previewed.extend(self.gumball_preview());
        let preview = ghost_lines(&previewed);
        let overlay = Arc::new(preview);
        for (pane, rect, _) in &shown {
            let size = pane_size(*rect);
            let drawing = Drawing {
                slot: *pane,
                grid: Arc::clone(&self.panes[*pane].grid),
                scene: Arc::clone(&self.scene),
                overlay: Arc::clone(&overlay),
                matrix: self.panes[*pane].camera.matrix(size[0] / size[1]),
            };
            ui.painter().add(egui_wgpu_callback(*rect, drawing));
            self.paint_frame(ui, *pane, *rect);
            self.paint_points(ui, *pane, *rect, size, &previewed);
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

fn egui_wgpu_callback(rect: egui::Rect, drawing: Drawing) -> egui::Shape {
    eframe::egui_wgpu::Callback::new_paint_callback(rect, drawing).into()
}

impl eframe::App for Curva {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.share_work_plane();
        egui::Panel::bottom("command line").show(ui, |ui| self.command_line(ui));
        egui::Panel::right("layers and properties").show(ui, |ui| {
            self.layers_panel(ui);
            ui.separator();
            self.properties_panel(ui);
        });
        self.apply_view_request();
        egui::CentralPanel::default().show(ui, |ui| self.viewports(ui));
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

/// A picture of the model for an MCP agent, as the bytes of a PNG file.
fn mcp_picture(doc: &Document) -> Result<Vec<u8>, String> {
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    let path = std::env::temp_dir().join(format!("curva-mcp-{}.png", std::process::id()));
    save_picture(
        scene_lines(&outlines, &doc.selection()),
        &camera.matrix(aspect),
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
    for line in script.split(';') {
        processor.submit(&mut doc, line);
    }
    let outlines = Outlines::of(doc.state());
    let mut camera = Camera::default();
    if let Some((low, high)) = outlines.bounds() {
        camera.frame(low, high);
    }
    let aspect = f64::from(PICTURE_SIZE[0]) / f64::from(PICTURE_SIZE[1]);
    save_picture(
        scene_lines(&outlines, &doc.selection()),
        &camera.matrix(aspect),
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
            value("--curves")
                .and_then(|count| count.parse().ok())
                .unwrap_or(0)
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
