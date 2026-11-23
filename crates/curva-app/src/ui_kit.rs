//! The UI kit: the one place the interface's spacing, sizes and shared controls are set (owner's
//! choice, 22 November 2026: panel titles quiet and centred, on/off as filled chips, compact rows
//! as in Rhino). Every panel draws with these, so changing this file changes the whole interface.
//!
//! The rules:
//! - Inside every panel, [`PAD`] on all four sides.
//! - [`ROW_GAP`] between rows, [`ITEM_GAP`] between things in a row.
//! - Buttons, fields and chips are [`ROW`] tall, with [`CONTROL_RADIUS`] corners.
//! - A panel's title is centred and quiet; all other text starts on the panel's left edge.
//! - Anything with a name that turns on and off is a [`toggle`] chip; a bare tick in a table
//!   cell, with no name beside it, stays a tick box.
//! - Icons are centred on their ink, across and down (`icons::paint` does this).

use crate::theme;
use eframe::egui;

/// Room inside every panel, on all four sides.
pub(crate) const PAD: f32 = 8.0;
/// The height of a row: buttons, fields and chips.
pub(crate) const ROW: f32 = 20.0;
/// Between one row and the next.
pub(crate) const ROW_GAP: f32 = 4.0;
/// Between things side by side in a row.
pub(crate) const ITEM_GAP: f32 = 8.0;
/// Between chips side by side, closer than other things so a set of them reads as one.
pub(crate) const CHIP_GAP: f32 = 4.0;
/// The corners of buttons, fields and chips.
pub(crate) const CONTROL_RADIUS: u8 = 6;
/// Room either side of a chip's or button's name.
pub(crate) const CONTROL_PAD: f32 = 8.0;
/// The size of panel titles, the same on every panel.
pub(crate) const TITLE_FONT: f32 = 13.0;
/// The height of a panel's title row.
pub(crate) const TITLE_ROW: f32 = ROW;

/// Sets egui's own widgets to the kit's spacing, row height and corners, so a stock button,
/// field or slider matches the kit's own controls.
pub(crate) fn apply(style: &mut egui::Style) {
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(ITEM_GAP, ROW_GAP);
    spacing.interact_size.y = ROW;
    spacing.button_padding = egui::vec2(CONTROL_PAD, 2.0);
    let widgets = &mut style.visuals.widgets;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = CONTROL_RADIUS.into();
    }
}

/// A named on/off chip: filled with the accent when on, like a control at rest when off.
/// Clicking it turns it over; the response says `changed` when it did.
pub(crate) fn toggle(
    ui: &mut egui::Ui,
    on: &mut bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let theme = theme::current();
    let galley = text.into().into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = egui::vec2(galley.size().x + 2.0 * CONTROL_PAD, ROW);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let (fill, ink) = chip_colours(*on, response.hovered(), theme);
    ui.painter().rect_filled(rect, CONTROL_RADIUS, fill);
    let corner = rect.center() - galley.size() / 2.0;
    ui.painter().galley(corner, galley, ink);
    let shown = *on;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, shown, ""));
    response
}

/// A chip's fill and ink: the accent and white when on; the hover or rest colour when off.
fn chip_colours(on: bool, hovered: bool, theme: &theme::Theme) -> (egui::Color32, egui::Color32) {
    match (on, hovered) {
        (true, false) => (theme::ACCENT, egui::Color32::WHITE),
        (true, true) => (theme::ACCENT.gamma_multiply(0.85), egui::Color32::WHITE),
        (false, false) => (theme.control, theme.text),
        (false, true) => (theme.hover, theme.text),
    }
}

/// A run of chips side by side, [`CHIP_GAP`] apart, wrapping when the row is full.
pub(crate) fn chip_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = CHIP_GAP;
        add(ui)
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kit_sets_egui_to_its_spacing_and_corners() {
        let mut style = egui::Style::default();
        apply(&mut style);
        assert_eq!(style.spacing.item_spacing, egui::vec2(ITEM_GAP, ROW_GAP));
        assert_eq!(style.spacing.interact_size.y, ROW);
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            CONTROL_RADIUS.into()
        );
    }

    #[test]
    fn a_chip_shows_on_in_the_accent_and_off_like_a_control() {
        for theme in [theme::DARK, theme::LIGHT] {
            assert_eq!(chip_colours(true, false, &theme).0, theme::ACCENT);
            assert_eq!(chip_colours(false, false, &theme).0, theme.control);
            assert_eq!(chip_colours(false, true, &theme).0, theme.hover);
        }
    }
}
