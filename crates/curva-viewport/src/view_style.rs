//! What the drawing code honours of a view's style ([`curva_document::style::ViewStyle`],
//! `docs/specs/views-and-styles.md` §2): [`Style::of`] reads it. More of the style's settings are
//! drawn as the drawing code learns them.

use crate::{EDGE_ON_FACES_COLOUR, INK_COLOUR, PAPER_COLOUR};
use curva_document::style::{self, ColourFrom, Engine, OcclusionLight, VectorFaces, ViewStyle};

/// Outlines wider than this, in millimetres, are drawn thick (three pixels), as Massing's are.
const THICK_OUTLINE_MM: f32 = 0.5;

/// How a view draws the model: its surfaces, the model's lines, the outlines of shapes, and the
/// lines surfaces hide. Every display mode is one of these ([`Style::WIREFRAME`] and the others),
/// drawn by the same code from its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// The faces of surfaces and solids; `None` draws lines only, as Wireframe does.
    pub surfaces: Option<Surfaces>,
    /// What colour the model's lines (curves and edges) are drawn in.
    pub lines: LineColour,
    /// The outlines of curved shapes against what is behind them, found from the surfaces'
    /// depth (#182); drawn where surfaces hide what is behind them.
    pub outlines: OutlineLook,
    /// Whether the lines surfaces hide are drawn too, dotted, as Rhino 8's Technical mode does.
    pub hidden_lines: bool,
}

/// The Surfaces layer: the faces of surfaces and solids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surfaces {
    /// One colour for every face, or `None` for each object's own colour.
    pub colour: Option<[f32; 3]>,
    /// Whether faces are see-through, so the lines behind them show dimmed, as in Ghosted.
    pub see_through: bool,
    /// How the faces are lit.
    pub light: Light,
}

/// How a style lights its surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    /// The view's own lighting (Studio by default, [`crate::Look`]).
    View,
    /// The sun, with its shadows, as Massing is lit (#551).
    Sun,
    /// Unlit: each face one flat colour, as Rhino 8's Hidden mode draws them (#164).
    Flat,
}

/// What colour a style draws the model's lines in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineColour {
    /// Each line in its object's own colour, as in Shaded.
    Own,
    /// Every line in black ink, as in Hidden; previews and markers keep their colours.
    Ink,
}

/// The Outlines layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineLook {
    /// Whether outlines are drawn.
    pub shown: bool,
    /// The outlines' colour.
    pub colour: [f32; 3],
    /// Whether they are thick, as in Massing, rather than as thin as edges.
    pub thick: bool,
}

impl Style {
    /// Lines only: curves, and the edges of surfaces and solids.
    pub const WIREFRAME: Self = Self {
        surfaces: None,
        lines: LineColour::Own,
        outlines: OutlineLook {
            shown: false,
            ..OutlineLook::EDGES
        },
        hidden_lines: false,
    };
    /// Surfaces and solids filled in their own colours and lit, with their edges on top.
    pub const SHADED: Self = Self {
        surfaces: Some(Surfaces {
            colour: None,
            see_through: false,
            light: Light::View,
        }),
        outlines: OutlineLook::EDGES,
        ..Self::WIREFRAME
    };
    /// Surfaces and solids see-through at half strength, so the edges behind them show.
    pub const GHOSTED: Self = Self {
        surfaces: Some(Surfaces {
            colour: None,
            see_through: true,
            light: Light::View,
        }),
        outlines: OutlineLook::EDGES,
        ..Self::WIREFRAME
    };
    /// Rhino 8's Hidden mode (#164): faces flat in paper white hiding what is behind them, and
    /// the visible lines and outlines in black ink.
    pub const HIDDEN: Self = Self {
        surfaces: Some(Surfaces {
            colour: Some(PAPER_COLOUR),
            see_through: false,
            light: Light::Flat,
        }),
        lines: LineColour::Ink,
        outlines: OutlineLook {
            shown: true,
            colour: INK_COLOUR,
            thick: false,
        },
        hidden_lines: false,
    };
    /// Rhino 8's Technical mode (#164): as Hidden, with the lines faces hide drawn dotted.
    pub const TECHNICAL: Self = Self {
        hidden_lines: true,
        ..Self::HIDDEN
    };
    /// A white study model (#551): white faces lit by the sun, with their shadows, thin black
    /// edges and thick black outlines. Its sky-to-grey background is the caller's.
    pub const MASSING: Self = Self {
        surfaces: Some(Surfaces {
            colour: Some(PAPER_COLOUR),
            see_through: false,
            light: Light::Sun,
        }),
        lines: LineColour::Ink,
        outlines: OutlineLook {
            shown: true,
            colour: INK_COLOUR,
            thick: true,
        },
        hidden_lines: false,
    };

    /// What of `view` the drawing code honours: its NURBS faces and their light, whether its
    /// lines are in ink, its outlines and its hidden lines.
    #[must_use]
    pub fn of(view: &ViewStyle) -> Self {
        let faces = &view.nurbs.faces;
        let light = match view.engine {
            Engine::Vector => match view.lighting.vector {
                VectorFaces::Flat => Light::Flat,
                VectorFaces::Tones => Light::View,
            },
            Engine::Occlusion => match view.lighting.occlusion {
                OcclusionLight::Studio => Light::View,
                OcclusionLight::Sun => Light::Sun,
                OcclusionLight::Flat => Light::Flat,
            },
            Engine::Rendered => Light::View,
        };
        let surfaces = faces.shown.then(|| Surfaces {
            colour: match faces.colour {
                ColourFrom::One(colour) => Some(colour.to_f32()),
                _ => None,
            },
            see_through: faces.opacity < 1.0,
            light,
        });
        // Curves in one colour put every model line in ink; the line drawer has no other
        // single colour for lines yet.
        let lines = match view.curves.colour {
            style::LineColour::One(_) => LineColour::Ink,
            style::LineColour::Object => LineColour::Own,
        };
        Self {
            surfaces,
            lines,
            outlines: OutlineLook {
                shown: view.outlines.shown,
                colour: view.outlines.colour.to_f32(),
                thick: view.outlines.width > THICK_OUTLINE_MM,
            },
            hidden_lines: view.hidden_lines.shown,
        }
    }

    /// Whether faces are drawn solid, hiding what is behind them: shaded and not see-through.
    #[must_use]
    pub fn solid(&self) -> bool {
        self.surfaces.is_some_and(|surfaces| !surfaces.see_through)
    }

    /// Whether faces are drawn see-through, as in Ghosted.
    pub fn see_through(&self) -> bool {
        self.surfaces.is_some_and(|surfaces| surfaces.see_through)
    }

    /// How the faces are lit, if there are any.
    pub fn light(&self) -> Option<Light> {
        self.surfaces.map(|surfaces| surfaces.light)
    }
}

impl Default for Style {
    /// Shaded, as a new view opens.
    fn default() -> Self {
        Self::SHADED
    }
}

impl OutlineLook {
    /// OutlineLook as dark and thin as the edges on faces.
    pub const EDGES: Self = Self {
        shown: true,
        colour: EDGE_ON_FACES_COLOUR,
        thick: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wireframe_draws_no_surfaces() {
        let all = [
            Style::WIREFRAME,
            Style::SHADED,
            Style::GHOSTED,
            Style::HIDDEN,
            Style::TECHNICAL,
            Style::MASSING,
        ];
        let without: Vec<_> = all.iter().filter(|s| s.surfaces.is_none()).collect();
        assert_eq!(without, vec![&Style::WIREFRAME]);
    }

    #[test]
    fn only_ghosted_is_see_through_and_only_technical_shows_hidden_lines() {
        assert!(Style::GHOSTED.see_through() && !Style::GHOSTED.solid());
        assert!(Style::SHADED.solid() && !Style::SHADED.see_through());
        assert!(!Style::WIREFRAME.solid() && !Style::WIREFRAME.see_through());
        let (technical, hidden) = (Style::TECHNICAL, Style::HIDDEN);
        assert!(technical.hidden_lines && !hidden.hidden_lines);
        assert_eq!(Style::HIDDEN.light(), Some(Light::Flat));
        assert_eq!(Style::MASSING.light(), Some(Light::Sun));
        assert_eq!(Style::default(), Style::SHADED);
    }

    #[test]
    fn each_built_in_style_draws_as_its_display_mode_did() {
        let pairs = [
            (ViewStyle::wireframe(), Style::WIREFRAME),
            (ViewStyle::shaded(), Style::SHADED),
            (ViewStyle::ghosted(), Style::GHOSTED),
            (ViewStyle::hidden(), Style::HIDDEN),
            (ViewStyle::technical(), Style::TECHNICAL),
            (ViewStyle::massing(), Style::MASSING),
        ];
        for (view, drawn) in pairs {
            assert_eq!(Style::of(&view), drawn, "{}", view.name);
        }
    }

    #[test]
    fn a_style_s_own_settings_reach_the_drawing() {
        let mut view = ViewStyle::shaded();
        view.nurbs.faces.colour = ColourFrom::One(style::Colour([255, 0, 0]));
        view.nurbs.faces.opacity = 0.7;
        view.outlines.shown = false;
        view.outlines.width = 1.0;
        let drawn = Style::of(&view);
        let surfaces = drawn.surfaces.expect("faces shown");
        assert_eq!(surfaces.colour, Some([1.0, 0.0, 0.0]));
        assert!(surfaces.see_through);
        assert!(!drawn.outlines.shown && drawn.outlines.thick);
        view.engine = Engine::Vector;
        assert_eq!(Style::of(&view).light(), Some(Light::View), "tones are lit");
        view.lighting.vector = VectorFaces::Flat;
        assert_eq!(Style::of(&view).light(), Some(Light::Flat));
    }
}
