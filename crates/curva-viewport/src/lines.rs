//! The vertices of drawn lines and faces, and the lines of the grid, the scene and previews.

use curva_kernel::{Curve, Point};

use crate::{
    CPlane, CURVE_COLOUR, EDGE_ON_FACES_COLOUR, GRID_COLOUR, GRID_HALF_SIZE, GRID_STEP,
    LOCKED_COLOUR, Outlines, PREVIEW_COLOUR, SELECTED_COLOUR, X_AXIS_COLOUR, Y_AXIS_COLOUR,
};

/// One end of a drawn line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in model space.
    pub position: [f32; 3],
    /// Colour, red, green and blue from 0 to 1.
    pub colour: [f32; 3],
    /// How thick its line is drawn: millimetres on the screen when positive, or that many
    /// pixels, negated, when zero or negative ([`line_width`]).
    pub width: f32,
    /// The object's number in the scene's list ([`SceneParts::objects`]), whose selection the
    /// graphics card looks up, or [`NO_OBJECT`] for lines whose look is final as given.
    pub object: u32,
}

/// The [`Vertex::object`] of lines and faces that belong to no object in the scene's list:
/// previews, the grid, and pictures, whose selection is drawn into their colours.
pub const NO_OBJECT: u32 = u32::MAX;

/// How thick the default line is, in pixels.
pub const DEFAULT_LINE_PIXELS: f32 = 2.0;

/// A pixel of a 96 dpi screen, in millimetres.
pub(crate) const MM_PER_PIXEL: f32 = 25.4 / 96.0;

/// The grid and axes stay a pixel thick.
pub(crate) const GRID_WIDTH: f32 = -1.0;

/// The [`Vertex::width`] of a line: millimetres, or the default line when `None`. Selected lines
/// draw a pixel thicker, so they stand out.
#[must_use]
pub fn line_width(mm: Option<f64>, selected: bool) -> f32 {
    match mm {
        Some(mm) if mm > 0.0 => mm as f32 + if selected { MM_PER_PIXEL } else { 0.0 },
        _ => -(DEFAULT_LINE_PIXELS + if selected { 1.0 } else { 0.0 }),
    }
}

/// One corner of a shaded triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    /// Position in model space.
    pub position: [f32; 3],
    /// Which way the surface faces here, of length 1.
    pub normal: [f32; 3],
    /// Colour, red, green and blue from 0 to 1.
    pub colour: [f32; 3],
    /// As [`Vertex::object`].
    pub object: u32,
}

pub(crate) fn grid_segment(out: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], colour: [f32; 3]) {
    segment_with(out, a, b, colour, GRID_WIDTH);
}

pub(crate) fn segment_with(
    out: &mut Vec<Vertex>,
    a: [f32; 3],
    b: [f32; 3],
    colour: [f32; 3],
    width: f32,
) {
    out.push(Vertex {
        position: a,
        colour,
        width,
        object: NO_OBJECT,
    });
    out.push(Vertex {
        position: b,
        colour,
        width,
        object: NO_OBJECT,
    });
}

pub(crate) fn narrow(p: Point) -> [f32; 3] {
    p.map(|value| value as f32)
}

/// `p` measured from `origin`, in single precision (#225): positions go to the graphics card
/// measured from a point near the model, so a model far from the origin draws as exactly as one
/// at it. The subtraction is done in double precision, before narrowing.
#[must_use]
pub fn narrow_from(p: Point, origin: Point) -> [f32; 3] {
    std::array::from_fn(|k| (p[k] - origin[k]) as f32)
}

/// `points` measured from `origin`, still in double precision; borrowed as they are when
/// `origin` is the origin.
pub(crate) fn moved_to(points: &[Point], origin: Point) -> std::borrow::Cow<'_, [Point]> {
    if origin == [0.0; 3] {
        return std::borrow::Cow::Borrowed(points);
    }
    points
        .iter()
        .map(|p| std::array::from_fn(|k| p[k] - origin[k]))
        .collect()
}

pub(crate) fn polyline(out: &mut Vec<Vertex>, points: &[Point], colour: [f32; 3]) {
    polyline_with(out, points, colour, -DEFAULT_LINE_PIXELS);
}

/// A run of points drawn with a dash pattern (lengths drawn and left out in turn, in model units).
pub(crate) fn dashed(
    out: &mut Vec<Vertex>,
    points: &[Point],
    pattern: &[f64],
    colour: [f32; 3],
    width: f32,
) {
    let mut slot = 0;
    let mut left = pattern[0];
    for pair in points.windows(2) {
        let (mut a, b) = (pair[0], pair[1]);
        let mut length = curva_kernel::gap(a, b);
        while length > 1e-12 {
            let step = left.min(length);
            let t = step / length;
            let c: Point = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
            if slot % 2 == 0 {
                segment_with(out, narrow(a), narrow(c), colour, width);
            }
            left -= step;
            length -= step;
            a = c;
            if left <= 1e-12 {
                slot = (slot + 1) % pattern.len();
                left = pattern[slot];
            }
        }
    }
}

pub(crate) fn polyline_with(out: &mut Vec<Vertex>, points: &[Point], colour: [f32; 3], width: f32) {
    for pair in points.windows(2) {
        segment_with(out, narrow(pair[0]), narrow(pair[1]), colour, width);
    }
}

/// The grid and axes of a plane as line segments.
#[must_use]
pub fn grid_lines(plane: &CPlane) -> Vec<Vertex> {
    grid_lines_from(plane, [0.0; 3])
}

/// As [`grid_lines`], measured from `origin` (#225).
fn grid_lines_from(plane: &CPlane, origin: Point) -> Vec<Vertex> {
    let on_plane = |u: f64, v: f64| {
        let along = |axis: usize| plane.origin[axis] + plane.x[axis] * u + plane.y[axis] * v;
        narrow_from([along(0), along(1), along(2)], origin)
    };
    let mut out = Vec::new();
    let reach = f64::from(GRID_HALF_SIZE);
    for step in (-GRID_HALF_SIZE..=GRID_HALF_SIZE).step_by(GRID_STEP as usize) {
        let at = f64::from(step);
        if step != 0 {
            grid_segment(
                &mut out,
                on_plane(at, -reach),
                on_plane(at, reach),
                GRID_COLOUR,
            );
            grid_segment(
                &mut out,
                on_plane(-reach, at),
                on_plane(reach, at),
                GRID_COLOUR,
            );
        }
    }
    grid_segment(
        &mut out,
        on_plane(-reach, 0.0),
        on_plane(reach, 0.0),
        X_AXIS_COLOUR,
    );
    grid_segment(
        &mut out,
        on_plane(0.0, -reach),
        on_plane(0.0, reach),
        Y_AXIS_COLOUR,
    );
    out
}

/// The grid, axes and objects as line segments, for a single view.
#[must_use]
pub fn scene_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    scene_lines_from(outlines, selected, [0.0; 3])
}

/// As [`scene_lines`], measured from `origin` (#225).
#[must_use]
pub fn scene_lines_from(outlines: &Outlines, selected: &[u64], origin: Point) -> Vec<Vertex> {
    let mut out = grid_lines_from(&CPlane::WORLD_XY, origin);
    out.extend(object_lines_from(outlines, selected, origin));
    out
}

/// The grid, axes and objects as line segments for a shaded view.
#[must_use]
pub fn shaded_scene_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    shaded_scene_lines_from(outlines, selected, [0.0; 3])
}

/// As [`shaded_scene_lines`], measured from `origin` (#225).
#[must_use]
pub fn shaded_scene_lines_from(
    outlines: &Outlines,
    selected: &[u64],
    origin: Point,
) -> Vec<Vertex> {
    let mut out = grid_lines_from(&CPlane::WORLD_XY, origin);
    out.extend(shaded_object_lines_from(outlines, selected, origin));
    out
}

/// The objects as line segments.
#[must_use]
pub fn object_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    object_lines_from(outlines, selected, [0.0; 3])
}

/// As [`object_lines`], measured from `origin` (#225).
#[must_use]
pub fn object_lines_from(outlines: &Outlines, selected: &[u64], origin: Point) -> Vec<Vertex> {
    lines_of(outlines, selected, false, None, (false, origin))
}

/// The objects as line segments for a shaded view, where the edges of surfaces and solids
/// are dark so they show against the faces, as in Rhino. Curves keep their layer's colour.
#[must_use]
pub fn shaded_object_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    shaded_object_lines_from(outlines, selected, [0.0; 3])
}

/// As [`shaded_object_lines`], measured from `origin` (#225).
#[must_use]
pub fn shaded_object_lines_from(
    outlines: &Outlines,
    selected: &[u64],
    origin: Point,
) -> Vec<Vertex> {
    lines_of(outlines, selected, true, None, (false, origin))
}

/// The window's scene (spec `display-pipeline.md` step 4): the lines and faces of every object,
/// each vertex carrying its object's number in [`Self::objects`] and drawn unselected. The
/// graphics card looks each number up in its selection table, so a change of selection sends a
/// few bytes instead of the whole scene again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneParts {
    /// The objects' lines, for wireframe and ghosted views.
    pub lines: Vec<Vertex>,
    /// The objects' lines for shaded views, with dark edges on faces.
    pub shaded_lines: Vec<Vertex>,
    /// The faces of surfaces and solids, as triangles; with [`SceneParts::shared`], only those
    /// not in `shapes`.
    pub faces: Vec<MeshVertex>,
    /// Surfaces and solids drawn from the mesh they share with their copies, each at its
    /// placement; empty with [`SceneParts::of`].
    pub shapes: Vec<PlacedShape>,
    /// Whole circles the graphics card draws from their centre and axes, one entry each, left
    /// out of `lines` and `shaded_lines`; empty with [`SceneParts::of`].
    pub circles: Vec<crate::PlacedCircle>,
    /// Which object each number stands for: number `n` is `objects[n]`.
    pub objects: Vec<u64>,
    /// What every position is measured from (#225): a point near the model
    /// ([`Outlines::drawing_origin`]).
    pub origin: Point,
}

/// A surface or solid drawn from the mesh it shares with its copies (`display-on-bevy.md`
/// step 4, #153): copies of a body and the parts of block instances hand the same `mesh`, so
/// a view holds it once and draws every copy at its `placement`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedShape {
    /// The object's number in [`SceneParts::objects`].
    pub object: u32,
    /// The colour its faces are drawn in when not selected.
    pub colour: [f32; 3],
    /// The faces as made, unplaced ([`curva_kernel::Body::shared_mesh`]).
    pub mesh: std::sync::Arc<curva_kernel::BodyMesh>,
    /// Where this copy sits, for points written as rows ([`curva_kernel::Body::placement`]).
    pub placement: [[f64; 4]; 4],
}

impl SceneParts {
    /// The scene of `outlines`, numbered, every face as triangles, measured from the middle of
    /// the model ([`Outlines::drawing_origin`]) as a window scene is.
    #[must_use]
    pub fn of(outlines: &Outlines) -> Self {
        let (objects, numbers) = numbered(outlines);
        let origin = outlines.drawing_origin();
        Self {
            lines: lines_of(outlines, &[], false, Some(&numbers), (false, origin)),
            shaded_lines: lines_of(outlines, &[], true, Some(&numbers), (false, origin)),
            faces: faces_of(outlines, &[], &[], Some(&numbers), (false, origin)),
            shapes: Vec::new(),
            circles: Vec::new(),
            objects,
            origin,
        }
    }

    /// As [`SceneParts::of`], but surfaces and solids come as [`PlacedShape`]s, each shape
    /// held once however many copies there are; `faces` keeps the rest (annotation's fills).
    /// Whole, undashed circles come as [`crate::PlacedCircle`]s, out of the lines.
    #[must_use]
    pub fn shared(outlines: &Outlines) -> Self {
        let (objects, numbers) = numbered(outlines);
        let origin = outlines.drawing_origin();
        Self {
            lines: lines_of(outlines, &[], false, Some(&numbers), (true, origin)),
            shaded_lines: lines_of(outlines, &[], true, Some(&numbers), (true, origin)),
            faces: faces_of(outlines, &[], &[], Some(&numbers), (true, origin)),
            shapes: shapes_of(outlines, &numbers, origin),
            circles: crate::circles::circles_of(outlines, &numbers, origin),
            objects,
            origin,
        }
    }
}

/// Every object's number, in the order first met: the list, and each id's number.
fn numbered(outlines: &Outlines) -> (Vec<u64>, std::collections::HashMap<u64, u32>) {
    let mut objects = Vec::new();
    let mut numbers = std::collections::HashMap::new();
    let ids = outlines.objects.iter().map(|(id, _)| *id);
    for id in ids.chain(outlines.faces.iter().map(|faces| faces.id)) {
        numbers.entry(id).or_insert_with(|| {
            objects.push(id);
            (objects.len() - 1) as u32
        });
    }
    (objects, numbers)
}

/// The surfaces and solids that share their mesh, numbered by `numbers`.
fn shapes_of(
    outlines: &Outlines,
    numbers: &std::collections::HashMap<u64, u32>,
    origin: Point,
) -> Vec<PlacedShape> {
    outlines
        .faces
        .iter()
        .filter_map(|faces| {
            let (mesh, placement) = faces.shared()?;
            Some(PlacedShape {
                object: numbers.get(&faces.id).copied().unwrap_or(NO_OBJECT),
                colour: if faces.locked {
                    LOCKED_COLOUR
                } else {
                    faces.colour
                },
                mesh: std::sync::Arc::clone(mesh),
                placement: placed_from(placement, origin),
            })
        })
        .collect()
}

/// A placement for points written as rows, moved so the points it places are measured from
/// `origin`: its last row, the move, less `origin`.
fn placed_from(placement: &[[f64; 4]; 4], origin: Point) -> [[f64; 4]; 4] {
    let mut moved = *placement;
    for k in 0..3 {
        moved[3][k] -= origin[k];
    }
    moved
}

/// The faces of a [`SceneParts::shared`] numbered by its `objects`, leaving out the objects
/// `hidden`, as while a preview stands in for them: the triangles not in its shapes, measured
/// from its `origin`.
#[must_use]
pub fn scene_faces_without(
    outlines: &Outlines,
    objects: &[u64],
    hidden: &[u64],
    origin: Point,
) -> Vec<MeshVertex> {
    let numbers = objects
        .iter()
        .enumerate()
        .map(|(number, id)| (*id, number as u32))
        .collect();
    faces_of(outlines, &[], hidden, Some(&numbers), (true, origin))
}

/// The `shapes` of a [`SceneParts::shared`] numbered by its `objects`, leaving out the objects
/// `hidden`, as while a preview stands in for them.
#[must_use]
pub fn scene_shapes_without(
    shapes: &[PlacedShape],
    objects: &[u64],
    hidden: &[u64],
) -> Vec<PlacedShape> {
    shapes
        .iter()
        .filter(|shape| {
            objects
                .get(shape.object as usize)
                .is_none_or(|id| !hidden.contains(id))
        })
        .cloned()
        .collect()
}

/// Lines of every object. With `numbers`, each vertex carries its object's number and the
/// selection is left to the graphics card; without, selected objects are drawn so here. With
/// `apart`, as in a window scene ([`SceneParts::shared`]), the circles
/// [`crate::circles::drawn_apart`] are left out. Positions are measured from `origin`.
pub(crate) fn lines_of(
    outlines: &Outlines,
    selected: &[u64],
    shaded: bool,
    numbers: Option<&std::collections::HashMap<u64, u32>>,
    (apart, origin): (bool, Point),
) -> Vec<Vertex> {
    let mut out = Vec::new();
    for (index, (id, points)) in outlines.objects.iter().enumerate() {
        let start = out.len();
        let width = line_width(
            outlines.widths.get(index).copied().flatten(),
            selected.contains(id),
        );
        // Views showing faces leave seams out: the shading already shows the smooth surface,
        // which a seam would cut across. Wireframe keeps them until outlines of curved shapes
        // are drawn there.
        if shaded && outlines.seams.get(index).copied().unwrap_or(false) {
            continue;
        }
        if apart && crate::circles::drawn_apart(outlines, index).is_some() {
            continue;
        }
        let colour = piece_colour(outlines, index, selected.contains(id), shaded);
        let dashes = outlines.dashes.get(index).copied().unwrap_or(&[]);
        let points = moved_to(points, origin);
        if dashes.is_empty() {
            polyline_with(&mut out, &points, colour, width);
        } else {
            dashed(&mut out, &points, dashes, colour, width);
        }
        if let Some(&number) = numbers.and_then(|numbers| numbers.get(id)) {
            for vertex in &mut out[start..] {
                vertex.object = number;
            }
        }
    }
    out
}

/// The colour entry `index` of `outlines` draws in: the selection colour when `chosen`, grey
/// when locked, dark for an edge on faces in a `shaded` view, else its own.
pub(crate) fn piece_colour(
    outlines: &Outlines,
    index: usize,
    chosen: bool,
    shaded: bool,
) -> [f32; 3] {
    let on_faces = outlines.on_faces.get(index).copied().unwrap_or(false);
    if chosen {
        SELECTED_COLOUR
    } else if outlines.locked.get(index).copied().unwrap_or(false) {
        LOCKED_COLOUR
    } else if shaded && on_faces {
        EDGE_ON_FACES_COLOUR
    } else {
        outlines.colours.get(index).copied().unwrap_or(CURVE_COLOUR)
    }
}

/// The surfaces and solids as shaded triangles, selected ones in the selection colour.
#[must_use]
pub fn face_vertices(outlines: &Outlines, selected: &[u64]) -> Vec<MeshVertex> {
    face_vertices_without(outlines, selected, &[])
}

/// As [`face_vertices`], measured from `origin` (#225).
#[must_use]
pub fn face_vertices_from(outlines: &Outlines, selected: &[u64], origin: Point) -> Vec<MeshVertex> {
    faces_of(outlines, selected, &[], None, (false, origin))
}

/// As [`face_vertices`], leaving out the objects `hidden`, as while a preview stands in for them.
#[must_use]
pub fn face_vertices_without(
    outlines: &Outlines,
    selected: &[u64],
    hidden: &[u64],
) -> Vec<MeshVertex> {
    faces_of(outlines, selected, hidden, None, (false, [0.0; 3]))
}

/// Faces of every object not `hidden`, numbered as [`lines_of`] numbers lines; those with a
/// shared mesh left out when `apart`, as in a window scene. Positions are measured from `origin`.
pub(crate) fn faces_of(
    outlines: &Outlines,
    selected: &[u64],
    hidden: &[u64],
    numbers: Option<&std::collections::HashMap<u64, u32>>,
    (apart, origin): (bool, Point),
) -> Vec<MeshVertex> {
    let mut out = Vec::new();
    for faces in outlines
        .faces
        .iter()
        .filter(|faces| !hidden.contains(&faces.id) && (!apart || faces.shared().is_none()))
    {
        let colour = if selected.contains(&faces.id) {
            SELECTED_COLOUR
        } else if faces.locked {
            LOCKED_COLOUR
        } else {
            faces.colour
        };
        let object = numbers
            .and_then(|numbers| numbers.get(&faces.id))
            .copied()
            .unwrap_or(NO_OBJECT);
        for triangle in &faces.mesh.triangles {
            for &corner in triangle {
                let (Some(position), Some(normal)) = (faces.corner(corner), faces.normal(corner))
                else {
                    continue;
                };
                out.push(MeshVertex {
                    position: narrow_from(position, origin),
                    normal: narrow(normal),
                    colour,
                    object,
                });
            }
        }
    }
    out
}

/// The triangles of the surfaces and solids a command would make, in the preview colour, to be
/// drawn ghosted like Serpentine3D's previews. Every solid preview comes through here.
#[must_use]
pub fn ghost_faces(meshes: &[curva_kernel::BodyMesh], origin: Point) -> Vec<MeshVertex> {
    let mut out = Vec::new();
    for mesh in meshes {
        for &corner in mesh.triangles.iter().flatten() {
            let index = corner as usize;
            if let (Some(position), Some(normal)) =
                (mesh.positions.get(index), mesh.normals.get(index))
            {
                out.push(MeshVertex {
                    position: narrow_from(*position, origin),
                    normal: narrow(*normal),
                    colour: PREVIEW_COLOUR,
                    object: NO_OBJECT,
                });
            }
        }
    }
    out
}

/// How a command's preview is drawn. Every preview in Curva comes through here, so that they
/// all look the same.
#[must_use]
pub fn ghost_lines(curves: &[Curve], origin: Point) -> Vec<Vertex> {
    curves
        .iter()
        .filter_map(|curve| curve.sample().ok())
        .flat_map(|points| preview_lines(&moved_to(&points, origin), None))
        .collect()
}

/// The live preview through a command's points, ending at the cursor.
#[must_use]
pub fn preview_lines(points: &[Point], cursor: Option<Point>) -> Vec<Vertex> {
    let mut all = points.to_vec();
    if !all.is_empty() {
        all.extend(cursor);
    }
    let mut out = Vec::new();
    polyline(&mut out, &all, PREVIEW_COLOUR);
    out
}
