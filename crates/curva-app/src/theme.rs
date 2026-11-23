//! The two themes, and the settings that choose between them (spec
//! `docs/specs/ui-look-and-layout.md` §1): warm greys in the Claude app's manner, one accent,
//! orange `#e0782f`, and Inter for the interface. The theme in use is kept here so every part of
//! the interface paints with it.

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The one accent: selected objects, previews, held snaps, the active tab, ticked toggles,
/// focused fields and highlighted rows.
pub const ACCENT: Color32 = Color32::from_rgb(0xe0, 0x78, 0x2f);

/// Something live and working, such as a connected agent.
pub const LIVE: Color32 = Color32::from_rgb(110, 200, 120);

/// The selection window's edge and fill, over the viewports.
pub const SELECTION_BOX_EDGE: Color32 = Color32::from_rgb(230, 230, 235);
pub const SELECTION_BOX_FILL: Color32 = Color32::from_rgba_premultiplied(30, 40, 60, 40);

/// Behind pictures for agents: the dark grey they had before Bevy drew them.
pub const PICTURE_BACKGROUND: Color32 = Color32::from_rgb(28, 31, 36);

/// White paper, behind the Hidden and Technical display modes' pictures.
pub const PAPER: Color32 = Color32::WHITE;

/// Rhino's Shaded viewport grey, behind the look pictures for comparing with it.
pub const RHINO_GREY: Color32 = Color32::from_rgb(158, 163, 171);

/// One theme's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Whether it is the dark one.
    pub dark: bool,
    /// Behind everything.
    pub window: Color32,
    /// The 3D view's colour at the bottom, which it shades to from `window` at the top.
    pub window_low: Color32,
    /// A solid panel.
    pub panel: Color32,
    /// The tint laid over the blur on a glass panel.
    pub glass: Color32,
    /// A panel's edge.
    pub panel_edge: Color32,
    /// Text.
    pub text: Color32,
    /// Quieter text: titles, hints.
    pub quiet: Color32,
    /// A button or field at rest.
    pub control: Color32,
    /// A button or field under the pointer.
    pub hover: Color32,
    /// The viewport's colours.
    pub viewport: curva_viewport::Palette,
}

/// The dark theme (owner, 23 November 2026, after Apple's frosted glass): deep black frosted
/// panels, a dark see-through tint over a strong blur with a fine light edge, over a warm grey
/// background; controls faint washes of white.
pub const DARK: Theme = Theme {
    dark: true,
    window: Color32::from_rgb(0x26, 0x26, 0x24),
    window_low: Color32::from_rgb(0x1f, 0x1f, 0x1d),
    panel: Color32::from_rgb(0x0e, 0x0e, 0x0e),
    glass: Color32::from_rgba_premultiplied(0x04, 0x04, 0x04, 0x80),
    panel_edge: Color32::from_rgba_premultiplied(0x26, 0x26, 0x26, 0x26),
    text: Color32::from_rgb(0xf4, 0xf4, 0xf2),
    quiet: Color32::from_rgb(0x9a, 0x9a, 0x96),
    control: Color32::from_rgba_premultiplied(0x12, 0x12, 0x12, 0x12),
    hover: Color32::from_rgba_premultiplied(0x22, 0x22, 0x22, 0x22),
    viewport: curva_viewport::Palette::DARK,
};

/// The light theme, after Apple's light frosted glass: white see-through panels over a strong blur
/// and a clean, slightly cool light grey background (Apple's system greys), a fine dark edge,
/// controls faint washes of grey.
pub const LIGHT: Theme = Theme {
    dark: false,
    window: Color32::from_rgb(0xf5, 0xf5, 0xf7),
    window_low: Color32::from_rgb(0xe1, 0xe1, 0xe6),
    panel: Color32::from_rgb(0xfb, 0xfb, 0xfa),
    glass: Color32::from_rgba_premultiplied(0xd9, 0xd9, 0xd9, 0xd9),
    panel_edge: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x1c),
    text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
    quiet: Color32::from_rgb(0x6e, 0x6e, 0x73),
    control: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x0e),
    hover: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x1a),
    viewport: curva_viewport::Palette::LIGHT,
};

/// The dark theme with Increase contrast on, as Apple's accessibility setting: nearly solid
/// panels, firmer edges and controls, and quiet text closer to the rest.
pub const DARK_CONTRAST: Theme = Theme {
    glass: Color32::from_rgba_premultiplied(0x04, 0x04, 0x04, 0xe0),
    panel_edge: Color32::from_rgba_premultiplied(0x73, 0x73, 0x73, 0x73),
    text: Color32::WHITE,
    quiet: Color32::from_rgb(0xc8, 0xc8, 0xc4),
    control: Color32::from_rgba_premultiplied(0x26, 0x26, 0x26, 0x26),
    hover: Color32::from_rgba_premultiplied(0x40, 0x40, 0x40, 0x40),
    ..DARK
};

/// The light theme with Increase contrast on.
pub const LIGHT_CONTRAST: Theme = Theme {
    glass: Color32::from_rgba_premultiplied(0xf6, 0xf6, 0xf6, 0xf6),
    panel_edge: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x59),
    text: Color32::BLACK,
    quiet: Color32::from_rgb(0x3c, 0x3c, 0x40),
    control: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x1c),
    hover: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x30),
    ..LIGHT
};

static LIGHT_ON: AtomicBool = AtomicBool::new(false);
static CONTRAST_ON: AtomicBool = AtomicBool::new(false);

/// The theme in use.
#[must_use]
pub fn current() -> &'static Theme {
    match (
        LIGHT_ON.load(Ordering::Relaxed),
        CONTRAST_ON.load(Ordering::Relaxed),
    ) {
        (true, false) => &LIGHT,
        (true, true) => &LIGHT_CONTRAST,
        (false, false) => &DARK,
        (false, true) => &DARK_CONTRAST,
    }
}

/// Puts the light or dark theme in use, with or without increased contrast.
pub fn use_light(light: bool, contrast: bool) {
    LIGHT_ON.store(light, Ordering::Relaxed);
    CONTRAST_ON.store(contrast, Ordering::Relaxed);
}

/// Which theme to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    /// The system's.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// The user's settings, kept beside the layout in Curva's own folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Which theme.
    pub theme: ThemeChoice,
    /// Frosted glass panels, or solid ones.
    pub glass: bool,
    /// Whether the grid shows.
    pub grid: bool,
    /// The grid's colour chosen for each display mode.
    pub grid_colours: crate::grid::GridColours,
    /// Whether Curva looks for a newer release on GitHub when it opens.
    pub check_updates: bool,
    /// The AI Assistant's connection.
    pub assistant: crate::assistant::Connection,
    /// Undo steps kept whatever they hold, as Rhino's "Minimum undo steps".
    pub undo_min_steps: usize,
    /// Megabytes undo may hold beyond those, as Rhino's "Maximum memory used".
    pub undo_max_megabytes: usize,
    /// Whether Curva, run from an AppImage, has offered to add itself to the applications.
    pub desktop_offered: bool,
    /// The SketchUp import choices last made (issue #432).
    pub sketchup: curva_document::formats::SketchUpImport,
    /// Whether those choices are used without showing the SketchUp import dialog.
    pub sketchup_always: bool,
    /// How large the interface is drawn, in percent: 100 to 200, as Apple's text size.
    pub interface_size: u32,
    /// Firmer edges, nearly solid panels and stronger quiet text, as Apple's Increase contrast.
    pub increased_contrast: bool,
    /// No fades, slides or pulsing glow, as Apple's Reduce motion.
    pub reduce_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            glass: true,
            grid: true,
            grid_colours: crate::grid::GridColours::default(),
            check_updates: true,
            assistant: crate::assistant::Connection::default(),
            undo_min_steps: curva_document::UndoLimits::default().min_steps,
            undo_max_megabytes: curva_document::UndoLimits::default().max_bytes / MEGABYTE,
            desktop_offered: false,
            sketchup: curva_document::formats::SketchUpImport::default(),
            sketchup_always: false,
            interface_size: 100,
            increased_contrast: false,
            reduce_motion: false,
        }
    }
}

/// Bytes in a megabyte, as the undo setting counts them.
pub const MEGABYTE: usize = 1024 * 1024;

impl Settings {
    /// How much undo may keep.
    #[must_use]
    pub fn undo_limits(&self) -> curva_document::UndoLimits {
        curva_document::UndoLimits {
            min_steps: self.undo_min_steps,
            max_bytes: self.undo_max_megabytes.saturating_mul(MEGABYTE),
        }
    }

    /// Whether the light theme is wanted, given what the system uses.
    #[must_use]
    pub fn light(&self, system: Option<egui::Theme>) -> bool {
        match self.theme {
            ThemeChoice::System => system == Some(egui::Theme::Light),
            ThemeChoice::Light => true,
            ThemeChoice::Dark => false,
        }
    }

    /// The settings saved in `file`, or the standard ones.
    #[must_use]
    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Saves the settings to `file`.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(file, text)
    }
}

/// Where the settings are kept.
#[must_use]
pub fn file() -> Option<PathBuf> {
    Some(crate::recovery::app_folder()?.join("settings.json"))
}

/// egui's look for a theme: its greys, text, and the accent for selection and focus.
#[must_use]
pub fn visuals(theme: &Theme) -> egui::Visuals {
    let mut visuals = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = theme.panel;
    visuals.window_fill = theme.panel;
    visuals.window_stroke = egui::Stroke::new(1.0, theme.panel_edge);
    visuals.extreme_bg_color = if theme.dark {
        Color32::from_rgb(0x22, 0x22, 0x20)
    } else {
        Color32::from_rgb(0xff, 0xff, 0xff)
    };
    visuals.faint_bg_color = theme.control;
    visuals.override_text_color = Some(theme.text);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(if theme.dark { 0.55 } else { 0.35 });
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.text_cursor.stroke = egui::Stroke::new(2.0, ACCENT);
    for (widget, fill) in [
        (&mut visuals.widgets.inactive, theme.control),
        (&mut visuals.widgets.hovered, theme.hover),
        (&mut visuals.widgets.active, theme.hover),
        (&mut visuals.widgets.open, theme.hover),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.fg_stroke.color = theme.text;
    }
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.panel_edge);
    visuals.widgets.noninteractive.fg_stroke.color = theme.text;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT.gamma_multiply(0.7));
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    visuals
}

/// The fonts: Inter for the interface, bundled with its licence in `licenses/fonts/`; the
/// standard monospace font stays for the command history, so typed values line up.
#[must_use]
pub fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/Inter-Regular.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".to_owned());
    // Inter Bold, for the specializations, as its own family.
    fonts.font_data.insert(
        "Inter Bold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/Inter-Bold.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Bold".into()),
        vec!["Inter Bold".to_owned(), "Inter".to_owned()],
    );
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_give_every_colour_and_share_the_accent() {
        for theme in [DARK, LIGHT] {
            let visuals = visuals(&theme);
            assert_eq!(visuals.selection.stroke.color, ACCENT);
            assert_eq!(visuals.text_cursor.stroke.color, ACCENT);
            assert_eq!(visuals.override_text_color, Some(theme.text));
            // Text stands out from the panels in both.
            let lightness = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
            assert!(lightness(theme.text).abs_diff(lightness(theme.panel)) > 300);
        }
        assert_ne!(DARK.window, LIGHT.window);
    }

    #[test]
    fn the_setting_or_the_system_chooses_the_theme() {
        let mut settings = Settings::default();
        assert!(settings.light(Some(egui::Theme::Light)));
        assert!(!settings.light(Some(egui::Theme::Dark)));
        settings.theme = ThemeChoice::Dark;
        assert!(!settings.light(Some(egui::Theme::Light)));
        settings.theme = ThemeChoice::Light;
        assert!(settings.light(None));
    }
}
