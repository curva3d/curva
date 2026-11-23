//! Colours by what they are for, from the theme in use. Code outside the kit asks for a role,
//! never a colour, so the two themes and any later ones stay whole.

use crate::theme;
use eframe::egui::{self, Color32};

/// Text.
pub(crate) fn text() -> Color32 {
    theme::current().text
}

/// Quieter text: titles, hints, folders, unchosen tabs.
pub(crate) fn quiet() -> Color32 {
    theme::current().quiet
}

/// The accent: chosen things, focus, what a command adds.
pub(crate) fn accent() -> Color32 {
    theme::ACCENT
}

/// A solid panel.
pub(crate) fn panel() -> Color32 {
    theme::current().panel
}

/// Behind everything; a pill's ground.
pub(crate) fn window() -> Color32 {
    theme::current().window
}

/// A panel's edge, and a strip under the pointer.
pub(crate) fn edge() -> Color32 {
    theme::current().panel_edge
}

/// A control at rest.
pub(crate) fn control() -> Color32 {
    theme::current().control
}

/// A control under the pointer.
pub(crate) fn hover() -> Color32 {
    theme::current().hover
}

/// The wash behind a chosen item in a list, pill or table.
pub(crate) fn chosen_fill() -> Color32 {
    theme::ACCENT.gamma_multiply(0.22)
}

/// The softer wash behind a control that opens something, such as the view picker.
pub(crate) fn opener_fill() -> Color32 {
    theme::ACCENT.gamma_multiply(0.16)
}

/// The outline of a control that opens something.
pub(crate) fn opener_edge() -> Color32 {
    theme::ACCENT.gamma_multiply(0.55)
}

/// A strip's scroll handle, quiet so it never draws the eye.
pub(crate) fn scroll_handle() -> Color32 {
    theme::current().quiet.gamma_multiply(0.6)
}

/// The outline of where a dragged panel would dock.
pub(crate) fn dock_edge() -> Color32 {
    theme::ACCENT.gamma_multiply(0.6)
}

/// The fill of where a dragged panel would dock.
pub(crate) fn dock_fill() -> Color32 {
    Color32::from_rgba_premultiplied(56, 30, 12, 56)
}

/// A panel's shadow.
pub(crate) fn shadow() -> Color32 {
    Color32::from_black_alpha(90)
}

/// The accent at `strength`, from nothing at 0 to full at 1, for glows that fade.
pub(crate) fn accent_at(strength: f32) -> Color32 {
    theme::ACCENT.gamma_multiply(strength)
}

/// A line of `width` in the accent.
pub(crate) fn accent_line(width: f32) -> egui::Stroke {
    egui::Stroke::new(width, theme::ACCENT)
}

/// A line of `width` in the quiet colour, or the accent when `lit`.
pub(crate) fn mark(width: f32, lit: bool) -> egui::Stroke {
    egui::Stroke::new(width, if lit { accent() } else { quiet() })
}

/// Something live and working, such as a connected agent.
pub(crate) fn live() -> Color32 {
    theme::LIVE
}

/// `from` moved towards `to` by `amount` (0 to 1).
pub(crate) fn mix(from: Color32, to: Color32, amount: f32) -> Color32 {
    let blend =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color32::from_rgb(
        blend(from.r(), to.r()),
        blend(from.g(), to.g()),
        blend(from.b(), to.b()),
    )
}

/// A sheet of paper in a small picture of a layout.
pub(crate) fn sheet() -> Color32 {
    Color32::from_rgb(250, 250, 247)
}
