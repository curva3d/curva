//! The display modes as visualisation styles (`docs/specs/views-and-styles.md` §2.3): each mode
//! is a built-in [`ViewStyle`], drawn from its settings.

use crate::Curva;
use crate::theme;
use crate::viewports::Pane;
use curva_commands::DisplayMode;
use curva_document::style::{Fill, StyleError, ViewStyle};
use curva_viewport::Style;
use eframe::egui;
use std::borrow::Cow;
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

/// The user's own styles ("My styles", `views-and-styles.md` §2.4): the `styles` folder in
/// Curva's own folder in the user's app data. `None` where that folder is unknown.
pub(crate) fn library() -> Option<curva_document::style::Library> {
    crate::recovery::app_folder()
        .map(|folder| curva_document::style::Library::at(folder.join("styles")))
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

/// The display mode whose built-in style is called `name`, ignoring case.
fn mode_named(name: &str) -> Option<DisplayMode> {
    let name = curva_document::style::built_in_name(name)?;
    crate::grid::DISPLAY_MODES
        .iter()
        .copied()
        .find(|mode| format!("{mode:?}") == name)
}

impl Pane {
    /// Draws with display mode `mode`'s built-in style, forgetting a style chosen by name.
    pub(crate) fn set_display(&mut self, mode: DisplayMode) {
        self.display = mode;
        self.style = None;
    }

    /// Draws with the style called `name`. A built-in one sets the display mode of its name; any
    /// other is the model's, looked up each time the viewport is drawn, so its edits show at once
    /// ([`Curva::pane_style`]).
    pub(crate) fn set_style(&mut self, name: &str) {
        match mode_named(name) {
            Some(mode) => self.set_display(mode),
            None => self.style = Some(name.trim().to_owned()),
        }
    }

    /// The name of the style it draws with.
    pub(crate) fn style_name(&self) -> String {
        self.style
            .clone()
            .unwrap_or_else(|| format!("{:?}", self.display))
    }

    /// How it draws, for a viewport put in its place to take up ([`Pane::set_drawn`]).
    pub(crate) fn drawn(&self) -> (DisplayMode, Option<String>) {
        (self.display, self.style.clone())
    }

    /// Draws as another viewport did ([`Pane::drawn`]).
    pub(crate) fn set_drawn(&mut self, (display, style): (DisplayMode, Option<String>)) {
        self.display = display;
        self.style = style;
    }
}

/// The style `pane` draws with in `doc`: the model's style it names, or else its display mode's
/// built-in style, as when the style it named has been deleted.
fn style_of<'d>(doc: &'d curva_document::Document, pane: &Pane) -> Cow<'d, ViewStyle> {
    pane.style
        .as_deref()
        .and_then(|name| doc.style_named(name))
        .unwrap_or_else(|| Cow::Borrowed(built_in_style(pane.display)))
}

/// How `pane` draws in `doc` ([`Curva::pane_style`]).
fn drawn_style(doc: &curva_document::Document, pane: &Pane) -> Style {
    if pane.style.is_none() {
        return mode_style(pane.display);
    }
    Style::of(&style_of(doc, pane))
}

impl Curva {
    /// The style viewport `pane` draws with: the model's style it names, or else its display
    /// mode's built-in style, as when the style it named has been deleted.
    pub(crate) fn pane_view_style(&self, pane: usize) -> Cow<'_, ViewStyle> {
        style_of(&self.doc, &self.panes[pane])
    }

    /// How viewport `pane` draws. A style of the model's is worked out from it each frame, which
    /// takes far less than drawing, so editing it in the model shows at once; display modes'
    /// built-in styles are worked out once.
    pub(crate) fn pane_style(&self, pane: usize) -> Style {
        drawn_style(&self.doc, &self.panes[pane])
    }

    /// Makes viewport `pane` draw with the style called `name`: the model's own, or a built-in
    /// one.
    pub(crate) fn set_pane_style(&mut self, pane: usize, name: &str) -> Result<(), StyleError> {
        if self.doc.style_named(name).is_none() {
            return Err(StyleError::Missing(name.trim().to_owned()));
        }
        self.panes[pane].set_style(name);
        Ok(())
    }

    /// Renames the model's style `from` to `to`; the viewports and saved views drawing with it
    /// follow.
    pub(crate) fn rename_style(&mut self, from: &str, to: &str) -> Result<(), StyleError> {
        self.doc.rename_style(from, to)?;
        for pane in &mut self.panes {
            if pane
                .style
                .as_deref()
                .is_some_and(|name| name.trim().eq_ignore_ascii_case(from.trim()))
            {
                pane.style = Some(to.trim().to_owned());
            }
        }
        Ok(())
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

    /// A copy of Shaded called `name`, its NURBS faces in `colour`.
    fn model_style(name: &str, colour: [u8; 3]) -> ViewStyle {
        let mut style = ViewStyle::shaded();
        style.name = name.to_owned();
        style.nurbs.faces.colour =
            curva_document::style::ColourFrom::One(curva_document::style::Colour(colour));
        style
    }

    #[test]
    fn a_viewport_takes_a_built_in_style_as_its_display_mode_and_others_by_name() {
        let mut pane = Pane::new(curva_viewport::StandardView::Top);
        pane.set_style("ghosted");
        assert_eq!(
            (pane.display, pane.style.clone()),
            (DisplayMode::Ghosted, None)
        );
        assert_eq!(pane.style_name(), "Ghosted");
        pane.set_style(" Diagram ");
        assert_eq!(pane.style.as_deref(), Some("Diagram"));
        assert_eq!(pane.style_name(), "Diagram");
        let drawn = pane.drawn();
        let mut other = Pane::new(curva_viewport::StandardView::Front);
        other.set_drawn(drawn);
        assert_eq!(
            other.style_name(),
            "Diagram",
            "a view put in its place keeps it"
        );
        pane.set_display(DisplayMode::Hidden);
        assert_eq!(
            pane.style, None,
            "choosing a display mode forgets the style"
        );
    }

    #[test]
    fn a_viewport_shows_its_model_style_s_edits_at_once_and_falls_back_when_it_is_deleted() {
        let mut doc = curva_document::Document::default();
        doc.save_style(model_style("Diagram", [0, 0, 255])).unwrap();
        let mut pane = Pane::new(curva_viewport::StandardView::Perspective);
        pane.set_style("Diagram");
        assert_eq!(style_of(&doc, &pane).name, "Diagram");
        let blue = drawn_style(&doc, &pane);
        doc.save_style(model_style("Diagram", [255, 0, 0])).unwrap();
        let red = drawn_style(&doc, &pane);
        assert_ne!(blue, red, "the edit shows on the next frame");
        assert_eq!(red.faces[0].colour, Some([1.0, 0.0, 0.0]));
        assert!(doc.delete_style("Diagram"));
        assert_eq!(
            style_of(&doc, &pane).name,
            "Shaded",
            "its display mode's style"
        );
        assert_eq!(drawn_style(&doc, &pane), Style::shaded());
    }
}
