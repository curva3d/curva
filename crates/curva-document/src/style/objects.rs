//! How each kind of object is drawn: faces, edges and lines of their own, curves, points,
//! annotation, lights and clipping planes.

use super::Colour;

/// Where faces take their colour from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourFrom {
    /// The object's colour.
    Object,
    /// The layer's colour.
    Layer,
    /// The object's material.
    Material,
    /// One colour for all.
    One(Colour),
}

/// What colour lines are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineColour {
    /// The object's colour.
    Object,
    /// One colour for all.
    One(Colour),
}

/// How back faces are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackFaces {
    /// As the front.
    AsFront,
    /// In another colour.
    One(Colour),
    /// Not drawn.
    Hidden,
}

/// How thick a line is (§2.1): following the object's print width times a factor, or fixed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineWidth {
    /// The object's own width (its print width, or the default line), times this.
    Object(f32),
    /// This many millimetres.
    Fixed(f32),
}

/// A line's pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// Solid.
    Solid,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
}

/// Faces of one kind of object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faces {
    /// Whether they show.
    pub shown: bool,
    /// Where their colour comes from.
    pub colour: ColourFrom,
    /// 0 (invisible) to 1 (solid).
    pub opacity: f32,
    /// How back faces are drawn.
    pub back: BackFaces,
    /// Whether they are shaded flat, showing their facets.
    pub flat: bool,
}

/// Edges of one kind of object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    /// Whether they show.
    pub shown: bool,
    /// How thick.
    pub width: LineWidth,
    /// What colour.
    pub colour: LineColour,
    /// Their pattern.
    pub pattern: Pattern,
}

/// A kind of line of its own, such as isocurves or SubD creases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineLook {
    /// Whether it shows.
    pub shown: bool,
    /// How thick.
    pub width: LineWidth,
    /// Its colour, or `None` for the edges' colour.
    pub colour: Option<Colour>,
}

/// How one kind of object's outlines differ from the style's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineChoice {
    /// As the style's Outlines.
    AsStyle,
    /// Twice as thick.
    Thicker,
    /// None.
    Off,
}

/// Surfaces, polysurfaces and solids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NurbsLook {
    /// Their faces.
    pub faces: Faces,
    /// Their edges.
    pub edges: Edges,
    /// Tangent edges and seams.
    pub tangent_seams: bool,
    /// Open (naked) edges, highlighted.
    pub naked_edges: bool,
    /// Isocurves.
    pub isocurves: LineLook,
    /// Isocurves on flat faces too.
    pub isocurves_on_flat: bool,
    /// Their outlines.
    pub outline: OutlineChoice,
}

/// How thick mesh wires are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireWidth {
    /// One pixel, as Rhino 8 draws them.
    OnePixel,
    /// As the edges.
    AsEdges,
}

/// Mesh wires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wires {
    /// Whether they show.
    pub shown: bool,
    /// How thick.
    pub width: WireWidth,
    /// Their colour, or `None` for the edges' colour.
    pub colour: Option<Colour>,
}

/// Meshes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshLook {
    /// Whether faces, edges and outlines are those of NURBS.
    pub same_as_nurbs: bool,
    /// Their own faces.
    pub faces: Faces,
    /// Their own edges.
    pub edges: Edges,
    /// Their own outlines.
    pub outline: OutlineChoice,
    /// Their wires.
    pub wires: Wires,
    /// Their vertices.
    pub vertices: bool,
}

/// SubD objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubdLook {
    /// Whether faces and outlines are those of NURBS.
    pub same_as_nurbs: bool,
    /// Their own faces.
    pub faces: Faces,
    /// Their own outlines.
    pub outline: OutlineChoice,
    /// Smooth edges.
    pub smooth: LineLook,
    /// Creases.
    pub creases: LineLook,
    /// Boundaries.
    pub boundary: LineLook,
    /// The reflected half of a symmetric SubD.
    pub reflected_half: bool,
    /// The symmetry axis.
    pub symmetry_axis: bool,
}

/// How a line ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnds {
    /// Round.
    Round,
    /// Flat at the end point.
    Flat,
    /// Square, past the end point.
    Square,
}

/// Curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveLook {
    /// Whether they show.
    pub shown: bool,
    /// How thick.
    pub width: LineWidth,
    /// What colour.
    pub colour: LineColour,
    /// Their pattern.
    pub pattern: Pattern,
    /// How they end.
    pub ends: LineEnds,
    /// Their curvature graph.
    pub curvature: bool,
}

/// How points are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointShape {
    /// A square.
    Square,
    /// A dot.
    Round,
    /// A cross.
    Cross,
}

/// Points and point clouds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLook {
    /// Whether points show.
    pub shown: bool,
    /// Their size, in pixels.
    pub size: f32,
    /// Their shape.
    pub shape: PointShape,
    /// Whether point clouds show.
    pub clouds: bool,
    /// Point clouds' size, in pixels.
    pub cloud_size: f32,
}

/// Text and dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnnotationLook {
    /// Whether text shows.
    pub text: bool,
    /// Whether dimensions show.
    pub dimensions: bool,
    /// Their colour.
    pub colour: LineColour,
}

/// Light objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightObjects {
    /// Whether they show.
    pub shown: bool,
    /// Whether the model is lit by them instead of the style's lighting.
    pub light_with_them: bool,
}

/// A hatch pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hatch {
    /// None: the fill is plain.
    Plain,
    /// Diagonal lines.
    Diagonal,
    /// Crossed lines.
    Cross,
    /// Concrete.
    Concrete,
}

/// Clipping planes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClippingLook {
    /// Whether the planes themselves show.
    pub planes_shown: bool,
    /// Whether cut solids are filled.
    pub fill: bool,
    /// The fill's colour.
    pub fill_colour: LineColour,
    /// The fill's hatch.
    pub hatch: Hatch,
    /// Whether cut edges are drawn.
    pub cut_edges: bool,
    /// Their width, in millimetres.
    pub cut_edge_width: f32,
}
