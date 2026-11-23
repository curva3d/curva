//! The display modes as visualisation styles (`docs/specs/views-and-styles.md` §2.3): each mode
//! is a built-in [`ViewStyle`], drawn from its settings.

use crate::theme;
use curva_commands::DisplayMode;
use curva_document::style::{Fill, ViewStyle};
use curva_viewport::Style;
use eframe::egui;
use std::sync::LazyLock;

/// The built-in style display mode `mode` draws with.
pub(crate) fn mode_view_style(mode: DisplayMode) -> ViewStyle {
    match mode {
        DisplayMode::Wireframe => ViewStyle::wireframe(),
        DisplayMode::Shaded => ViewStyle::shaded(),
        DisplayMode::Ghosted => ViewStyle::ghosted(),
        DisplayMode::Hidden => ViewStyle::hidden(),
        DisplayMode::Technical => ViewStyle::technical(),
        DisplayMode::Massing => ViewStyle::massing(),
    }
}

/// What the drawing code honours of each display mode's style, in [`crate::grid::DISPLAY_MODES`]
/// order, with each built-in style, worked out once.
static BUILT: LazyLock<Vec<(ViewStyle, Style)>> = LazyLock::new(|| {
    crate::grid::DISPLAY_MODES
        .iter()
        .map(|&mode| {
            let style = mode_view_style(mode);
            let drawn = Style::of(&style);
            (style, drawn)
        })
        .collect()
});

/// Where display mode `mode` is in [`BUILT`].
fn index_of(mode: DisplayMode) -> usize {
    crate::grid::DISPLAY_MODES
        .iter()
        .position(|&m| m == mode)
        .unwrap_or(1)
}

/// The built-in style display mode `mode` draws with, made once.
pub(crate) fn built_in_style(mode: DisplayMode) -> &'static ViewStyle {
    &BUILT[index_of(mode)].0
}

/// How display mode `mode` is drawn.
pub(crate) fn mode_style(mode: DisplayMode) -> Style {
    BUILT[index_of(mode)].1
}

/// The built-in style a detail's display mode, saved by name (`"Ghosted"`), draws with; Shaded
/// for a name it does not know.
pub(crate) fn named_style(name: &str) -> Style {
    crate::grid::DISPLAY_MODES
        .iter()
        .find(|&&mode| format!("{mode:?}") == name)
        .map_or_else(|| mode_style(DisplayMode::Shaded), |&mode| mode_style(mode))
}

/// A style's background, shading from the first colour at the top to the second at the bottom:
/// the window theme's for a style that follows the theme.
pub(crate) fn style_background(style: &ViewStyle) -> (egui::Color32, egui::Color32) {
    let colour = |c: curva_document::style::Colour| {
        let [r, g, b] = c.0;
        egui::Color32::from_rgb(r, g, b)
    };
    let background = &style.background;
    match background.fill {
        Fill::Theme => (theme::current().window, theme::current().window_low),
        Fill::Gradient => (colour(background.top), colour(background.bottom)),
        Fill::OneColour | Fill::Paper => (colour(background.top), colour(background.top)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_detail_draws_with_the_style_of_the_mode_it_is_named_after() {
        for mode in crate::grid::DISPLAY_MODES {
            assert_eq!(
                named_style(&format!("{mode:?}")),
                mode_style(mode),
                "{mode:?}"
            );
        }
        assert_eq!(named_style("Rendered"), Style::shaded());
    }

    #[test]
    fn each_mode_draws_with_the_built_in_style_of_its_name() {
        for mode in crate::grid::DISPLAY_MODES {
            let style = mode_view_style(mode);
            assert_eq!(style.name, format!("{mode:?}"));
            assert_eq!(Some(style), ViewStyle::built_in(&format!("{mode:?}")));
        }
    }

    #[test]
    fn hidden_and_massing_keep_their_backgrounds() {
        let white = egui::Color32::WHITE;
        assert_eq!(style_background(&ViewStyle::hidden()), (white, white));
        assert_eq!(
            style_background(&ViewStyle::massing()),
            crate::sheets::MASSING_SKY
        );
    }
}
