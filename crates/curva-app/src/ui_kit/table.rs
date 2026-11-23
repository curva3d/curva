//! The parts of a table or tree, as the Layers panel's: header text, the chosen row's wash,
//! where a dragged row would land, column dividers, a tree's arrows and drag grips, and plain
//! fields and ticks for cells.

use super::{LINE, TIGHT_GAP, colours};
use eframe::egui;

/// A header's words, small and quiet, starting at `at` (the middle of their left end).
pub(crate) fn header_text(ui: &egui::Ui, at: egui::Pos2, text: String) {
    let font = super::caption_font();
    ui.painter()
        .text(at, egui::Align2::LEFT_CENTER, text, font, colours::quiet());
}

/// A note in a header, small and in the accent, as "In detail" over the detail's column.
pub(crate) fn header_note(ui: &egui::Ui, at: egui::Pos2, text: String) {
    let font = super::caption_font();
    ui.painter()
        .text(at, egui::Align2::LEFT_CENTER, text, font, colours::accent());
}

/// The wash behind the chosen row at `rect`.
pub(crate) fn chosen_row(ui: &egui::Ui, rect: egui::Rect) {
    super::chosen_wash(ui.painter(), rect);
}

/// Outlines `rect` in the accent while something is dragged over `response`.
pub(crate) fn drop_outline<T: std::any::Any + Send + Sync>(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
) {
    if response.dnd_hover_payload::<T>().is_some() {
        ui.painter().rect_stroke(
            rect,
            2.0,
            colours::accent_line(LINE),
            egui::StrokeKind::Inside,
        );
    }
}

/// The accent line of a column divider being dragged or pointed at.
pub(crate) fn divider_lit(ui: &egui::Ui, x: f32, y: egui::Rangef) {
    ui.painter().vline(x, y, colours::accent_line(LINE));
}

/// A tree's open or shut arrow, `width` wide and `height` tall, pointing right when shut and
/// down when open, quiet or in the accent under the pointer; with nothing under it, only its
/// room is kept. Returns the click.
pub(crate) fn tree_arrow(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    shut: bool,
    shown: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    if shown {
        let towards = if shut {
            egui::vec2(1.0, 0.0)
        } else {
            egui::vec2(0.0, 1.0)
        };
        let colour = if response.hovered() {
            colours::accent()
        } else {
            colours::quiet()
        };
        super::arrow_head(ui.painter(), rect.center(), towards, colour);
    }
    response
}

/// A drag grip `width` wide and `height` tall: three rows of dots.
pub(crate) fn drag_grip(ui: &mut egui::Ui, width: f32, height: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    super::grip_dots(ui.painter(), rect.center(), 3);
    response
}

/// A field with no frame, `width` wide, for words in a cell that keep all their room.
pub(crate) fn plain_field(ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::symmetric(2, 0))
            .desired_width(width),
    )
}

/// A bare tick in a cell, with no name beside it.
pub(crate) fn tick(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    ui.checkbox(on, "")
}

/// Things in this row sit tight together, as a tree's arrow, grip and name.
pub(crate) fn tight(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = TIGHT_GAP;
}

/// A bare tick that can only be changed while `enabled`, greyed out otherwise, as a layer's On
/// under a layer that is off.
pub(crate) fn tick_when(ui: &mut egui::Ui, enabled: bool, on: &mut bool) -> egui::Response {
    ui.add_enabled(enabled, egui::Checkbox::without_text(on))
}

/// The soft wash behind a row related to what is chosen, such as the layer a selected object is
/// on (owner, 23 November 2026): fainter than [`chosen_row`], with no edge.
pub(crate) fn related_row(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter()
        .rect_filled(rect, super::CONTROL_RADIUS / 2, colours::related_fill());
}
