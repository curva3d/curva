//! The Curva application: a window with a viewport and a command line.
//!
//! `curva --picture out.png --run "line;0,0;50,20"` runs commands without a window and saves a
//! picture of the viewport, which is how automatic runs check the drawing.
//! `curva --orbit-benchmark` opens the window, spins the view and prints how fast it drew.

use curva_commands::{Kind, Message, Processor, Translations};
use curva_document::{Document, State};
use curva_viewport::{
    Camera, Drawing, LineRenderer, Outlines, preview_lines, save_picture, scene_lines,
};
use eframe::egui;
use std::sync::Arc;

/// How close to a curve a click must be, in pixels.
const PICK_REACH: f64 = 6.0;
/// How far, in points, a left press must travel to draw a selection box and not be a click.
const BOX_MIN_DRAG: f32 = 6.0;
const BOX_EDGE: egui::Color32 = egui::Color32::from_rgb(230, 230, 235);
const BOX_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 40, 60, 40);
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
    left_gestures: LeftGestures,
    camera: Camera,
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

/// Spins the view for a fixed number of frames and reports how fast the window drew them.
#[derive(Default)]
struct Benchmark {
    times: Vec<f64>,
    work: Vec<f32>,
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
        let work = self.work.iter().sum::<f32>() * 1000.0 / self.work.len().max(1) as f32;
        println!("frame gap: median {middle:.1} ms, 99th percentile {worst:.1} ms");
        println!("work per frame: {work:.2} ms");
    }
}

impl Curva {
    fn new(context: &eframe::CreationContext<'_>, benchmark: bool) -> Self {
        if let Some(state) = context.wgpu_render_state.as_ref() {
            let renderer = LineRenderer::new(&state.device, state.target_format);
            state.renderer.write().callback_resources.insert(renderer);
        }
        Self {
            doc: Document::default(),
            processor: Processor::default(),
            text: Translations::english(),
            typed: String::new(),
            highlight: 0,
            left_gestures: LeftGestures::default(),
            camera: Camera::default(),
            outlines: Outlines::default(),
            scene: Arc::new(Vec::new()),
            drawn: None,
            benchmark: benchmark.then(Benchmark::default),
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
            self.scene = Arc::new(scene_lines(&self.outlines, &selection));
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

    fn command_line(&mut self, ui: &mut egui::Ui) {
        let log = self.processor.log();
        for line in &log[log.len().saturating_sub(LOG_LINES)..] {
            ui.label(line.as_str());
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
            output.response.request_focus();
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
            submit = ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
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
        let delete = ui.input(|input| input.key_pressed(egui::Key::Delete));
        if delete && self.typed.is_empty() && !self.processor.is_running() {
            self.processor.submit(&mut self.doc, "delete");
        }
    }

    fn takes_position(&self) -> bool {
        self.processor.current_request().is_some_and(|request| {
            request.kind == Kind::Point || (request.kind == Kind::Number && request.from.is_some())
        })
    }

    fn handle_mouse(&mut self, ui: &egui::Ui, response: &egui::Response, size: [f64; 2]) {
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
            if middle || (right && shift) {
                self.camera.pan(dx, dy, size[1]);
            } else if right {
                self.camera.orbit(dx, dy);
            }
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|input| input.smooth_scroll_delta.y));
            if scroll != 0.0 {
                self.camera.zoom((-scroll * 0.002).exp());
            }
        }
        let events = ui.input(|input| input.events.clone());
        let on_viewport = |position: egui::Pos2| {
            response.rect.contains(position)
                && ui.ctx().layer_id_at(position) == Some(response.layer_id)
        };
        let gestures = self.left_gestures.take(&events, on_viewport);
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
                if let Some(at) = self.camera.ground_point(local(pressed), size) {
                    self.processor.click(&mut self.doc, at);
                }
            }
            return;
        }
        for (from, to) in gestures {
            let hits = if is_box_drag(from, to) {
                self.outlines
                    .in_box(&self.camera, local(from), local(to), size)
            } else {
                let hit = self
                    .outlines
                    .pick(&self.camera, local(from), size, PICK_REACH);
                hit.into_iter().collect()
            };
            let chosen = combined_selection(self.doc.selection(), hits, shift, remove);
            self.doc.select(&chosen);
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        self.handle_mouse(ui, &response, size);
        self.refresh_scene();
        let cursor = response.hover_pos().and_then(|position| {
            let local = position - rect.min;
            self.camera
                .ground_point([f64::from(local.x), f64::from(local.y)], size)
        });
        let preview = self
            .processor
            .current_request()
            .map(|request| preview_lines(&request.preview, cursor))
            .unwrap_or_default();
        let drawing = Drawing {
            scene: Arc::clone(&self.scene),
            overlay: Arc::new(preview),
            matrix: self.camera.matrix(size[0] / size[1].max(1.0)),
        };
        ui.painter().add(egui_wgpu_callback(rect, drawing));
        let dragging = self
            .left_gestures
            .pressed_at
            .zip(ui.input(|input| input.pointer.latest_pos()))
            .filter(|(from, to)| !self.takes_position() && is_box_drag(*from, *to));
        if let Some((from, to)) = dragging {
            paint_selection_box(ui, rect, from, to);
        }
    }
}

fn egui_wgpu_callback(rect: egui::Rect, drawing: Drawing) -> egui::Shape {
    eframe::egui_wgpu::Callback::new_paint_callback(rect, drawing).into()
}

impl eframe::App for Curva {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        egui::Panel::bottom("command line").show(ui, |ui| self.command_line(ui));
        egui::CentralPanel::default().show(ui, |ui| self.viewport(ui));
        if let Some(benchmark) = &mut self.benchmark {
            self.camera.orbit(BENCHMARK_DRAG, 0.0);
            if benchmark.record(ui, frame) {
                benchmark.report(frame);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                self.benchmark = None;
            }
        }
    }
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
    if let Some(path) = value("--picture") {
        let script = value("--run").map_or("", String::as_str);
        return picture(path, script);
    }
    let benchmark = arguments
        .iter()
        .any(|argument| argument == "--orbit-benchmark");
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
