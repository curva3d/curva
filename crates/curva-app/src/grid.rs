//! The viewports' grid: spaced for the model's unit as Rhino 8's templates space it, and coloured
//! for each display mode, in the colour chosen for that mode in Settings if one was.

use crate::*;
use curva_document::Unit;
use curva_viewport::{GridLook, Palette};
use serde::{Deserialize, Serialize};

/// The display modes, in the order the Display panel and Settings list them.
pub(crate) const DISPLAY_MODES: [DisplayMode; 6] = [
    DisplayMode::Wireframe,
    DisplayMode::Shaded,
    DisplayMode::Ghosted,
    DisplayMode::Hidden,
    DisplayMode::Technical,
    DisplayMode::Massing,
];

/// The words that name a display mode.
pub(crate) const fn mode_key(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Wireframe => "ui.display.wireframe",
        DisplayMode::Shaded => "ui.display.shaded",
        DisplayMode::Ghosted => "ui.display.ghosted",
        DisplayMode::Hidden => "ui.display.hidden",
        DisplayMode::Technical => "ui.display.technical",
        DisplayMode::Massing => "ui.display.massing",
    }
}

/// The grid colour chosen in Settings for each display mode; `None` keeps the standard one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridColours {
    pub wireframe: Option<[u8; 3]>,
    pub shaded: Option<[u8; 3]>,
    pub ghosted: Option<[u8; 3]>,
    pub hidden: Option<[u8; 3]>,
    pub technical: Option<[u8; 3]>,
    pub massing: Option<[u8; 3]>,
}

impl GridColours {
    /// The colour chosen for `mode`, to read or change.
    pub(crate) fn of(&mut self, mode: DisplayMode) -> &mut Option<[u8; 3]> {
        match mode {
            DisplayMode::Wireframe => &mut self.wireframe,
            DisplayMode::Shaded => &mut self.shaded,
            DisplayMode::Ghosted => &mut self.ghosted,
            DisplayMode::Hidden => &mut self.hidden,
            DisplayMode::Technical => &mut self.technical,
            DisplayMode::Massing => &mut self.massing,
        }
    }
}

/// Units between two grid lines, and every how many lines a major one comes, for `unit`, as
/// Rhino 8's "Large Objects" templates set them: a line every 10 mm in millimetres and every unit
/// in metres, feet and inches, with a major one every 10 lines, or every 12 in inches (a foot).
pub(crate) const fn spacing(unit: Unit) -> (f64, i32) {
    match unit {
        Unit::Millimetres => (10.0, 10),
        Unit::Metres | Unit::Feet => (1.0, 10),
        Unit::Inches => (1.0, 12),
    }
}

/// A colour in 0 to 1 from one in 0 to 255.
fn unit_rgb(colour: egui::Color32) -> [f32; 3] {
    [colour.r(), colour.g(), colour.b()].map(|c| f32::from(c) / 255.0)
}

/// The middle of a background shading from `top` to `low`.
fn middle(top: egui::Color32, low: egui::Color32) -> [f32; 3] {
    let [a, b] = [unit_rgb(top), unit_rgb(low)];
    [0, 1, 2].map(|i| (a[i] + b[i]) / 2.0)
}

/// The grid's standard colours over a view's background, shading from `top` to `low`: the
/// theme's own over the theme's background, the light theme's over any other light one (white
/// paper, the Massing sky), so the grid reads faintly on each.
pub(crate) fn standard_palette(top: egui::Color32, low: egui::Color32) -> Palette {
    let theme = theme::current();
    let middle = middle(top, low);
    let light = middle.iter().sum::<f32>() / 3.0 > 0.5;
    if (top, low) == (theme.window, theme.window_low) || !light {
        theme.viewport
    } else {
        Palette::light_over(middle)
    }
}

impl Curva {
    /// How viewport `pane` draws its grid: spaced for the model's unit, in the colour chosen for
    /// its display mode if one was, else in the standard colours over its background.
    pub(crate) fn grid_look(&self, pane: usize) -> GridLook {
        let (top, low) = self.pane_background(pane);
        let palette = standard_palette(top, low);
        let mut colours = self.settings.grid_colours;
        let palette = match *colours.of(self.panes[pane].display) {
            Some([r, g, b]) => {
                palette.with_grid(unit_rgb(egui::Color32::from_rgb(r, g, b)), middle(top, low))
            }
            None => palette,
        };
        let (spacing, major_every) = spacing(self.doc.state().unit);
        GridLook::new(palette).spaced(spacing, major_every)
    }

    /// Settings → General: the grid's colour in each display mode, with a way back to the
    /// standard one.
    pub(crate) fn grid_colour_settings(&mut self, ui: &mut egui::Ui) {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let header = say("ui.settings.grid_colours");
        let standard_label = say("ui.settings.grid_standard");
        let names = DISPLAY_MODES.map(|mode| say(mode_key(mode)));
        let standards = DISPLAY_MODES.map(|mode| {
            let (top, low) = mode_background(mode);
            standard_palette(top, low)
                .major
                .map(|c| (c * 255.0).round() as u8)
        });
        let colours = &mut self.settings.grid_colours;
        egui::CollapsingHeader::new(header).show(ui, |ui| {
            egui::Grid::new("grid colours").show(ui, |ui| {
                for ((mode, name), standard) in DISPLAY_MODES.into_iter().zip(names).zip(standards)
                {
                    ui.label(name);
                    let chosen = colours.of(mode);
                    let mut colour = chosen.unwrap_or(standard);
                    if ui.color_edit_button_srgb(&mut colour).changed() {
                        *chosen = Some(colour);
                    }
                    if chosen.is_some() && ui.button(&standard_label).clicked() {
                        *chosen = None;
                    }
                    ui.end_row();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_is_spaced_as_rhinos_large_objects_templates() {
        assert_eq!(spacing(Unit::Millimetres), (10.0, 10));
        assert_eq!(spacing(Unit::Metres), (1.0, 10));
        assert_eq!(spacing(Unit::Feet), (1.0, 10));
        assert_eq!(spacing(Unit::Inches), (1.0, 12));
    }

    #[test]
    fn a_grid_on_white_paper_is_faint_dark_lines() {
        let white = egui::Color32::WHITE;
        let palette = standard_palette(white, white);
        assert!(palette.minor.iter().all(|c| *c > 0.9 && *c < 1.0));
        assert!(palette.major.iter().all(|c| *c < palette.minor[0]));
    }

    #[test]
    fn each_display_mode_keeps_its_own_grid_colour() {
        let mut colours = GridColours::default();
        *colours.of(DisplayMode::Hidden) = Some([200, 0, 0]);
        assert_eq!(*colours.of(DisplayMode::Hidden), Some([200, 0, 0]));
        assert_eq!(*colours.of(DisplayMode::Shaded), None);
        let text = serde_json::to_string(&colours).unwrap();
        assert_eq!(serde_json::from_str::<GridColours>(&text).unwrap(), colours);
    }
}
