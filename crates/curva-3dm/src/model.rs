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
    /// Objects, those of block definitions included (their attributes say so).
    pub objects: Vec<Object>,
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
    /// A block instance: its definition's id and placement (row-major 4 × 4).
    Instance {
        /// The definition.
        block: Uuid,
        /// Where it is placed.
        xform: [[f64; 4]; 4],
    },
}
