//! What the drawing code honours of a view's style ([`curva_document::style::ViewStyle`],
//! `docs/specs/views-and-styles.md` §2): [`Style::of`] reads it. More of the style's settings are
//! drawn as the drawing code learns them.

use crate::{EDGE_ON_FACES_COLOUR, LineTable};
use curva_document::style::{ColourFrom, Engine, OcclusionLight, VectorFaces, ViewStyle};

/// Outlines wider than this, in millimetres, are drawn thick (three pixels), as Massing's are.
const THICK_OUTLINE_MM: f32 = 0.5;

/// How a view draws the model: its surfaces, each kind of line, the outlines of shapes, and the
/// lines surfaces hide. Every display mode is one ([`Style::wireframe`] and the others), drawn
/// by the same code from its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// The faces of surfaces and solids; `None` draws lines only, as Wireframe does.
    pub surfaces: Option<Surfaces>,
    /// Each kind of line's colour, width and whether it shows, and hidden lines'.
    pub lines: LineTable,
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
    /// What of `view` the drawing code honours: its NURBS faces and their light, each kind of
    /// line, its outlines and its hidden lines.
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
        Self {
            surfaces,
            lines: LineTable::from_style(view),
            outlines: OutlineLook {
                shown: view.outlines.shown,
                colour: view.outlines.colour.to_f32(),
                thick: view.outlines.width > THICK_OUTLINE_MM,
            },
            hidden_lines: view.hidden_lines.shown,
        }
    }

    /// Lines only: curves, and the edges of surfaces and solids.
    #[must_use]
    pub fn wireframe() -> Self {
        Self::of(&ViewStyle::wireframe())
    }

    /// Surfaces and solids in their own colours and lit, with dark edges on top.
    #[must_use]
    pub fn shaded() -> Self {
        Self::of(&ViewStyle::shaded())
    }

    /// Surfaces and solids see-through at half strength, so the edges behind them show.
    #[must_use]
    pub fn ghosted() -> Self {
        Self::of(&ViewStyle::ghosted())
    }

    /// Rhino 8's Hidden mode (#164): faces flat in paper white, lines in black ink.
    #[must_use]
    pub fn hidden() -> Self {
        Self::of(&ViewStyle::hidden())
    }

    /// Rhino 8's Technical mode (#164): as Hidden, with the lines faces hide drawn dotted.
    #[must_use]
    pub fn technical() -> Self {
        Self::of(&ViewStyle::technical())
    }

    /// A white study model (#551), lit by the sun, with thick black outlines.
    #[must_use]
    pub fn massing() -> Self {
        Self::of(&ViewStyle::massing())
    }

    /// Whether faces are drawn solid, hiding what is behind them: shaded and not see-through.
    #[must_use]
    pub fn solid(&self) -> bool {
        self.surfaces.is_some_and(|surfaces| !surfaces.see_through)
    }

    /// Whether faces are drawn see-through, as in Ghosted.
    #[must_use]
    pub fn see_through(&self) -> bool {
        self.surfaces.is_some_and(|surfaces| surfaces.see_through)
    }

    /// How the faces are lit, if there are any.
    #[must_use]
    pub fn light(&self) -> Option<Light> {
        self.surfaces.map(|surfaces| surfaces.light)
    }
}

impl Default for Style {
    /// Shaded, as a new view opens.
    fn default() -> Self {
        Self::shaded()
    }
}

impl OutlineLook {
    /// Outlines as dark and thin as the edges on faces.
    pub const EDGES: Self = Self {
        shown: true,
        colour: EDGE_ON_FACES_COLOUR,
        thick: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{INK_COLOUR, PAPER_COLOUR};
    use curva_document::style;

    #[test]
    fn only_wireframe_draws_no_surfaces() {
        let all = [
            Style::wireframe(),
            Style::shaded(),
            Style::ghosted(),
            Style::hidden(),
            Style::technical(),
            Style::massing(),
        ];
        let without: Vec<_> = all.iter().filter(|s| s.surfaces.is_none()).collect();
        assert_eq!(without, vec![&Style::wireframe()]);
    }

    #[test]
    fn only_ghosted_is_see_through_and_only_technical_shows_hidden_lines() {
        assert!(Style::ghosted().see_through() && !Style::ghosted().solid());
        assert!(Style::shaded().solid() && !Style::shaded().see_through());
        assert!(!Style::wireframe().solid() && !Style::wireframe().see_through());
        assert!(Style::technical().hidden_lines && !Style::hidden().hidden_lines);
        assert_eq!(Style::hidden().light(), Some(Light::Flat));
        assert_eq!(Style::massing().light(), Some(Light::Sun));
        assert_eq!(Style::default(), Style::shaded());
    }

    #[test]
    fn each_built_in_style_draws_its_faces_and_outlines_as_its_display_mode_did() {
        let paper = Some(Surfaces {
            colour: Some(PAPER_COLOUR),
            see_through: false,
            light: Light::Flat,
        });
        assert_eq!(Style::hidden().surfaces, paper);
        let ink = OutlineLook {
            shown: true,
            colour: INK_COLOUR,
            thick: false,
        };
        assert_eq!(Style::hidden().outlines, ink);
        assert_eq!(
            Style::massing().outlines,
            OutlineLook { thick: true, ..ink }
        );
        assert_eq!(Style::shaded().outlines, OutlineLook::EDGES);
        assert!(!Style::wireframe().outlines.shown);
        let massing = Style::massing().surfaces.expect("faces");
        assert_eq!(
            (massing.colour, massing.light),
            (Some(PAPER_COLOUR), Light::Sun)
        );
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
