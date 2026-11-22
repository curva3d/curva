//! Text: section headers, captions, quiet notes, and text painted at a point, in the kit's four
//! sizes and the theme's colours.

use super::{CAPTION_FONT, SMALL_FONT, TITLE_FONT, colours};
use eframe::egui;

/// The font for panel titles and tabs.
pub(crate) fn title_font() -> egui::FontId {
    egui::FontId::proportional(TITLE_FONT)
}

/// The font for smaller text.
pub(crate) fn small_font() -> egui::FontId {
    egui::FontId::proportional(SMALL_FONT)
}

/// The font for captions and table headers.
pub(crate) fn caption_font() -> egui::FontId {
    egui::FontId::proportional(CAPTION_FONT)
}

/// The bold font, for the specializations.
pub(crate) fn bold_font() -> egui::FontId {
    egui::FontId::new(TITLE_FONT, egui::FontFamily::Name("Bold".into()))
}

/// A section's header inside a panel: strong, on the left edge.
pub(crate) fn section(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.label(egui::RichText::new(text).strong())
}

/// A question or a name that must stand out, such as a dialog's question.
pub(crate) fn strong(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).strong()
}

/// A caption: small and quiet, for what explains rather than says.
pub(crate) fn caption(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).small().weak()
}

/// Quiet text, at body size.
pub(crate) fn quiet(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).weak()
}

/// Text in the accent, for what a click answers with, such as a command's options.
pub(crate) fn accented(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).color(colours::accent())
}

/// Curva's name beside its mark.
pub(crate) fn brand(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).strong().size(super::BODY_FONT)
}

/// A heading, for the About page only.
pub(crate) fn heading(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.heading(text.into())
}

/// How wide `text` is in `font`.
pub(crate) fn text_width(ui: &egui::Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font.clone(), colours::text())
            .size()
            .x
    })
}

/// Laid-out text, to measure before painting it. Painted in whatever
/// colour it is painted with.
pub(crate) fn galley(
    ui: &egui::Ui,
    text: String,
    font: egui::FontId,
) -> std::sync::Arc<egui::Galley> {
    ui.painter()
        .layout_no_wrap(text, font, egui::Color32::PLACEHOLDER)
}

/// A name and, quieter and smaller before it, the folder it is in, right-aligned against the
/// right edge of `room`, as the top bar shows the model's file; `glow` (0 to 1) lights the line
/// as after copying it. Returns the line's rectangle, for clicks.
pub(crate) fn name_and_folder(
    ui: &egui::Ui,
    room: egui::Rect,
    name: &str,
    folder: Option<&str>,
    glow: Option<f32>,
) -> egui::Rect {
    let (name_font, folder_font) = (name_font(), caption_font());
    let name_width = text_width(ui, name, &name_font);
    let folder_width = folder.map_or(0.0, |f| text_width(ui, f, &folder_font));
    let middle = room.center().y;
    let line = egui::Rect::from_min_max(
        egui::pos2(room.max.x - name_width - folder_width - 6.0, middle - 10.0),
        egui::pos2(room.max.x + 4.0, middle + 10.0),
    );
    let painter = ui.painter_at(room.expand(4.0));
    if let Some(strength) = glow {
        super::glow_outline(&painter, line, strength);
    }
    let at = egui::pos2(room.max.x, middle);
    painter.text(
        at,
        egui::Align2::RIGHT_CENTER,
        name,
        name_font,
        colours::text(),
    );
    if let Some(folder) = folder {
        let at = egui::pos2(room.max.x - name_width, middle);
        painter.text(
            at,
            egui::Align2::RIGHT_CENTER,
            folder,
            folder_font,
            colours::quiet(),
        );
    }
    line
}

/// The font for a file's name: between small and body.
pub(crate) fn name_font() -> egui::FontId {
    egui::FontId::proportional(SMALL_FONT + 0.5)
}

/// Text that comes in lit in the accent and fades to plain text as `glow` falls from 1 to 0, as a
/// new notice.
pub(crate) fn fading_accent(text: &str, glow: f32) -> egui::RichText {
    egui::RichText::new(text).color(colours::mix(colours::text(), colours::accent(), glow))
}
