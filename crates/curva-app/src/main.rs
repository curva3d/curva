//! The Curva application: a window with a viewport and a command line.
//!
//! `curva --picture out.png --run "line;0,0;50,20"` runs commands without a window and saves a
//! picture of the viewport, which is how automatic runs check the drawing.

use curva_commands::{Kind, Message, Processor, Translations};
use curva_document::{Document, State};
use curva_viewport::{
    Camera, Drawing, LineRenderer, Outlines, preview_lines, save_picture, scene_lines,
};
use eframe::egui;
use std::sync::Arc;

/// How close to a curve a click must be, in pixels.
const PICK_REACH: f64 = 6.0;
const LOG_LINES: usize = 5;
const PICTURE_SIZE: [u32; 2] = [1200, 800];

struct Curva {
    doc: Document,
    processor: Processor,
    text: Translations,
    typed: String,
    camera: Camera,
    outlines: Outlines,
    scene: Arc<Vec<curva_viewport::Vertex>>,
    drawn: Option<(State, Vec<u64>)>,
}

impl Curva {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        if let Some(state) = context.wgpu_render_state.as_ref() {
            let renderer = LineRenderer::new(&state.device, state.target_format);
            state.renderer.write().callback_resources.insert(renderer);
        }
        Self {
            doc: Document::default(),
            processor: Processor::default(),
            text: Translations::english(),
            typed: String::new(),
            camera: Camera::default(),
            outlines: Outlines::default(),
            scene: Arc::new(Vec::new()),
            drawn: None,
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

    fn command_line(&mut self, ui: &mut egui::Ui) {
        let log = self.processor.log();
        for line in &log[log.len().saturating_sub(LOG_LINES)..] {
            ui.label(line.as_str());
        }
        let prompt = self
            .processor
            .prompt()
            .unwrap_or_else(|| self.text.render(&Message::new("ui.command")));
        let mut submit = false;
        ui.horizontal(|ui| {
            ui.label(format!("{prompt}:"));
            let edit =
                ui.add(egui::TextEdit::singleline(&mut self.typed).desired_width(f32::INFINITY));
            edit.request_focus();
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
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                && let Some(first) = self.processor.completions(&self.typed).first()
            {
                self.typed = (*first).to_owned();
            }
        });
        if !self.processor.is_running() {
            let matches = self.processor.completions(&self.typed);
            if !matches.is_empty() {
                ui.label(
                    self.text
                        .render(&Message::with("ui.matches", vec![matches.join(", ")])),
                );
            }
        }
        if submit {
            let line = std::mem::take(&mut self.typed);
            self.processor.submit(&mut self.doc, &line);
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.typed.clear();
            self.processor.cancel(&mut self.doc);
        }
        let delete = ui.input(|input| input.key_pressed(egui::Key::Delete));
        if delete && self.typed.is_empty() && !self.processor.is_running() {
            self.processor.submit(&mut self.doc, "delete");
        }
    }

    fn handle_mouse(&mut self, ui: &egui::Ui, response: &egui::Response, size: [f64; 2]) {
        let delta = response.drag_delta();
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        let shift = ui.input(|input| input.modifiers.shift);
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Secondary) && shift)
        {
            self.camera.pan(dx, dy, size[1]);
        } else if response.dragged_by(egui::PointerButton::Secondary) {
            self.camera.orbit(dx, dy);
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|input| input.smooth_scroll_delta.y));
            if scroll != 0.0 {
                self.camera.zoom((-scroll * 0.002).exp());
            }
        }
        if !response.clicked() {
            return;
        }
        let Some(position) = response.interact_pointer_pos() else {
            return;
        };
        let local = position - response.rect.min;
        let screen = [f64::from(local.x), f64::from(local.y)];
        let takes_position = self.processor.current_request().is_some_and(|request| {
            request.kind == Kind::Point || (request.kind == Kind::Number && request.from.is_some())
        });
        if takes_position {
            if let Some(at) = self.camera.ground_point(screen, size) {
                self.processor.click(&mut self.doc, at);
            }
            return;
        }
        let hit = self.outlines.pick(&self.camera, screen, size, PICK_REACH);
        let mut chosen = if shift {
            self.doc.selection()
        } else {
            Vec::new()
        };
        chosen.extend(hit);
        self.doc.select(&chosen);
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
    }
}

fn egui_wgpu_callback(rect: egui::Rect, drawing: Drawing) -> egui::Shape {
    eframe::egui_wgpu::Callback::new_paint_callback(rect, drawing).into()
}

impl eframe::App for Curva {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("command line").show(ui, |ui| self.command_line(ui));
        egui::CentralPanel::default().show(ui, |ui| self.viewport(ui));
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
    eframe::run_native(
        "Curva",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            ..eframe::NativeOptions::default()
        },
        Box::new(|context| Ok(Box::new(Curva::new(context)))),
    )
    .map_err(|error| error.to_string())
}
