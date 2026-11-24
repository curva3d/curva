//! View styles: how a view draws the model (`docs/specs/views-and-styles.md` §2). A style is plain
//! data, saved inside the model and as `.curvastyle` files ([`ViewStyle::to_text`]); the viewport
//! reads it. Every setting of Rhino 8's display modes has a place here, grouped by what it
//! draws: the scene (background, grid, light, shadows), each kind of object, lines, and the
//! drawings a style makes of plans, sections and elevations.

mod builtin;
mod file;
pub(crate) mod kept;
mod library;
mod objects;
mod value;

pub use builtin::BUILT_IN;
pub use file::{Problem, ReadStyle};
pub use kept::{StyleError, built_in_name};
pub use library::{EXTENSION, Library, LibraryError, LibraryStyle};
pub use objects::*;

/// A colour, red, green and blue from 0 to 255; written `#rrggbb` in style files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour(pub [u8; 3]);

impl Colour {
    /// Black ink.
    pub const BLACK: Self = Self([0, 0, 0]);
    /// White paper.
    pub const WHITE: Self = Self([255, 255, 255]);
    /// The near-black of edges drawn over shaded faces.
    pub const EDGE: Self = Self([20, 20, 26]);

    /// Red, green and blue from 0 to 1.
    #[must_use]
    pub fn to_f32(self) -> [f32; 3] {
        self.0.map(|c| f32::from(c) / 255.0)
    }
}

/// How a style shades surfaces: the choice of engine (§2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// Drawn shapes: faces flat or in tones, crisp lines and shadow shapes; prints exactly.
    Vector,
    /// Lit faces with ambient occlusion and soft shadows.
    Occlusion,
    /// Materials, an environment, reflections and exposure.
    Rendered,
}

/// What a style is used for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsedFor {
    /// 3D views.
    pub views: bool,
    /// Live plans.
    pub plans: bool,
    /// Live sections and elevations.
    pub sections: bool,
    /// Sheets and their PDF.
    pub sheets: bool,
}

/// How the background is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// The window theme's own background, light or dark.
    Theme,
    /// One colour, the top one.
    OneColour,
    /// Shading from the top colour to the bottom one.
    Gradient,
    /// Paper: the top colour with a grain.
    Paper,
}

/// Behind the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Background {
    /// How it is filled.
    pub fill: Fill,
    /// The colour, or the gradient's top.
    pub top: Colour,
    /// The gradient's bottom.
    pub bottom: Colour,
    /// The paper's grain, 0 to 1.
    pub grain: f32,
}

/// The grid and axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    /// Whether the grid shows.
    pub shown: bool,
    /// Its colour, or `None` for the standard one for the background.
    pub colour: Option<Colour>,
    /// Whether it draws over objects rather than under them.
    pub over_objects: bool,
    /// The grid's own red and green axes.
    pub axes: bool,
    /// The small world axes in the corner.
    pub world_axes: bool,
    /// The Z axis.
    pub z_axis: bool,
}

/// How faces are lit by the Vector engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorFaces {
    /// Each face one flat colour.
    Flat,
    /// A few tones, by which way a face turns.
    Tones,
}

/// How faces are lit by the Occlusion engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcclusionLight {
    /// A soft light from the viewer and from above.
    Studio,
    /// The view's sun, casting shadows.
    Sun,
    /// Unlit.
    Flat,
}

/// How faces are lit by the Rendered engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderedLight {
    /// The view's sun and the sky.
    SunAndSky,
    /// A studio picture around the model.
    Studio,
    /// The lights in the model.
    ModelLights,
}

/// The light on faces, for each engine; only the style's own engine's choice is used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// For the Vector engine.
    pub vector: VectorFaces,
    /// For the Occlusion engine.
    pub occlusion: OcclusionLight,
    /// For the Rendered engine.
    pub rendered: RenderedLight,
    /// How much faces shine, 0 (matte) to 1 (polished).
    pub shine: f32,
    /// The light every face gets, in shadow too.
    pub ambient: Colour,
}

/// The view's sun, until views carry their own (§1.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Where it shines from, in degrees clockwise from north.
    pub compass: f32,
    /// Its height above the horizon, in degrees.
    pub height: f32,
}

/// How a shadow's edge is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowEdge {
    /// Exact shapes, sharp at any scale.
    Crisp,
    /// Soft, as a picture.
    Soft,
}

/// Shadows cast by the sun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadows {
    /// Whether they show.
    pub shown: bool,
    /// Their edge; the Vector engine's are always crisp.
    pub edge: ShadowEdge,
    /// Their colour.
    pub colour: Colour,
    /// How dark, 0 to 1.
    pub strength: f32,
    /// How soft a soft edge is, in pixels.
    pub softness: f32,
    /// Whether they fall only on the ground, not on objects.
    pub ground_only: bool,
}

/// Darkening where things meet; not used by the Vector engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientOcclusion {
    /// Whether it shows.
    pub shown: bool,
    /// How dark, 0 to 1.
    pub strength: f32,
    /// How far it reaches, in millimetres.
    pub spread: f32,
    /// Its colour.
    pub colour: Colour,
}

/// Where the Rendered engine takes faces' colours from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialFrom {
    /// The object's material.
    Material,
    /// The layer's material.
    Layer,
    /// The object's colour.
    ObjectColour,
}

/// What surrounds the model in the Rendered engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    /// A studio.
    Studio,
    /// A sky made from the sun.
    Sky,
    /// A plain colour.
    Plain,
}

/// The Rendered engine's own settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderedLook {
    /// Where faces' colours come from.
    pub material_from: MaterialFrom,
    /// Whether textures show.
    pub textures: bool,
    /// What surrounds the model.
    pub environment: Environment,
    /// How much faces reflect, 0 to 1.
    pub reflections: f32,
    /// Brighter or darker, in stops.
    pub exposure: f32,
    /// A glow on bright spots.
    pub glow: bool,
}

/// Over the whole picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    /// Paper grain, 0 to 1.
    pub grain: f32,
    /// A darkened border, 0 to 1.
    pub vignette: f32,
    /// A soft gradient from the top, 0 to 1.
    pub wash: f32,
}

/// Where a shape ends against what is behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outlines {
    /// Whether they show.
    pub shown: bool,
    /// Their width, in millimetres.
    pub width: f32,
    /// Their colour.
    pub colour: Colour,
    /// Sharp bends inside a shape too.
    pub folds: bool,
    /// Where one object meets another.
    pub where_objects_meet: bool,
}

/// Edges behind surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HiddenLines {
    /// Whether they show.
    pub shown: bool,
    /// Their pattern.
    pub pattern: Pattern,
    /// How thick.
    pub width: LineWidth,
    /// Their colour.
    pub colour: Colour,
    /// 0 to 1.
    pub opacity: f32,
}

/// Rules that change lines (§2.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineRules {
    /// Every line's width times this.
    pub weight: f32,
    /// Whether lines thin and lighten with distance.
    pub by_distance: bool,
    /// Where they start to, in metres.
    pub near: f32,
    /// Where they are thinnest and lightest, in metres.
    pub far: f32,
    /// Their width there, as a part of their own, 0 to 1.
    pub far_width: f32,
    /// How far toward the background they fade there, 0 to 1.
    pub far_fade: f32,
}

/// How cut solids are filled in plans and sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutFill {
    /// One solid colour.
    Solid,
    /// A hatch.
    Hatch,
    /// The object's colour.
    ObjectColour,
    /// Not filled.
    Empty,
}

/// Where a plan or section cuts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    /// Cut lines' width, in millimetres.
    pub width: f32,
    /// Cut lines' colour.
    pub colour: Colour,
    /// How cut solids are filled.
    pub fill: CutFill,
    /// The fill's colour.
    pub fill_colour: Colour,
    /// The hatch, when the fill is one.
    pub hatch: Hatch,
    /// The hatch's size, as a part of its own (1 is as drawn).
    pub hatch_scale: f32,
    /// A plan's cut height above its level, in metres.
    pub plan_height: f32,
}

/// What a plan or section shows past its cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beyond {
    /// Whether it shows.
    pub shown: bool,
    /// Lines' width, in millimetres.
    pub width: f32,
    /// Lines' colour.
    pub colour: Colour,
    /// Whether lines lighten with depth.
    pub fade: bool,
    /// How far past the cut it shows, in metres.
    pub depth: f32,
    /// Whether surfaces beyond are filled.
    pub faces: bool,
}

/// What lies above a plan's cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Above {
    /// Not drawn.
    Hide,
    /// Dashed, as overhead lines.
    Dashed,
    /// Solid.
    Solid,
}

/// Plans, sections and elevations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawings {
    /// Where things are cut.
    pub cut: Cut,
    /// Past the cut.
    pub beyond: Beyond,
    /// Above a plan's cut.
    pub above: Above,
    /// Its colour.
    pub above_colour: Colour,
    /// Floors below, faint.
    pub below: bool,
}

/// How locked objects look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locked {
    /// As any other.
    Normal,
    /// Greyed.
    Greyed,
    /// See-through.
    Ghosted,
}

/// Selection, locked objects and the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Behaviour {
    /// The selection colour.
    pub selection_colour: Colour,
    /// Whether selected objects show through others.
    pub selected_show_through: bool,
    /// How locked objects look.
    pub locked: Locked,
    /// Locked objects' opacity, 0 to 1, when ghosted.
    pub locked_opacity: f32,
    /// Boxes instead of objects while moving.
    pub boxes_while_moving: bool,
    /// Whether transparency is drawn.
    pub transparency: bool,
    /// Whether highlights are drawn.
    pub highlights: bool,
}

/// How a view draws the model: every setting of a visualisation style.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewStyle {
    /// Its name, unique in the model and in the library.
    pub name: String,
    /// How surfaces are shaded.
    pub engine: Engine,
    /// What it is used for.
    pub used_for: UsedFor,
    /// Behind the model.
    pub background: Background,
    /// The grid and axes.
    pub grid: Grid,
    /// The light on faces.
    pub lighting: Lighting,
    /// The sun.
    pub sun: Sun,
    /// Shadows.
    pub shadows: Shadows,
    /// Ambient occlusion.
    pub occlusion: AmbientOcclusion,
    /// The Rendered engine's settings.
    pub rendered: RenderedLook,
    /// Over the whole picture.
    pub finish: Finish,
    /// Surfaces, polysurfaces and solids.
    pub nurbs: NurbsLook,
    /// Meshes.
    pub meshes: MeshLook,
    /// SubD objects.
    pub subd: SubdLook,
    /// Curves.
    pub curves: CurveLook,
    /// Points and point clouds.
    pub points: PointLook,
    /// Text and dimensions.
    pub annotation: AnnotationLook,
    /// Light objects.
    pub lights: LightObjects,
    /// Clipping planes.
    pub clipping: ClippingLook,
    /// Outlines.
    pub outlines: Outlines,
    /// Hidden lines.
    pub hidden_lines: HiddenLines,
    /// Line rules.
    pub line_rules: LineRules,
    /// Plans, sections and elevations.
    pub drawings: Drawings,
    /// Selection, locked objects and the rest.
    pub behaviour: Behaviour,
    /// Settings this version does not know, read from a file, kept as written so saving keeps
    /// them (§2.4): each its full key and its value.
    pub unknown: Vec<(String, String)>,
}

/// The kinds of object with faces, whose faces, edges and outlines a style sets apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacedKind {
    /// Surfaces, polysurfaces and solids.
    Nurbs,
    /// Meshes.
    Mesh,
    /// SubD objects.
    Subd,
}

impl ViewStyle {
    /// The faces `kind` draws with: its own, or NURBS' when it is the same as NURBS.
    #[must_use]
    pub fn faces(&self, kind: FacedKind) -> &Faces {
        match kind {
            FacedKind::Mesh if !self.meshes.same_as_nurbs => &self.meshes.faces,
            FacedKind::Subd if !self.subd.same_as_nurbs => &self.subd.faces,
            _ => &self.nurbs.faces,
        }
    }

    /// The edges `kind` draws with; SubD objects draw NURBS' edges for their creases and
    /// boundaries' pattern and colour unless those set their own.
    #[must_use]
    pub fn edges(&self, kind: FacedKind) -> &Edges {
        match kind {
            FacedKind::Mesh if !self.meshes.same_as_nurbs => &self.meshes.edges,
            _ => &self.nurbs.edges,
        }
    }

    /// The outline choice `kind` draws with.
    #[must_use]
    pub fn outline(&self, kind: FacedKind) -> OutlineChoice {
        match kind {
            FacedKind::Mesh if !self.meshes.same_as_nurbs => self.meshes.outline,
            FacedKind::Subd if !self.subd.same_as_nurbs => self.subd.outline,
            _ => self.nurbs.outline,
        }
    }

    /// Whether NURBS faces show and hide what is behind them: shown and fully opaque.
    #[must_use]
    pub fn solid(&self) -> bool {
        let faces = &self.nurbs.faces;
        faces.shown && faces.opacity >= 1.0
    }

    /// Whether NURBS faces show see-through.
    #[must_use]
    pub fn see_through(&self) -> bool {
        let faces = &self.nurbs.faces;
        faces.shown && faces.opacity < 1.0
    }
}

impl Default for ViewStyle {
    /// Shaded, as a new view opens.
    fn default() -> Self {
        Self::shaded()
    }
}

#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod tests;
