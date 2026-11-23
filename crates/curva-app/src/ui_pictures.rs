//! `curva --ui-pictures <folder>` saves pictures of Curva's own window in several states (both
//! themes, Settings, a command running, the Print and Layouts panels, and the dialogs), taken by the window itself
//! through egui, so the interface can be looked at without anyone driving the screen. It opens a
//! small building to show, then closes when the last picture is saved.

use crate::*;

/// The window's size in the pictures, in points.
const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// Frames to let a state settle (panels slide, the scene draws) before its picture.
const SETTLE: u32 = 40;

/// The states pictured, in order, each with its file name.
const STATES: [&str; 11] = [
    "1-main-dark",
    "2-main-light",
    "3-command-running",
    "4-settings",
    "5-print",
    "6-layouts",
    "7-display-panel",
    "8-mesh-settings",
    "9-sketchup-import",
    "10-units-question",
    "11-recovery",
];

/// Where the pictures go and how far along they are.
pub(crate) struct UiPictures {
    folder: std::path::PathBuf,
    state: usize,
    frames: u32,
    asked: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl UiPictures {
    pub(crate) fn new(folder: &std::path::Path) -> Self {
        Self {
            folder: folder.to_owned(),
            state: 0,
            frames: 0,
            asked: None,
        }
    }
}

impl Curva {
    /// Moves the pictures along: sets up each state, waits for it to settle, asks the window for
    /// a picture of itself, and saves it when it comes; closes the window after the last.
    pub(crate) fn take_ui_pictures(&mut self, ctx: &egui::Context) {
        let Some(mut pictures) = self.startup.ui_pictures.take() else {
            return;
        };
        ctx.request_repaint();
        if pictures.state == 0 && pictures.frames == 0 {
            // The standard arrangement, so the pictures never depend on a saved one.
            self.layout = layout::Layout::default();
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW));
            let scene = BenchmarkScene {
                curves: 0,
                splines: 0,
                solids: 0,
                shapes: 0,
                trees: 0,
                offset: [0.0; 3],
                select: false,
                building: true,
                house: false,
                meshes: 0,
            };
            let _ = benchmark_scene(&mut self.doc, scene);
            self.processor.submit(&mut self.doc, "ZoomExtents");
        }
        if pictures.frames == 0 {
            self.set_up_picture(STATES[pictures.state]);
        }
        pictures.frames += 1;
        let saved = pictures
            .asked
            .as_ref()
            .is_some_and(|done| done.load(std::sync::atomic::Ordering::SeqCst));
        if saved {
            pictures.state += 1;
            pictures.frames = 0;
            pictures.asked = None;
            if pictures.state == STATES.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        } else if pictures.frames > SETTLE && pictures.asked.is_none() {
            let file = pictures
                .folder
                .join(format!("{}.png", STATES[pictures.state]));
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (flag, repaint) = (done.clone(), ctx.clone());
            let callback = egui::ScreenshotCallback::new(move |image| {
                if let Err(error) = save_picture(&image, &file) {
                    eprintln!("{}: {error}", file.display());
                }
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                repaint.request_repaint();
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
            pictures.asked = Some(done);
        }
        self.startup.ui_pictures = Some(pictures);
    }

    /// Puts the window in the state named `name`.
    fn set_up_picture(&mut self, name: &str) {
        // `CURVA_PICTURES_LIGHT` takes every picture in the light theme.
        let light = name.contains("light") || std::env::var_os("CURVA_PICTURES_LIGHT").is_some();
        // `CURVA_PICTURES_SOLID` takes them with solid panels rather than frosted glass.
        self.settings.glass = std::env::var_os("CURVA_PICTURES_SOLID").is_none();
        self.settings.theme = if light {
            theme::ThemeChoice::Light
        } else {
            theme::ThemeChoice::Dark
        };
        if self.processor.is_running() {
            self.processor.cancel(&mut self.doc);
        }
        self.settings_ui.unit_asked = None;
        self.startup.left_behind = None;
        match name {
            "3-command-running" => self.processor.submit(&mut self.doc, "Circle"),
            "4-settings" => self.open_settings(SettingsTab::General),
            "5-print" => self.open_piece(layout::Piece::Print),
            "6-layouts" => self.open_piece(layout::Piece::Layouts),
            "7-display-panel" => self.open_piece(layout::Piece::Display),
            "8-mesh-settings" => {
                let all: Vec<u64> = self.doc.state().objects.iter().map(|o| o.id).collect();
                self.doc.select(&all);
                self.processor.submit(&mut self.doc, "Mesh");
            }
            "9-sketchup-import" => {
                self.doc.select(&[]);
                self.processor.submit(&mut self.doc, "Import");
                self.processor.submit(&mut self.doc, "picture.skp");
            }
            "10-units-question" => self.settings_ui.unit_asked = Some(curva_document::Unit::Metres),
            "11-recovery" => self.startup.left_behind = Some("recovery.curva".into()),
            _ => {}
        }
    }
}

/// Saves `image` as a PNG file at `file`.
fn save_picture(image: &egui::ColorImage, file: &std::path::Path) -> Result<(), String> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let [width, height] = image.size;
    let out = std::fs::File::create(file).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(out), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    writer.write_image_data(&bytes).map_err(|e| e.to_string())
}
