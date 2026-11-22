//! The UI kit: the one library every part of Curva's interface is drawn with (owner, 22 November
//! 2026: "I don't want custom code on the UI, all of it needs to be a library that is utilized").
//! Panels say what they show (a section, a chip, a dropdown, tabs) and the kit decides how it
//! looks, so changing the kit changes the whole interface, and a new panel or specialization
//! looks like the rest without trying. A test (`panel_code_uses_the_ui_kit`) fails the build if
//! code outside the kit paints, picks fonts or colours, or sizes spacing itself.
//!
//! The owner's choices, from mock-ups: panel titles quiet and centred; on/off as filled chips;
//! compact rows, as in Rhino.
//!
//! The rules:
//! - Inside every panel, [`PAD`] on all four sides.
//! - [`ROW_GAP`] between rows, [`ITEM_GAP`] between things in a row, [`CHIP_GAP`] between chips.
//! - Buttons, fields and chips are [`ROW`] tall, with [`CONTROL_RADIUS`] corners.
//! - A panel's title is centred and quiet; all other text starts on the panel's left edge.
//! - Text comes in four sizes only: [`TITLE_FONT`], [`BODY_FONT`], [`SMALL_FONT`] and
//!   [`CAPTION_FONT`].
//! - Colours come from the theme by role ([`colours`]), never picked by hand.
//! - Anything with a name that turns on and off is a [`toggle`] chip; a bare tick in a table
//!   cell, with no name beside it, stays a tick box.
//! - Icons are centred on their ink, across and down (`icons::paint` does this).

use eframe::egui;

pub(crate) mod colours;
mod containers;
mod controls;
pub(crate) mod glow;
mod panel_parts;
mod shapes;
mod table;
mod text;

pub(crate) use containers::*;
pub(crate) use controls::*;
pub(crate) use panel_parts::*;
pub(crate) use shapes::*;
pub(crate) use table::*;
pub(crate) use text::*;

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
/// Between tight things in a row: toolbar buttons, a tree's arrow and its name.
pub(crate) const TIGHT_GAP: f32 = 2.0;
/// Between groups in a row, such as the command tab groups.
pub(crate) const GROUP_GAP: f32 = 16.0;
/// The corners of buttons, fields and chips.
pub(crate) const CONTROL_RADIUS: u8 = 6;
/// The corners of a pill holding controls, such as the specializations.
pub(crate) const PILL_RADIUS: u8 = 9;
/// The corners of panels.
pub(crate) const PANEL_RADIUS: u8 = 12;
/// Room either side of a chip's or button's name.
pub(crate) const CONTROL_PAD: f32 = 8.0;
/// The height of a panel's title row.
pub(crate) const TITLE_ROW: f32 = ROW;
/// The size of a small square icon button, as in a panel's toolbar.
pub(crate) const TOOL_BUTTON: f32 = 22.0;
/// The size of a command button with an icon.
pub(crate) const COMMAND_BUTTON: f32 = 30.0;
/// How far a fade runs in from the edge of a strip that scrolls.
pub(crate) const FADE: f32 = 28.0;
/// The width of a line: edges, outlines.
pub(crate) const LINE: f32 = 1.0;
/// The width of drawn marks: chevrons, crosses, glows.
pub(crate) const MARK: f32 = 1.5;
/// The width of the underline under the chosen tab.
pub(crate) const UNDERLINE: f32 = 2.0;

/// Panel titles, the same on every panel.
pub(crate) const TITLE_FONT: f32 = 13.0;
/// Body text, buttons and fields.
pub(crate) const BODY_FONT: f32 = 14.0;
/// Smaller text: group names, file names.
pub(crate) const SMALL_FONT: f32 = 12.0;
/// The smallest text: captions, table headers, the version.
pub(crate) const CAPTION_FONT: f32 = 11.0;
/// Headings, on the About page only.
pub(crate) const HEADING_FONT: f32 = 18.0;
/// The command history, so typed values line up.
pub(crate) const MONO_FONT: f32 = 13.0;

/// Sets egui's own widgets to the kit's text sizes, spacing, row height and corners, so a stock
/// button, field or slider matches the kit's own controls. Run after the theme's colours.
pub(crate) fn apply(style: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    for (text_style, size, family) in [
        (TextStyle::Body, BODY_FONT, FontFamily::Proportional),
        (TextStyle::Button, BODY_FONT, FontFamily::Proportional),
        (TextStyle::Small, SMALL_FONT, FontFamily::Proportional),
        (TextStyle::Heading, HEADING_FONT, FontFamily::Proportional),
        (TextStyle::Monospace, MONO_FONT, FontFamily::Monospace),
    ] {
        style
            .text_styles
            .insert(text_style, FontId::new(size, family));
    }
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

/// Puts the theme in use on every part of the interface: its colours, then the kit's sizes.
pub(crate) fn apply_theme(ctx: &egui::Context) {
    ctx.set_visuals(crate::theme::visuals(crate::theme::current()));
    ctx.all_styles_mut(apply);
}

/// Runs `add` with `gap` between things side by side, the kit's gap again after.
pub(crate) fn with_gap<R>(ui: &mut egui::Ui, gap: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let before = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap;
    let out = add(ui);
    ui.spacing_mut().item_spacing.x = before;
    out
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
        assert_eq!(style.text_styles[&egui::TextStyle::Body].size, BODY_FONT);
    }

    /// What only the kit may do: paint, pick fonts, colours, corners and frames, set spacing,
    /// or build scroll areas, dropdowns and tabs of its own.
    const KIT_ONLY: [&str; 22] = [
        ".painter()",
        "painter_at(",
        "layer_painter(",
        "RichText::",
        "FontId::",
        "Color32::",
        "gamma_multiply",
        "egui::Shape::",
        "epaint::",
        "Frame::",
        "ScrollArea::",
        "ComboBox::",
        "ui.heading(",
        "selectable_label(",
        "spacing_mut()",
        "theme::current()",
        "theme::ACCENT",
        "CornerRadius",
        "Stroke::new(",
        "rect_filled(",
        ".checkbox(",
        "style_mut()",
    ];

    /// The files that may: the kit, the icon library, the theme, and what is drawn in the 3D
    /// view rather than on a panel (the gumball, control points, the selection window, dots, the grid,
    /// picked parts, the viewports, the sheets of a layout and the printed page's preview).
    fn may_paint(path: &str) -> bool {
        path.starts_with("ui_kit/")
            || path.starts_with("icons")
            || [
                "theme.rs",
                "gumball.rs",
                "grips.rs",
                "mouse.rs",
                "dots.rs",
                "part_picking.rs",
                "viewports.rs",
                "sheets.rs",
                "grid.rs",
                "panels/print_preview.rs",
            ]
            .contains(&path)
    }

    /// Every source file under `dir`, with its path from `src`.
    fn sources(src: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(src, &path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let name = path
                    .strip_prefix(src)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }

    /// The owner's rule (22 November 2026): no custom code on the UI; every panel uses the kit.
    /// Test code is not counted.
    #[test]
    fn panel_code_uses_the_ui_kit() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&src, &src, &mut files);
        let mut found = Vec::new();
        for (path, text) in files.iter().filter(|(path, _)| !may_paint(path)) {
            let code = text.split("\n#[cfg(test)]").next().unwrap_or(text);
            for (number, line) in code.lines().enumerate() {
                let line = line.split("//").next().unwrap_or(line);
                for pattern in KIT_ONLY.iter().filter(|p| line.contains(*p)) {
                    found.push(format!("{path}:{}: {pattern}", number + 1));
                }
            }
        }
        assert!(
            found.is_empty(),
            "drawn without the UI kit:\n{}",
            found.join("\n")
        );
    }
}
