//! The Print panel (spec `drawing-output.md` §1; Rhino 8's Print dialog as a panel): which
//! layouts, colour or black, a PDF or the printer, and a preview of the page as it will print.

use crate::*;
use curva_commands::{PrintJob, PrintTo};
use curva_document::drawing::{Mark, Page};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

/// The tallest the page preview is drawn, in points.
const MOST_PREVIEW_HEIGHT: f32 = 260.0;

/// What the Print panel holds between frames.
#[derive(Default)]
pub(crate) struct PrintPanel {
    /// The layouts ticked to print; `None` until the panel first opens, when the one being
    /// worked on (or every one) is ticked.
    chosen: Option<Vec<u64>>,
    /// Whether everything prints black.
    black: bool,
    /// Whether it goes to the printer rather than a PDF.
    printer: bool,
    /// Which ticked layout the preview shows.
    page: usize,
    /// The save dialog open for the PDF, and the job waiting for its answer.
    saving: Option<(Receiver<Option<PathBuf>>, PrintJob)>,
    /// The page preview, built once on another thread and kept while nothing changes.
    preview: super::print_preview::Preview,
}

/// The preview's page drawn into `rect`, white paper with its marks; fills show as outlines.
fn paint_page(ui: &egui::Ui, page: &Page, rect: egui::Rect) {
    let painter = ui.painter_at(rect);
    let [width, height] = page.size;
    let scale = (rect.width() as f64 / width).min(rect.height() as f64 / height);
    let paper = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2((width * scale) as f32, (height * scale) as f32),
    );
    painter.rect_filled(paper, 0.0, egui::Color32::WHITE);
    let at = |p: [f64; 2]| {
        egui::pos2(
            paper.min.x + (p[0] * scale) as f32,
            paper.max.y - (p[1] * scale) as f32,
        )
    };
    let colour = |c: [f32; 3]| {
        let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
        egui::Color32::from_rgb(r, g, b)
    };
    for mark in &page.marks {
        match mark {
            Mark::Line {
                points,
                closed,
                colour: c,
                width,
                ..
            } => {
                let stroke = egui::Stroke::new(((width * scale) as f32).max(0.6), colour(*c));
                let mut shown: Vec<egui::Pos2> = points.iter().map(|p| at(*p)).collect();
                if *closed && let Some(first) = shown.first().copied() {
                    shown.push(first);
                }
                painter.add(egui::Shape::line(shown, stroke));
            }
            Mark::Fill {
                contours,
                colour: c,
                ..
            } => {
                for contour in contours {
                    let mut shown: Vec<egui::Pos2> = contour.iter().map(|p| at(*p)).collect();
                    if let Some(first) = shown.first().copied() {
                        shown.push(first);
                    }
                    painter.add(egui::Shape::line(shown, egui::Stroke::new(0.8, colour(*c))));
                }
            }
        }
    }
}

/// A box to tick for each layout; the ticked ones keep the Layouts panel's order.
fn layout_ticks(ui: &mut egui::Ui, chosen: &mut Vec<u64>, layouts: &[(u64, String)]) {
    for (id, name) in layouts {
        let mut ticked = chosen.contains(id);
        if ui_kit::toggle(ui, &mut ticked, name).changed() {
            if ticked {
                chosen.push(*id);
                let order: Vec<u64> = layouts.iter().map(|(l, _)| *l).collect();
                chosen.sort_by_key(|c| order.iter().position(|o| o == c));
            } else {
                chosen.retain(|c| c != id);
            }
        }
    }
}

/// Colour as displayed or everything black, and a PDF or the printer.
fn print_choices(ui: &mut egui::Ui, panel: &mut PrintPanel, say: &dyn Fn(&'static str) -> String) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut panel.black, false, say("ui.print.display"));
        ui.radio_value(&mut panel.black, true, say("ui.print.black"));
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut panel.printer, false, say("ui.print.pdf"));
        ui.radio_value(&mut panel.printer, true, say("ui.print.printer"));
    });
}

/// The preview, one ticked layout at a time, with buttons to step through them; drawn in the
/// paper's own shape, no taller than the preview may be.
fn page_preview(
    ui: &mut egui::Ui,
    doc: &Document,
    panel: &mut PrintPanel,
    chosen: &[u64],
    say: &dyn Fn(&'static str) -> String,
) {
    let count = chosen.len();
    let page_index = panel.page.min(count - 1);
    ui.horizontal(|ui| {
        if ui.small_button("◀").clicked() {
            panel.page = page_index.saturating_sub(1);
        }
        ui.label(format!("{} / {count}", page_index + 1));
        if ui.small_button("▶").clicked() {
            panel.page = (page_index + 1).min(count - 1);
        }
    });
    let key = super::print_preview::PreviewKey::of(doc, chosen[page_index], panel.black);
    let fields = curva_document::fields::FieldContext::now(doc.path());
    let (page, waiting) = panel.preview.page(doc, &key, fields);
    if waiting {
        // Never looks frozen: the page being drawn says so, and the panel checks back.
        ui.horizontal(|ui| {
            ui.spinner();
            ui.weak(say("ui.print.preparing"));
        });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    if let Some(page) = page {
        let width = ui.available_width().max(120.0);
        let aspect = (page.size[1] / page.size[0].max(1e-9)) as f32;
        let height = (width * aspect).min(MOST_PREVIEW_HEIGHT);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        paint_page(ui, page, rect);
    }
}

impl Curva {
    /// The Print panel's contents.
    pub(crate) fn print_panel(&mut self, ui: &mut egui::Ui) {
        self.serve_print_dialog();
        // Named as their tabs name them.
        let layouts: Vec<(u64, String)> = self
            .doc
            .layouts()
            .iter()
            .enumerate()
            .map(|(index, l)| (l.id, crate::sheets::layout_title(&self.text, l, index)))
            .collect();
        let say = |key: &'static str| self.text.render(&Message::new(key));
        if layouts.is_empty() {
            ui.label(say("error.print_no_layouts"));
            return;
        }
        let space = self.doc.space();
        let panel = &mut self.print_panel;
        let chosen = panel.chosen.get_or_insert_with(|| match space {
            Some(id) => vec![id],
            None => layouts.iter().map(|(id, _)| *id).collect(),
        });
        chosen.retain(|id| layouts.iter().any(|(l, _)| l == id));
        ui.label(egui::RichText::new(say("ui.print.layouts")).strong());
        layout_ticks(ui, chosen, &layouts);
        ui.separator();
        print_choices(ui, panel, &say);
        let chosen = panel.chosen.clone().unwrap_or_default();
        let job = PrintJob {
            layouts: chosen.clone(),
            black: panel.black,
            to: if panel.printer {
                PrintTo::Printer
            } else {
                PrintTo::Pdf(String::new())
            },
        };
        let busy = panel.saving.is_some();
        let go = ui
            .add_enabled(
                !chosen.is_empty() && !busy,
                egui::Button::new(say("ui.print.go")),
            )
            .clicked();
        if !chosen.is_empty() {
            page_preview(ui, &self.doc, panel, &chosen, &say);
        }
        if go {
            if job.to == PrintTo::Printer {
                self.print(&job);
            } else {
                self.ask_where_to_save(job, &chosen);
            }
        }
    }

    /// Opens the system's save dialog for the PDF; the job waits for its answer.
    fn ask_where_to_save(&mut self, job: PrintJob, chosen: &[u64]) {
        let name = chosen
            .first()
            .and_then(|id| {
                let index = self.doc.layouts().iter().position(|l| l.id == *id)?;
                Some(crate::sheets::layout_title(
                    &self.text,
                    &self.doc.layouts()[index],
                    index,
                ))
            })
            .unwrap_or_else(|| self.text.render(&Message::new("ui.untitled")));
        let filter = self.text.render(&Message::new("files.pdf"));
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter(filter, &["pdf"])
            .set_file_name(format!("{name}.pdf"));
        let (send, picked) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let chosen = pollster::block_on(dialog.save_file());
            let _ = send.send(chosen.map(|file| file.path().to_path_buf()));
        });
        self.print_panel.saving = Some((picked, job));
    }

    /// Prints the waiting job once the save dialog answers; Cancel drops it.
    fn serve_print_dialog(&mut self) {
        let Some((picked, _)) = &self.print_panel.saving else {
            return;
        };
        let answer = match picked.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        let Some((_, mut job)) = self.print_panel.saving.take() else {
            return;
        };
        if let Some(path) = answer {
            let mut path = path.display().to_string();
            if !path.to_ascii_lowercase().ends_with(".pdf") {
                path.push_str(".pdf");
            }
            job.to = PrintTo::Pdf(path);
            self.print(&job);
        }
    }
}
