//! A `.3dm` model as the file holds it, independent of Curva's own types: what the reader gives
//! and the writer takes.

use crate::archive::Uuid;

/// A point or vector.
pub type P3 = [f64; 3];

/// A whole file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// The archive version read (50 and up, or 1 to 5); 70 when writing.
    pub version: u32,
    /// The length unit, as openNURBS numbers them (2 millimetres, 3 centimetres, 4 metres,
    /// 8 inches, 9 feet…).
    pub unit: u32,
    /// The absolute tolerance, in the model's unit.
    pub tolerance: f64,
    /// The angle tolerance, in radians.
    pub angle_tolerance: f64,
    /// Layers, in table order: an object's layer is its index here.
    pub layers: Vec<Layer>,
    /// Groups, in table order.
    pub groups: Vec<Group>,
    /// Block definitions.
    pub blocks: Vec<Block>,
    /// Hatch patterns, in the file's order; hatches name them by their place in it.
    pub hatch_patterns: Vec<HatchPattern>,
    /// The linetype table.
    pub linetypes: Vec<Linetype>,
    /// Annotation (dimension) styles, in the file's order.
    pub dim_styles: Vec<DimStyle>,
    /// Layouts (Rhino's page views), in the file's order.
    pub pages: Vec<Page>,
    /// The model's standard views by name (Top, Front, Right, Perspective) and viewport id, as
    /// clipping planes name the views they cut.
    pub model_views: Vec<(String, Uuid)>,
    /// The unit layouts are drawn in, as openNURBS numbers units; the model's when the file
    /// does not say.
    pub page_unit: Option<u32>,
    /// Rhino's document user text, key and value.
    pub document_text: Vec<(String, String)>,
    /// The model's notes.
    pub notes: String,
    /// Objects, those of block definitions included (their attributes say so).
    pub objects: Vec<Object>,
    /// Named views, in the file's order.
    pub views: Vec<View>,
    /// Named construction planes, in the file's order.
    pub cplanes: Vec<NamedCPlane>,
    /// What was skipped and how many of each, for the report.
    pub skipped: Vec<(String, usize)>,
}

impl Model {
    /// Counts one skipped thing of `kind`.
    pub fn skip(&mut self, kind: &str) {
        match self.skipped.iter_mut().find(|(k, _)| k == kind) {
            Some((_, n)) => *n += 1,
            None => self.skipped.push((kind.to_owned(), 1)),
        }
    }
}

/// A layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// Its name, without its parents'.
    pub name: String,
    /// Its id.
    pub id: Uuid,
    /// Its parent's id, nil at the top.
    pub parent: Uuid,
    /// Red, green, blue.
    pub color: [u8; 3],
    /// Whether it is shown.
    pub visible: bool,
    /// Whether it is locked.
    pub locked: bool,
    /// Print width in millimetres: 0 the default, below 0 not printed.
    pub print_width: f64,
    /// Its linetype's index in the linetype table; -1 continuous.
    pub linetype: i32,
    /// The details (by viewport id) it is turned off in, beside its own visibility.
    pub hidden_in: Vec<Uuid>,
    /// Its section style, for its objects that have none of their own.
    pub section: Option<crate::sections::SectionStyle>,
}

/// A group.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Its name, often empty.
    pub name: String,
    /// Its id.
    pub id: Uuid,
}

/// A block definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// Its name.
    pub name: String,
    /// Its id, which instances name.
    pub id: Uuid,
    /// Its description.
    pub description: String,
    /// The ids of the objects it is made of.
    pub objects: Vec<Uuid>,
    /// The file it is linked to, if any.
    pub linked: Option<String>,
}

/// How an object's colour or print width is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// From its layer.
    #[default]
    Layer,
    /// Its own.
    Object,
    /// From its parent block instance (or its own colour when not in one).
    Parent,
}

/// An object's attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Attributes {
    /// Its id.
    pub id: Uuid,
    /// Its layer's index.
    pub layer: usize,
    /// Its name.
    pub name: String,
    /// Its own colour, used when `color_source` says so.
    pub color: [u8; 3],
    /// Where its colour comes from.
    pub color_source: Source,
    /// Its own print width in millimetres.
    pub print_width: f64,
    /// Where its print width comes from.
    pub print_width_source: Source,
    /// Whether it is shown.
    pub visible: bool,
    /// 0 normal, 1 hidden, 2 locked, 3 part of a block definition.
    pub mode: u8,
    /// The indices of the groups it is in.
    pub groups: Vec<usize>,
    /// Its user text, key and value.
    pub user_text: Vec<(String, String)>,
    /// Whether it is drawn on a layout's paper rather than in the model.
    pub page_space: bool,
    /// The view it belongs to: for an object on a layout, its page's id.
    pub viewport: Uuid,
    /// Its own linetype's index in the linetype table, used when `linetype_source` says so;
    /// -1 continuous.
    pub linetype: i32,
    /// Where its linetype comes from.
    pub linetype_source: Source,
    /// Where its section style comes from: 0 its layer, 1 itself (Rhino's numbers).
    pub section_source: u8,
    /// Its own section style, if it has one; a clipping plane's is the plane's style.
    pub section: Option<crate::sections::SectionStyle>,
}

/// A linetype (`ON_Linetype`): its name and pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Linetype {
    /// Its id.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// Its pattern: each segment's length in millimetres on paper (Rhino's, whatever the model's
    /// unit), and whether it is drawn (a dash or dot) or left out (a gap).
    pub segments: Vec<(f64, bool)>,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            id: Uuid::NIL,
            layer: 0,
            name: String::new(),
            color: [0, 0, 0],
            color_source: Source::Layer,
            print_width: 0.0,
            print_width_source: Source::Layer,
            visible: true,
            mode: 0,
            groups: Vec::new(),
            user_text: Vec::new(),
            page_space: false,
            viewport: Uuid::NIL,
            linetype: -1,
            linetype_source: Source::Layer,
            section_source: crate::sections::FROM_LAYER,
            section: None,
        }
    }
}

/// An object: its geometry and attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// What it is.
    pub geometry: Geometry,
    /// Its attributes.
    pub attributes: Attributes,
}

/// A plane: origin and axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Its origin.
    pub origin: P3,
    /// Its X direction.
    pub x: P3,
    /// Its Y direction.
    pub y: P3,
    /// Its Z direction.
    pub z: P3,
}

impl Plane {
    /// The plane's equation `[a, b, c, d]`, as openNURBS stores it beside the axes.
    #[must_use]
    pub fn equation(&self) -> [f64; 4] {
        let [a, b, c] = self.z;
        let d = -(a * self.origin[0] + b * self.origin[1] + c * self.origin[2]);
        [a, b, c, d]
    }
}

/// A NURBS curve as openNURBS holds it: `order` = degree + 1, knots without the two outermost
/// ones, control points homogeneous (`[x w, y w, z w, w]`, or `w` = 1) in `dimension` 2 or 3.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    /// 2 for trim curves, 3 otherwise.
    pub dimension: usize,
    /// Degree + 1.
    pub order: usize,
    /// `points.len() + order - 2` knots.
    pub knots: Vec<f64>,
    /// Homogeneous control points; z is 0 for 2D curves.
    pub points: Vec<[f64; 4]>,
    /// Whether it is rational.
    pub rational: bool,
}

/// A NURBS surface as openNURBS holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    /// Degrees + 1, along u and v.
    pub order: [usize; 2],
    /// Knots along u and v, without the outermost ones.
    pub knots: [Vec<f64>; 2],
    /// Homogeneous control points, `points[i][j]` with i along u.
    pub points: Vec<Vec<[f64; 4]>>,
    /// Whether it is rational.
    pub rational: bool,
}

/// A curve, of whatever kind the file holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    /// A line.
    Line {
        /// Its ends.
        ends: [P3; 2],
        /// Its parameter interval.
        domain: [f64; 2],
    },
    /// Straight segments.
    Polyline {
        /// Its points.
        points: Vec<P3>,
        /// The parameter at each point.
        params: Vec<f64>,
    },
    /// An arc or circle.
    Arc {
        /// Its plane, centred on the circle.
        plane: Plane,
        /// Its radius.
        radius: f64,
        /// Its start and end angles, radians.
        angles: [f64; 2],
        /// Its parameter interval.
        domain: [f64; 2],
    },
    /// A NURBS curve.
    Nurbs(NurbsCurve),
    /// Curves end to end.
    Poly {
        /// The pieces.
        segments: Vec<Curve>,
        /// The parameter at each joint, `segments.len() + 1` of them.
        params: Vec<f64>,
    },
}

/// A surface, of whatever kind the file holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface {
    /// A NURBS surface.
    Nurbs(NurbsSurface),
    /// A rectangle of a plane.
    Plane {
        /// The plane.
        plane: Plane,
        /// The parameter intervals.
        domain: [[f64; 2]; 2],
        /// The extents along x and y.
        extents: [[f64; 2]; 2],
    },
    /// A curve revolved about an axis.
    Revolution {
        /// The profile.
        curve: Box<Curve>,
        /// The axis, from and to.
        axis: [P3; 2],
        /// The angles turned, radians.
        angles: [f64; 2],
        /// The angle's parameter interval.
        domain: [f64; 2],
        /// Whether u is the angle (false) or the profile's parameter (true).
        transposed: bool,
    },
    /// The sum of two curves: `curves[0](u) + curves[1](v) + base`.
    Sum {
        /// The two curves.
        curves: [Box<Curve>; 2],
        /// The base vector.
        base: P3,
    },
}

/// A B-rep as openNURBS holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brep {
    /// Trim curves, in surface parameters.
    pub curves2: Vec<Option<Curve>>,
    /// Edge curves.
    pub curves3: Vec<Option<Curve>>,
    /// Face surfaces.
    pub surfaces: Vec<Option<Surface>>,
    /// Vertices.
    pub vertices: Vec<BrepVertex>,
    /// Edges.
    pub edges: Vec<BrepEdge>,
    /// Trims.
    pub trims: Vec<BrepTrim>,
    /// Loops.
    pub loops: Vec<BrepLoop>,
    /// Faces.
    pub faces: Vec<BrepFace>,
}

/// A B-rep vertex.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepVertex {
    /// Where it is.
    pub point: P3,
    /// The edges that meet at it.
    pub edges: Vec<i32>,
    /// Its tolerance.
    pub tolerance: f64,
}

/// A B-rep edge: part of a 3D curve between two vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepEdge {
    /// Its curve in `curves3`.
    pub curve: i32,
    /// Whether it runs against its curve.
    pub reversed: bool,
    /// The part of the curve it is.
    pub proxy_domain: [f64; 2],
    /// Its own parameter interval.
    pub domain: [f64; 2],
    /// Its start and end vertices.
    pub vertices: [i32; 2],
    /// The trims that use it.
    pub trims: Vec<i32>,
    /// Its tolerance.
    pub tolerance: f64,
}

/// A trim: an edge as one face's loop uses it, with its curve in the face's parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepTrim {
    /// Its curve in `curves2`.
    pub curve: i32,
    /// The part of the curve it is.
    pub proxy_domain: [f64; 2],
    /// Its own parameter interval.
    pub domain: [f64; 2],
    /// Whether it runs against its curve.
    pub proxy_reversed: bool,
    /// Its edge, or -1 for a singular trim at a pole.
    pub edge: i32,
    /// Its start and end vertices.
    pub vertices: [i32; 2],
    /// Whether it runs against its edge.
    pub reversed3d: bool,
    /// 0 unknown, 1 boundary, 2 mated, 3 seam, 4 singular, 5 crvonsrf, 6 ptonsrf, 7 slit.
    pub kind: i32,
    /// Which iso-parametric line it lies on, if any (openNURBS' `ISO`).
    pub iso: i32,
    /// Its loop.
    pub loop_index: i32,
    /// Its tolerances in u and v.
    pub tolerance: [f64; 2],
}

/// A loop of trims.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepLoop {
    /// Its trims, in order.
    pub trims: Vec<i32>,
    /// 1 outer, 2 inner, 3 slit.
    pub kind: i32,
    /// Its face.
    pub face: i32,
}

/// A face: a surface bounded by loops.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepFace {
    /// Its loops, the outer first.
    pub loops: Vec<i32>,
    /// Its surface in `surfaces`.
    pub surface: i32,
    /// Whether its normal is against its surface's.
    pub reversed: bool,
}

/// An extrusion object: a profile swept along a straight path, capped or not.
#[derive(Clone, Debug, PartialEq)]
pub struct Extrusion {
    /// The path's start and end.
    pub path: [P3; 2],
    /// The profile plane's up direction.
    pub up: P3,
    /// The profiles, in the profile plane's coordinates (z 0), the outer first.
    pub profiles: Vec<Curve>,
    /// Whether the bottom and top are capped.
    pub caps: [bool; 2],
}

/// What an object is.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// A point.
    Point(P3),
    /// A curve.
    Curve(Curve),
    /// A surface.
    Surface(Surface),
    /// A B-rep: a polysurface or solid.
    Brep(Brep),
    /// An extrusion object.
    Extrusion(Extrusion),
    /// A note (`ON_Text`).
    Text(Text),
    /// A linear dimension (`ON_DimLinear`).
    Dimension(LinearDimension),
    /// A radius or diameter dimension (`ON_DimRadial`).
    Radial(RadialDimension),
    /// An angle dimension (`ON_DimAngular`).
    Angular(AngularDimension),
    /// An ordinate dimension (`ON_DimOrdinate`).
    Ordinate(OrdinateDimension),
    /// A leader (`ON_Leader`).
    Leader(Leader),
    /// A detail view on a layout (`ON_DetailView`).
    Detail(Detail),
    /// A clipping plane (`ON_ClippingPlaneSurface`).
    ClippingPlane(ClippingPlane),
    /// A hatch.
    Hatch(Hatch),
    /// A text dot (`ON_TextDot`): a label at a point, the same size on screen.
    Dot {
        /// The point it labels.
        at: P3,
        /// What it shows.
        text: String,
        /// Words it holds without showing them.
        secondary: String,
        /// Its letters' height on screen, in points.
        height: i32,
        /// Its font's name; empty for the usual one.
        font: String,
    },
    /// A block instance: its definition's id and placement (row-major 4 × 4).
    Instance {
        /// The definition.
        block: Uuid,
        /// Where it is placed.
        xform: [[f64; 4]; 4],
    },
}

/// A named view: a camera, as openNURBS' viewport holds one.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    /// Its name.
    pub name: String,
    /// Where the camera is.
    pub location: P3,
    /// The direction it looks along.
    pub direction: P3,
    /// Its up direction.
    pub up: P3,
    /// The point it looks at, which orbiting turns about.
    pub target: P3,
    /// Whether it has no perspective.
    pub parallel: bool,
    /// The frustum: left, right, bottom, top, near and far.
    pub frustum: [f64; 6],
}

/// A named construction plane.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedCPlane {
    /// Its name.
    pub name: String,
    /// The plane.
    pub plane: Plane,
}

/// A hatch pattern (`ON_HatchPattern`).
#[derive(Clone, Debug, PartialEq)]
pub struct HatchPattern {
    /// Identifies it.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// What it shows.
    pub description: String,
    /// Whether it fills solid (openNURBS' fill type 0) rather than with lines (1).
    pub solid: bool,
    /// Its line families.
    pub lines: Vec<HatchLine>,
}

/// One family of a hatch pattern's lines (`ON_HatchLine`), at scale one in model units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HatchLine {
    /// Their direction, radians from the hatch plane's X.
    pub angle: f64,
    /// A point the first line passes through.
    pub base: [f64; 2],
    /// From one line to the next: along the line, then across it.
    pub offset: [f64; 2],
    /// Dash lengths, a gap when negative; solid when empty.
    pub dashes: Vec<f64>,
}

/// A hatch (`ON_Hatch`): outlines on a plane, filled with a pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Hatch {
    /// Its plane; the outlines are 2D curves in it.
    pub plane: Plane,
    /// What the pattern's sizes are multiplied by.
    pub scale: f64,
    /// How far the pattern is turned, radians.
    pub rotation: f64,
    /// The pattern, by its place in the hatch pattern table.
    pub pattern: i32,
    /// The outlines, each with whether it is an outer one (else a hole).
    pub loops: Vec<(bool, Curve)>,
    /// Where the pattern starts, in the plane.
    pub base: [f64; 2],
}

/// An annotation style (`ON_DimStyle`), as much of it as Curva reads.
#[derive(Clone, Debug, PartialEq)]
pub struct DimStyle {
    /// Identifies it; notes name their style by it.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// Its text height, in model units.
    pub text_height: f64,
    /// Notes' horizontal alignment, as Rhino 5 numbered it: 0 left, 1 centre, 2 right.
    pub h_align: u32,
    /// Notes' vertical alignment, as Rhino 5 numbered it: 3 top, 4 middle of the first line,
    /// 5 middle, 6 middle of the last line, 7 bottom, 8 under the bottom.
    pub v_align: u32,
    /// Arrowheads' length, in model units.
    pub arrow_size: f64,
    /// How far extension lines reach past the dimension line, in model units.
    pub extension_extension: f64,
    /// The gap between a measured point and its extension line, in model units.
    pub extension_offset: f64,
    /// The gap between the dimension line and its text, in model units.
    pub text_gap: f64,
    /// Decimal places of lengths.
    pub length_resolution: i32,
    /// Its font's family name; Rhino's default when `None`.
    pub font: Option<String>,
}

/// A note (`ON_Text`).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    /// Its words as rich text (RTF), or plain.
    pub rich: String,
    /// Its plane: the note's point and directions.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style the note sets for itself (its overrides), if it has one.
    pub own: Option<DimStyle>,
    /// A turn of the words within the plane, radians.
    pub rotation: f64,
}

/// A linear dimension (`ON_DimLinear`).
#[derive(Clone, Debug, PartialEq)]
pub struct LinearDimension {
    /// Its plane, at the first point measured; for a rotated dimension its X is the direction
    /// measured along, for an aligned one it runs from the first point to the second.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Whether it measures along its plane's X (rotated) rather than point to point (aligned).
    pub rotated: bool,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The second point measured, in the plane.
    pub second: [f64; 2],
    /// A point its dimension line passes through, in the plane.
    pub line: [f64; 2],
}

/// A radius or diameter dimension (`ON_DimRadial`).
#[derive(Clone, Debug, PartialEq)]
pub struct RadialDimension {
    /// Its plane, at the circle's centre.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Whether it shows the diameter rather than the radius.
    pub diameter: bool,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The point on the circle its arrow touches, in the plane.
    pub radius_point: [f64; 2],
    /// Where its leader ends and its text sits, in the plane.
    pub text_point: [f64; 2],
}

/// An angle dimension (`ON_DimAngular`).
#[derive(Clone, Debug, PartialEq)]
pub struct AngularDimension {
    /// Its plane, at the angle's corner.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// The directions of its two sides, in the plane.
    pub sides: [[f64; 2]; 2],
    /// How far along each side its extension lines start.
    pub offsets: [f64; 2],
    /// A point its arc passes through, in the plane.
    pub arc_point: [f64; 2],
}

/// openNURBS' `ON_UNSET_VALUE`: a kink offset left at its default.
pub const UNSET_VALUE: f64 = -1.234_321_012_343_21e308;

/// An ordinate dimension (`ON_DimOrdinate`).
#[derive(Clone, Debug, PartialEq)]
pub struct OrdinateDimension {
    /// Its plane, at the base point the distance is measured from.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its text, `<>` standing for the measured value.
    pub text: String,
    /// Which coordinate it gives: 0 implied by its leader, 1 X, 2 Y.
    pub direction: u32,
    /// The point measured, in the plane.
    pub point: [f64; 2],
    /// Where its leader ends, in the plane.
    pub leader: [f64; 2],
    /// Its two kink offsets; [`UNSET_VALUE`] for the default.
    pub kinks: [f64; 2],
}

/// A leader (`ON_Leader`).
#[derive(Clone, Debug, PartialEq)]
pub struct Leader {
    /// Its plane.
    pub plane: Plane,
    /// Its style, by id.
    pub style: Uuid,
    /// The style it sets for itself, if it has one.
    pub own: Option<DimStyle>,
    /// Its words, as rich text or plain.
    pub rich: String,
    /// The arrow's tip, then the points the line passes through, in the plane.
    pub points: Vec<[f64; 2]>,
}

/// A layout: one of Rhino's page views.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// Its viewport's id, by which objects on it name it.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// Its paper's width and height, in millimetres.
    pub size_mm: [f64; 2],
    /// Its user text (layout user text, on its viewport).
    pub user_text: Vec<(String, String)>,
}

/// A detail view on a layout (`ON_DetailView`): a window onto the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    /// Its viewport's id, by which layers name the details they are off in.
    pub id: Uuid,
    /// The camera it looks through.
    pub view: View,
    /// Its outline on the page, a 2D curve in millimetres from the page's lower left corner.
    pub boundary: NurbsCurve,
    /// Page length per model length in the same unit (1:50 is 0.02); 0 for no set scale.
    pub ratio: f64,
    /// Its display mode's id (one of Rhino's standard modes); nil for the page's.
    pub display: Uuid,
    /// Whether its projection is locked (it cannot be panned or zoomed).
    pub locked: bool,
}

/// Rhino's standard display modes by name and id (openNURBS' `ON_StandardDisplayModeId`).
pub const DISPLAY_MODES: [(&str, Uuid); 10] = [
    (
        "Wireframe",
        Uuid::parse("1311ADCB-D89E-4051-A3F0-F64441FB8EC6"),
    ),
    (
        "Shaded",
        Uuid::parse("8BC8DEBE-C83B-4c47-B13C-9DB074510CAC"),
    ),
    (
        "Rendered",
        Uuid::parse("CAE60BAE-2D51-4299-ABF7-A339FCA86F3B"),
    ),
    (
        "Ghosted",
        Uuid::parse("FF608B97-81D3-4186-831C-41F7DC140881"),
    ),
    ("X-Ray", Uuid::parse("B5C19D5D-0AEC-4ff7-A10E-E052E660263A")),
    (
        "Rendered with shadows",
        Uuid::parse("A5545314-9D87-428d-95AE-91052EEAD0FA"),
    ),
    (
        "Technical",
        Uuid::parse("63612C72-778F-4afd-B81B-17426FDFE8A6"),
    ),
    (
        "Artistic",
        Uuid::parse("B46AB226-05A0-4568-B454-4B1AB721C675"),
    ),
    ("Pen", Uuid::parse("F4616FA5-A831-4620-A97E-9B807D5EC376")),
    (
        "Monochrome",
        Uuid::parse("E1B5C8A2-ED43-4872-9A01-814E612D5363"),
    ),
];

/// A display mode's id by name, ignoring case; nil (Rhino's default) for one it does not have.
#[must_use]
pub fn display_mode_id(name: &str) -> Uuid {
    DISPLAY_MODES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
        .map_or(Uuid::NIL, |(_, id)| *id)
}

/// A display mode's name by id, if it is one of Rhino's standard ones.
#[must_use]
pub fn display_mode_name(id: Uuid) -> Option<&'static str> {
    DISPLAY_MODES
        .iter()
        .find(|(_, m)| *m == id)
        .map(|(n, _)| *n)
}

/// A clipping plane (`ON_ClippingPlaneSurface`): a rectangle of a plane that cuts away, in the
/// views it names, everything on the side opposite its normal (Rhino keeps the side the normal
/// points to).
#[derive(Clone, Debug, PartialEq)]
pub struct ClippingPlane {
    /// Its plane; the normal points to the side kept.
    pub plane: Plane,
    /// The rectangle's extents along the plane's x and y.
    pub extents: [[f64; 2]; 2],
    /// The viewports it cuts: model views and details, by viewport id.
    pub viewports: Vec<Uuid>,
    /// Its own id.
    pub id: Uuid,
    /// Whether it cuts at all.
    pub enabled: bool,
}
