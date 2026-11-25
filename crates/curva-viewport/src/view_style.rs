//! What the drawing code honours of a view's style ([`curva_document::style::ViewStyle`],
//! `docs/specs/views-and-styles.md` §2): [`Style::of`] reads it. More of the style's settings are
//! drawn as the drawing code learns them.

use crate::{EDGE_ON_FACES_COLOUR, FaceKind, LineTable};
use curva_document::style::{
    ColourFrom, Engine, FacedKind, OcclusionLight, VectorFaces, ViewStyle,
};

/// Rows of the face table ([`Style::face_rows`]): two for each [`FaceKind`].
pub(crate) const FACE_ROWS: usize = 2 * FaceKind::ALL.len();

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
    /// The faces of NURBS, meshes and SubD objects, each as the style sets them apart.
    pub faces: [KindFaces; 3],
    /// How lines thin and fade with distance (spec §2.2, "by distance"); `None` where they do
    /// not.
    pub distance: Option<DistanceFade>,
    /// How many of the model's units make a metre, for `distance`: 1000 for a model in
    /// millimetres, as new models are ([`Style::in_units`]).
    pub units_per_metre: f32,
}

/// Lines thinning and fading toward the background with distance: as drawn from `near` to
/// `far`, as thin and faint as `far_width` and `far_fade` say from `far` on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceFade {
    /// Where lines start to thin and fade, in metres from the view.
    pub near: f32,
    /// Where they are thinnest and faintest, in metres.
    pub far: f32,
    /// Their width there, as a part of their own, 0 to 1.
    pub far_width: f32,
    /// How far toward what is behind them they fade there, 0 to 1.
    pub far_fade: f32,
}

/// Rows of the distance table ([`Style::fade_rows`]).
pub(crate) const FADE_ROWS: usize = 2;

/// The plane `camera`'s distances are measured from for lines by distance, `[nx, ny, nz, d]`
/// giving `n·p + d`: through the eye in a perspective; through the point looked at in a
/// parallel view, whose eye moves as it zooms, so lines keep their look while zooming a plan.
pub(crate) fn distance_plane(camera: &crate::Camera) -> [f32; 4] {
    let n = camera.forward();
    let from = if camera.is_parallel() {
        camera.target
    } else {
        camera.eye()
    };
    let d = -(n[0] * from[0] + n[1] * from[1] + n[2] * from[2]);
    [n[0] as f32, n[1] as f32, n[2] as f32, d as f32]
}

/// One kind of object's faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindFaces {
    /// Whether they show.
    pub shown: bool,
    /// One colour for them all, or `None` for each object's own.
    pub colour: Option<[f32; 3]>,
    /// 0 (invisible) to 1 (solid).
    pub opacity: f32,
}

impl KindFaces {
    /// As each object is given.
    pub const GIVEN: Self = Self {
        shown: true,
        colour: None,
        opacity: 1.0,
    };

    /// `kind`'s faces in `view`.
    fn of(view: &ViewStyle, kind: FacedKind) -> Self {
        let faces = view.faces(kind);
        Self {
            shown: faces.shown,
            colour: match faces.colour {
                ColourFrom::One(colour) => Some(colour.to_f32()),
                _ => None,
            },
            opacity: faces.opacity,
        }
    }
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
    /// How wide they are, in millimetres on the screen; never thinner than the default line
    /// ([`OutlineLook::pixels`]).
    pub width: f32,
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
        let any_shown = [FacedKind::Nurbs, FacedKind::Mesh, FacedKind::Subd]
            .iter()
            .any(|&kind| view.faces(kind).shown);
        let surfaces = any_shown.then(|| Surfaces {
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
                width: view.outlines.width,
            },
            hidden_lines: view.hidden_lines.shown,
            faces: [FacedKind::Nurbs, FacedKind::Mesh, FacedKind::Subd]
                .map(|kind| KindFaces::of(view, kind)),
            distance: DistanceFade::of(view),
            units_per_metre: 1000.0,
        }
    }

    /// This style for a model whose unit is `millimetres` long, so distances in metres are
    /// measured in the model's units.
    #[must_use]
    pub fn in_units(self, millimetres: f64) -> Self {
        Self {
            units_per_metre: (1000.0 / millimetres.max(1e-9)) as f32,
            ..self
        }
    }

    /// The distance table the line shader reads: the plane distances are measured from
    /// (`plane`, `[nx, ny, nz, d]` with `n` of length 1, giving `n·p + d` in the model's units),
    /// then where lines start to thin and fade and where they are thinnest, in the model's units,
    /// and how thin and faint they are there. Lines keep their look everywhere without a rule.
    pub(crate) fn fade_rows(&self, plane: [f32; 4]) -> [[f32; 4]; FADE_ROWS] {
        match self.distance {
            Some(fade) => [
                plane,
                [
                    fade.near * self.units_per_metre,
                    fade.far * self.units_per_metre,
                    fade.far_width,
                    fade.far_fade,
                ],
            ],
            None => [[0.0; 4], [0.0, 1.0, 1.0, 0.0]],
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

    /// How faces of `kind` look: locked objects' and annotation's in their own colours, as
    /// opaque as NURBS faces, so Ghosted sees through them too.
    #[must_use]
    pub fn faces_of(&self, kind: FaceKind) -> KindFaces {
        match kind {
            FaceKind::Given => KindFaces {
                opacity: self.faces[0].opacity,
                ..KindFaces::GIVEN
            },
            FaceKind::Nurbs => self.faces[0],
            FaceKind::Mesh => self.faces[1],
            FaceKind::Subd => self.faces[2],
        }
    }

    /// Whether some kind's faces are drawn solid, hiding what is behind them: shown and opaque.
    #[must_use]
    pub fn solid(&self) -> bool {
        self.any_faces(|faces| faces.opacity >= 1.0)
    }

    /// Whether some kind's faces are drawn see-through, as in Ghosted.
    #[must_use]
    pub fn see_through(&self) -> bool {
        self.any_faces(|faces| faces.opacity < 1.0)
    }

    /// Whether the view draws faces and some kind's shown faces are as `test` says.
    fn any_faces(&self, test: impl Fn(KindFaces) -> bool) -> bool {
        self.surfaces.is_some()
            && FaceKind::ALL
                .into_iter()
                .map(|kind| self.faces_of(kind))
                .any(|faces| faces.shown && faces.opacity > 0.0 && test(faces))
    }

    /// The face table the face shader reads: for each [`FaceKind`] in order, its colour and
    /// whether to use it, then its opacity and whether it shows. No kind shows in a view without
    /// faces.
    pub(crate) fn face_rows(&self) -> [[f32; 4]; FACE_ROWS] {
        let mut rows = [[0.0; 4]; FACE_ROWS];
        if self.surfaces.is_none() {
            return rows;
        }
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        for kind in FaceKind::ALL {
            let look = self.faces_of(kind);
            let [r, g, b] = look.colour.unwrap_or_default();
            let at = kind as usize * 2;
            rows[at] = [r, g, b, flag(look.colour.is_some())];
            rows[at + 1] = [look.opacity, flag(look.shown), 0.0, 0.0];
        }
        rows
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

impl DistanceFade {
    /// `view`'s rule for lines by distance, if it has one that changes anything.
    fn of(view: &ViewStyle) -> Option<Self> {
        let rules = &view.line_rules;
        let changes = rules.far_width < 1.0 || rules.far_fade > 0.0;
        (rules.by_distance && changes).then(|| Self {
            near: rules.near.max(0.0),
            far: rules.far.max(rules.near.max(0.0) + 1e-3),
            far_width: rules.far_width.clamp(0.0, 1.0),
            far_fade: rules.far_fade.clamp(0.0, 1.0),
        })
    }
}

impl OutlineLook {
    /// Outlines as dark and thin as the edges on faces.
    pub const EDGES: Self = Self {
        shown: true,
        colour: EDGE_ON_FACES_COLOUR,
        width: 0.25,
    };

    /// How many pixels wide the outlines draw on a screen with `per_mm` pixels to a millimetre:
    /// their width, rounded, at least the default line's two pixels and at most
    /// [`MOST_OUTLINE_PIXELS`]. Shaded's 0.25 mm draws two pixels and Massing's 0.75 mm three, on
    /// a screen at 100 %.
    #[must_use]
    pub fn pixels(&self, per_mm: f32) -> u32 {
        let wanted = (self.width * per_mm).round();
        if wanted.is_finite() {
            (wanted as u32).clamp(2, MOST_OUTLINE_PIXELS)
        } else {
            2
        }
    }
}

/// The widest outlines draw, in pixels: the outline pass looks this far for a step in depth.
pub const MOST_OUTLINE_PIXELS: u32 = 8;

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
    fn each_built_in_style_draws_as_its_display_mode_did() {
        let paper = Some(Surfaces {
            colour: Some(PAPER_COLOUR),
            see_through: false,
            light: Light::Flat,
        });
        assert_eq!(Style::hidden().surfaces, paper);
        let ink = OutlineLook {
            shown: true,
            colour: INK_COLOUR,
            width: 0.25,
        };
        assert_eq!(Style::hidden().outlines, ink);
        assert_eq!(
            Style::massing().outlines,
            OutlineLook { width: 0.75, ..ink }
        );
        assert_eq!(Style::shaded().outlines, OutlineLook::EDGES);
        assert!(!Style::wireframe().outlines.shown);
        let massing = Style::massing().surfaces.expect("faces");
        assert_eq!(
            (massing.colour, massing.light),
            (Some(PAPER_COLOUR), Light::Sun)
        );
        // Every display mode's surfaces, as before.
        let own = |see_through: bool| Surfaces {
            colour: None,
            see_through,
            light: Light::View,
        };
        assert_eq!(Style::wireframe().surfaces, None);
        assert_eq!(Style::shaded().surfaces, Some(own(false)));
        assert_eq!(Style::ghosted().surfaces, Some(own(true)));
        assert_eq!(Style::technical().surfaces, paper);
        // Lines: dark edges on solid faces, light ones otherwise, ink in Hidden, Technical and
        // Massing, hidden lines only in Technical.
        let edge = |style: Style| style.lines.of(crate::LineKind::Edge).colour;
        assert_eq!(edge(Style::shaded()), Some(EDGE_ON_FACES_COLOUR));
        assert_eq!(edge(Style::wireframe()), None);
        assert_eq!(edge(Style::ghosted()), None);
        for inked in [Style::hidden(), Style::technical(), Style::massing()] {
            assert_eq!(edge(inked), Some(INK_COLOUR));
        }
        assert!(Style::technical().hidden_lines && !Style::massing().hidden_lines);
    }

    #[test]
    fn lines_by_distance_are_measured_in_the_model_s_units_from_the_view() {
        let plane = [0.0, 0.0, -1.0, 10.0];
        let mut view = ViewStyle::shaded();
        assert_eq!(
            Style::of(&view).fade_rows(plane),
            [[0.0; 4], [0.0, 1.0, 1.0, 0.0]],
            "no rule: lines as drawn everywhere"
        );
        view.line_rules.by_distance = true;
        let drawn = Style::of(&view);
        let rules = view.line_rules;
        assert_eq!(
            drawn.fade_rows(plane),
            [
                plane,
                [
                    rules.near * 1000.0,
                    rules.far * 1000.0,
                    rules.far_width,
                    rules.far_fade
                ]
            ],
            "metres in millimetres, as new models are"
        );
        let in_metres = drawn.in_units(1000.0).fade_rows(plane);
        assert_eq!(in_metres[1][..2], [rules.near, rules.far]);
        // A rule that changes nothing is no rule.
        view.line_rules.far_width = 1.0;
        view.line_rules.far_fade = 0.0;
        assert_eq!(Style::of(&view).distance, None);
    }

    #[test]
    fn distances_are_from_the_eye_in_a_perspective_and_from_the_target_in_a_parallel_view() {
        let mut camera = crate::Camera {
            target: [0.0, 0.0, 5.0],
            yaw: 0.0,
            pitch: std::f64::consts::FRAC_PI_2,
            distance: 20.0,
            ..crate::Camera::default()
        };
        // Looking straight down from 25 above the ground: the ground is 25 away.
        let [nx, ny, nz, d] = distance_plane(&camera);
        let away = nx * 0.0 + ny * 0.0 + nz * 0.0 + d;
        assert!((away - 25.0).abs() < 1e-3, "{away}");
        camera.parallel = true;
        let [.., nz, d] = distance_plane(&camera);
        assert!((nz * 0.0 + d - 5.0).abs() < 1e-3, "from the target: {d}");
    }

    /// Pixels per millimetre on a 96 dpi screen at 100 %.
    const PER_MM: f32 = 96.0 / 25.4;

    #[test]
    fn outlines_draw_as_wide_as_the_style_says_but_never_thinner_than_a_line() {
        let wide = |width: f32| OutlineLook {
            width,
            ..OutlineLook::EDGES
        };
        assert_eq!(Style::shaded().outlines.pixels(PER_MM), 2, "as before");
        assert_eq!(Style::massing().outlines.pixels(PER_MM), 3, "as before");
        assert_eq!(wide(0.0).pixels(PER_MM), 2, "never thinner than a line");
        assert_eq!(wide(1.5).pixels(PER_MM), 6);
        assert_eq!(wide(1.5).pixels(2.0 * PER_MM), MOST_OUTLINE_PIXELS);
        assert_eq!(wide(f32::NAN).pixels(PER_MM), 2);
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
        assert!(!drawn.outlines.shown && drawn.outlines.pixels(PER_MM) == 4);
        view.engine = Engine::Vector;
        assert_eq!(Style::of(&view).light(), Some(Light::View), "tones are lit");
        view.lighting.vector = VectorFaces::Flat;
        assert_eq!(Style::of(&view).light(), Some(Light::Flat));
    }
}
