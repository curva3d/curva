//! The display modes as visualisation styles (`docs/specs/views-and-styles.md` §2.3): each mode
//! is a built-in style, drawn from its settings.

use curva_commands::DisplayMode;
use curva_viewport::Style;

/// The built-in style display mode `mode` draws with.
pub(crate) fn mode_style(mode: DisplayMode) -> Style {
    match mode {
        DisplayMode::Wireframe => Style::WIREFRAME,
        DisplayMode::Shaded => Style::SHADED,
        DisplayMode::Ghosted => Style::GHOSTED,
        DisplayMode::Hidden => Style::HIDDEN,
        DisplayMode::Technical => Style::TECHNICAL,
        DisplayMode::Massing => Style::MASSING,
    }
}

/// The built-in style a detail's display mode, saved by name (`"Ghosted"`), draws with; Shaded
/// for a name it does not know.
pub(crate) fn named_style(name: &str) -> Style {
    match name {
        "Wireframe" => Style::WIREFRAME,
        "Ghosted" => Style::GHOSTED,
        "Hidden" => Style::HIDDEN,
        "Technical" => Style::TECHNICAL,
        "Massing" => Style::MASSING,
        _ => Style::SHADED,
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
        assert_eq!(named_style("Rendered"), Style::SHADED);
    }
}
