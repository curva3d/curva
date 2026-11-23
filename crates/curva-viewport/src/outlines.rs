//! What is drawn of each object, worked out from the model, with meshes kept until bodies change.

use curva_document::{Layer, Object, Shape, State};
use curva_kernel::{Curve, Point};

use crate::{CURVE_COLOUR, PickFilter, PickKind, mesh_box, snap};

/// Where a copy of a shape sits: a 4 × 4 matrix for points written as rows, `p' = p·A + t`
/// ([`curva_kernel::Body::placement`]).
pub(crate) type Placement = [[f64; 4]; 4];

/// The placement that leaves a shape where it was made.
const IN_PLACE: Placement = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A body's triangles, ready to shade.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Faces {
    pub(crate) id: u64,
    pub(crate) colour: [f32; 3],
    pub(crate) locked: bool,
    /// The triangles as made. For a body, the mesh it shares with its copies, unplaced
    /// ([`curva_kernel::Body::shared_mesh`]), so each shape is held once however many copies
    /// there are (#153); read its corners where this copy sits with [`Self::corner`].
    pub(crate) mesh: std::sync::Arc<curva_kernel::BodyMesh>,
    /// For a body, where this copy sits; `None` for an annotation's fills, drawn where made.
    pub(crate) placement: Option<Placement>,
    /// The lowest and highest corners of the placed mesh, or a box around them, so a line of
    /// sight can skip it quickly.
    pub(crate) around: (Point, Point),
    /// For a mesh with vertex colours, the colour at each of `mesh`'s positions, shown instead
    /// of its layer's colour (#267).
    pub(crate) colours: Option<std::sync::Arc<Vec<[f32; 3]>>>,
    /// What sort of object the faces belong to, for the selection filter, so an object drawn by
    /// its faces alone, such as a smooth SubD without creases, can still be picked (#324).
    pub(crate) kind: PickKind,
    /// What kind of object the faces belong to, for the look a view's style gives them.
    pub(crate) face_kind: crate::FaceKind,
}

impl Faces {
    /// For a body, the mesh it shares with its copies and where this copy sits, so views can
    /// hold each shape once (#153).
    pub(crate) fn shared(&self) -> Option<(&std::sync::Arc<curva_kernel::BodyMesh>, &Placement)> {
        Some((&self.mesh, self.placement.as_ref()?))
    }

    /// Corner `index` of the triangles, where this copy sits.
    pub(crate) fn corner(&self, index: u32) -> Option<Point> {
        let p = *self.mesh.positions.get(index as usize)?;
        Some(match &self.placement {
            Some(m) if *m != IN_PLACE => placed(m, p),
            _ => p,
        })
    }

    /// The surface's direction at corner `index`, where this copy sits, a unit long.
    pub(crate) fn normal(&self, index: u32) -> Option<Point> {
        let d = *self.mesh.normals.get(index as usize)?;
        Some(match &self.placement {
            Some(m) if *m != IN_PLACE => turned(m, d),
            _ => d,
        })
    }
}

/// `p` where `m` puts it.
pub(crate) fn placed(m: &Placement, p: Point) -> Point {
    std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
}

/// The direction `d` turned as `m` turns it, a unit long, as the kernel places a mesh's normals.
fn turned(m: &Placement, d: Point) -> Point {
    let t: Point = std::array::from_fn(|j| d[0] * m[0][j] + d[1] * m[1][j] + d[2] * m[2][j]);
    let length = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    if length > 0.0 {
        t.map(|v| v / length)
    } else {
        t
    }
}

/// The box around `around` placed by `m`: its eight corners placed, then boxed.
fn placed_box(m: &Placement, (low, high): (Point, Point)) -> (Point, Point) {
    if *m == IN_PLACE {
        return (low, high);
    }
    let mut out = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for corner in 0..8 {
        let p = placed(
            m,
            std::array::from_fn(|k| {
                if corner >> k & 1 == 0 {
                    low[k]
                } else {
                    high[k]
                }
            }),
        );
        out.0 = std::array::from_fn(|k| out.0[k].min(p[k]));
        out.1 = std::array::from_fn(|k| out.1[k].max(p[k]));
    }
    out
}

/// The drawable outline of each object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    /// The triangles of each surface and solid, for shaded views.
    pub(crate) faces: Vec<Faces>,
    /// Object id and points along it, shared with the cache of what each object drew as.
    pub objects: Vec<(u64, std::sync::Arc<[Point]>)>,
    /// The fixed places that can be snapped to.
    pub(crate) sites: Vec<snap::Site>,
    /// The colour of each object, in the same order.
    pub(crate) colours: Vec<[f32; 3]>,
    /// Whether each object is locked, in the same order.
    pub(crate) locked: Vec<bool>,
    /// How thick each object draws, in millimetres or `None` for the default line, in the same order.
    pub(crate) widths: Vec<Option<f64>>,
    /// The dash pattern each draws with (empty for a solid line), in the same order.
    pub(crate) dashes: Vec<&'static [f64]>,
    /// Whether each entry is an edge of a surface or solid, in the same order.
    pub(crate) on_faces: Vec<bool>,
    /// Whether each entry is a seam of a surface or solid (`curva_kernel::BodyMesh::seams`), which
    /// views showing faces leave out, in the same order.
    pub(crate) seams: Vec<bool>,
    /// For an edge of a surface or solid, its number among the body's edges, in the same order.
    pub(crate) edge_of: Vec<Option<usize>>,
    /// For a whole circle, its centre and its two axes as long as its radius, in the same order
    /// (`crate::circles`).
    pub(crate) circles: Vec<Option<[Point; 3]>>,
    /// The box around each object, in the same order, so snapping can skip far-off ones.
    pub(crate) boxes: Vec<(Point, Point)>,
    /// The sort of object each entry belongs to, in the same order, for the selection filter.
    pub(crate) kinds: Vec<PickKind>,
    /// The kind of line each entry draws as, in the same order ([`crate::LineKind`]).
    pub(crate) line_kinds: Vec<crate::LineKind>,
    /// Which kinds clicking and boxing may pick.
    pub(crate) filter: PickFilter,
    /// The objects and snap places indexed by where they are, made the first time snapping asks.
    pub(crate) snap_index: snap::SnapIndex,
}

/// Mesh-cache keys from here up are instances' parts.
pub(crate) const INSTANCE_KEYS: u64 = 1 << 52;

/// Whether a shape reads things besides itself and the settings [`same_context`] compares, and so
/// is drawn again each time: text holding fields (`%<...>%`), which read the file, the time and
/// other objects; a scale bar, which reads its detail view; and a table, which reads the sheets.
/// Dimensions and hatches read only those settings, so they are drawn again when those change.
pub(crate) fn has_fields(shape: &Shape) -> bool {
    use curva_document::annotation::Annotation;
    match shape {
        Shape::Annotation(Annotation::Text(text)) => text.content.contains("%<"),
        Shape::Annotation(Annotation::Leader(leader)) => leader.content.contains("%<"),
        Shape::Annotation(Annotation::ScaleBar(_) | Annotation::Table(_)) => true,
        _ => false,
    }
}

/// The curves an object draws as, each with its number among a body's edges, if it is one.
type Pieces = Vec<(Curve, Option<usize>)>;

/// What an object draws, each with its mesh-cache key: itself, or for an instance its definition's
/// objects, placed, all under its own id.
fn parts_of(state: &State, object: &Object) -> Vec<(Shape, u64)> {
    match &object.shape {
        Shape::Instance(instance) => state
            .instance_shapes(instance)
            .into_iter()
            .enumerate()
            .map(|(index, (shape, _))| (shape, INSTANCE_KEYS + (object.id << 12) + index as u64))
            .collect(),
        shape => vec![(shape.clone(), object.id)],
    }
}

/// Whether what blocks draw from besides their own objects is the same in `held` as in `now`:
/// the definitions, and for annotations inside them the unit, length display, layouts, hatch
/// patterns and annotation styles.
fn same_context(held: &State, now: &State) -> bool {
    held.blocks == now.blocks
        && held.unit == now.unit
        && held.length_display == now.length_display
        && held.layouts == now.layouts
        && held.hatch_patterns == now.hatch_patterns
        && held.annotation_styles == now.annotation_styles
}

/// Whether a block, or a block placed in it, holds a shape drawn again each time ([`has_fields`]).
fn block_has_fields(state: &State, definition: u64, depth: usize) -> bool {
    if depth > 16 {
        return true;
    }
    state
        .blocks
        .iter()
        .find(|d| d.id == definition)
        .is_some_and(|d| {
            d.objects.iter().any(|o| match &o.shape {
                Shape::Instance(inner) => block_has_fields(state, inner.definition, depth + 1),
                shape => has_fields(shape),
            })
        })
}

/// An annotation as it shows: its fields worked out.
pub(crate) fn shown_annotation(
    annotation: &curva_document::annotation::Annotation,
    state: &State,
    space: Option<u64>,
    fields: &curva_document::fields::FieldContext,
) -> curva_document::annotation::Annotation {
    use curva_document::annotation::{Annotation, Leader, Text};
    let resolve = |content: &str| curva_document::fields::resolve(content, state, space, fields);
    match annotation {
        Annotation::Text(text) if text.content.contains("%<") => Annotation::Text(Text {
            content: resolve(&text.content),
            ..text.clone()
        }),
        Annotation::Leader(leader) if leader.content.contains("%<") => Annotation::Leader(Leader {
            content: resolve(&leader.content),
            ..leader.clone()
        }),
        _ => annotation.clone(),
    }
}

/// Meshes of bodies already worked out, so a body is meshed again only when it changes.
#[derive(Debug, Default)]
pub struct MeshCache {
    /// Each body as last meshed, its mesh, and whether it is a closed solid.
    pub(crate) held: std::collections::HashMap<
        u64,
        (
            curva_kernel::Body,
            std::sync::Arc<curva_kernel::BodyMesh>,
            bool,
        ),
    >,
    /// What text fields read besides the model: the file and the time. The interface sets it.
    pub fields: curva_document::fields::FieldContext,
    /// Annotation's filled letters, with the annotation they were made for.
    pub(crate) annotations:
        std::collections::HashMap<u64, (Vec<Vec<Curve>>, std::sync::Arc<curva_kernel::BodyMesh>)>,
    /// Each object as last drawn, with its layer and that layer's parents then (spec
    /// `sublayers.md`) and what it drew as, so an object that
    /// has not changed is not sampled again (`docs/reviews/code-health-2026-11-14.md`, fix 3).
    pub(crate) drawn:
        std::collections::HashMap<u64, (std::sync::Arc<Object>, Vec<Layer>, Outlines)>,
    /// The model as last drawn, without its objects: what blocks draw from besides themselves,
    /// so unchanged instances are not drawn again while it stays the same.
    pub(crate) context: Option<State>,
    /// Each smooth SubD's live surface, so dragging its cage points redraws it with no
    /// subdivision (#334).
    pub(crate) live: std::collections::HashMap<u64, Option<crate::subd_live::Held>>,
}

impl MeshCache {
    /// The body's mesh as made, shared with its copies ([`curva_kernel::Body::shared_mesh`]),
    /// and whether it is a closed solid.
    pub(crate) fn mesh(
        &mut self,
        id: u64,
        body: &curva_kernel::Body,
    ) -> Option<(std::sync::Arc<curva_kernel::BodyMesh>, bool)> {
        if let Some((held, mesh, solid)) = self.held.get(&id)
            && held == body
        {
            return Some((std::sync::Arc::clone(mesh), *solid));
        }
        let mesh = body.shared_mesh().ok()?;
        let solid = body.is_solid();
        self.held
            .insert(id, (body.clone(), std::sync::Arc::clone(&mesh), solid));
        Some((mesh, solid))
    }

    /// An annotation's filled shapes (letters, arrowheads): each one's contours filled in their
    /// plane ([`crate::fills`]), all in one mesh, made once while the shapes are unchanged.
    pub(crate) fn annotation_mesh(
        &mut self,
        id: u64,
        fills: &[Vec<Curve>],
    ) -> Option<std::sync::Arc<curva_kernel::BodyMesh>> {
        if let Some((held, mesh)) = self.annotations.get(&id)
            && held == fills
        {
            return Some(std::sync::Arc::clone(mesh));
        }
        let mut all = curva_kernel::BodyMesh::default();
        for glyph in fills {
            crate::fills::add_filled(glyph, &mut all);
        }
        if all.triangles.is_empty() {
            return None;
        }
        let mesh = std::sync::Arc::new(all);
        self.annotations
            .insert(id, (fills.to_vec(), std::sync::Arc::clone(&mesh)));
        Some(mesh)
    }

    /// Forgets bodies that are gone, and meshes new or changed ones all at once, on every
    /// core, so that opening a model of many solids does not mesh them one by one.
    pub(crate) fn refresh(&mut self, state: &State) {
        use rayon::prelude::*;
        // Keys at or above INSTANCE_KEYS are the parts of instances, keyed by their object.
        let present: std::collections::HashSet<u64> =
            state.objects.iter().map(|object| object.id).collect();
        self.held.retain(|key, _| {
            let id = if *key >= INSTANCE_KEYS {
                (*key - INSTANCE_KEYS) >> 12
            } else {
                *key
            };
            present.contains(&id)
        });
        self.live.retain(|id, _| present.contains(id));
        let wanted: Vec<(u64, &curva_kernel::Body)> = state
            .objects
            .iter()
            .filter(|object| !object.hidden)
            .filter_map(|object| Some((object.id, object.shape.body()?)))
            .filter(|(id, body)| self.held.get(id).is_none_or(|(held, ..)| held != *body))
            .collect();
        let made: Vec<_> = wanted
            .par_iter()
            .filter_map(|(id, body)| {
                Some((
                    *id,
                    (*body).clone(),
                    body.shared_mesh().ok()?,
                    body.is_solid(),
                ))
            })
            .collect();
        for (id, body, mesh, solid) in made {
            self.held.insert(id, (body, mesh, solid));
        }
    }
}

impl Outlines {
    /// Samples every object. An object that cannot be sampled is left out, not fatal to drawing.
    #[must_use]
    pub fn of(state: &State) -> Self {
        Self::of_cached(state, &mut MeshCache::default())
    }

    /// As [`Self::of`], reusing the meshes of bodies that have not changed.
    #[must_use]
    pub fn of_cached(state: &State, cache: &mut MeshCache) -> Self {
        Self::of_space(state, None, cache)
    }

    /// The objects in one space: the model's (`None`) or a layout's sheet (spec `layouts.md`).
    #[must_use]
    pub fn of_space(state: &State, space: Option<u64>, cache: &mut MeshCache) -> Self {
        cache.refresh(state);
        // What blocks draw from besides themselves; while unchanged, so is an unchanged instance.
        let context_same = cache
            .context
            .as_ref()
            .is_some_and(|held| same_context(held, state));
        let mut outlines = Self::default();
        let mut earlier = std::mem::take(&mut cache.drawn);
        for object in state.objects.iter().filter(|o| o.space == space) {
            let layer = state.layers.iter().find(|l| l.id == object.layer);
            // Its layer and the layer's parents, whose being off or locked rules it too.
            let chain: Vec<Layer> = state
                .layer_chain(object.layer)
                .into_iter()
                .cloned()
                .collect();
            // Text with fields, scale bars and tables read other things, which can change without
            // them, so those are always drawn again; other annotation and instances too when the
            // settings they read changed, or an instance's blocks hold such shapes; any other
            // object only when it or its layers changed.
            let part = match earlier.remove(&object.id) {
                Some((was, was_chain, part))
                    if std::sync::Arc::ptr_eq(&was, object)
                        && was_chain == chain
                        && !has_fields(&object.shape)
                        && match &object.shape {
                            Shape::Instance(instance) => {
                                context_same && !block_has_fields(state, instance.definition, 0)
                            }
                            Shape::Annotation(_) => context_same,
                            _ => true,
                        } =>
                {
                    part
                }
                _ => Self::of_object(state, object, layer, cache),
            };
            outlines.append(&part);
            cache
                .drawn
                .insert(object.id, (std::sync::Arc::clone(object), chain, part));
        }
        if !context_same {
            cache.context = Some(State {
                objects: Vec::new(),
                ..state.clone()
            });
        }
        outlines
    }

    /// Adds `other`'s objects after its own.
    // CUSTOM BEVY PART: far-origin
    /// What a window scene measures positions from (#225, custom part `far-origin`): the middle
    /// of everything drawn, to the nearest 1000 units, so a model far from the origin draws as
    /// exactly as one at it, and small edits leave the point where it is. The origin when
    /// nothing is drawn.
    #[must_use]
    pub fn drawing_origin(&self) -> Point {
        let mut boxes = self.boxes.iter();
        let Some(&first) = boxes.next() else {
            return [0.0; 3];
        };
        let (low, high) = boxes.fold(first, |(mut low, mut high), (l, h)| {
            for axis in 0..3 {
                low[axis] = low[axis].min(l[axis]);
                high[axis] = high[axis].max(h[axis]);
            }
            (low, high)
        });
        std::array::from_fn(|k| ((low[k] + high[k]) / 2000.0).round() * 1000.0)
    }

    pub(crate) fn append(&mut self, other: &Self) {
        // The objects change, so the index over them is made again when next asked for.
        self.snap_index = snap::SnapIndex::default();
        self.faces.extend(other.faces.iter().cloned());
        self.objects.extend(other.objects.iter().cloned());
        self.sites.extend(other.sites.iter().cloned());
        self.colours.extend_from_slice(&other.colours);
        self.locked.extend_from_slice(&other.locked);
        self.widths.extend_from_slice(&other.widths);
        self.dashes.extend_from_slice(&other.dashes);
        self.on_faces.extend_from_slice(&other.on_faces);
        self.seams.extend_from_slice(&other.seams);
        self.edge_of.extend_from_slice(&other.edge_of);
        self.circles.extend_from_slice(&other.circles);
        self.boxes.extend_from_slice(&other.boxes);
        self.kinds.extend_from_slice(&other.kinds);
        self.line_kinds.extend_from_slice(&other.line_kinds);
    }

    /// What one object draws as, on `layer`: nothing if it or its layer is out of sight.
    pub(crate) fn of_object(
        state: &State,
        object: &Object,
        layer: Option<&Layer>,
        cache: &mut MeshCache,
    ) -> Self {
        let mut outlines = Self::default();
        // A layer shows only with all its parents on (spec `sublayers.md` §1).
        if object.hidden || !state.layer_shown(object.layer) {
            return outlines;
        }
        let look = Look::of(state, object, layer);
        for (shape, key) in &parts_of(state, object) {
            // A body is meshed once: its triangles for shading, and its edges, each listed
            // under the body's id, for wireframe, picking and snapping.
            let mut kind = PickKind::Curve;
            // A body's seams, by edge number, which shaded views leave out.
            let mut seam_edges: Vec<usize> = Vec::new();
            // Its faces and lines take the look its kind of object has in a view's style.
            let look = look.drawn_as(line_kind_of(shape));
            let pieces: Vec<(Curve, Option<usize>)> = match shape {
                Shape::Curve(curve) => {
                    if matches!(curve, Curve::Point { .. }) {
                        kind = PickKind::Point;
                    }
                    vec![(curve.clone(), None)]
                }
                Shape::Body(body) => {
                    let Some((edges, body_kind, seams)) =
                        outlines.body_pieces(object.id, *key, body, look, cache)
                    else {
                        continue;
                    };
                    (kind, seam_edges) = (body_kind, seams);
                    edges
                }
                // Instances inside definitions are opened before this.
                Shape::Instance(_) => continue,
                // A mesh shades from its own triangles, placed already, and draws its wires in
                // every view, in its own colour, as Rhino 8 does in Wireframe, Shaded and
                // Ghosted (#472).
                Shape::Mesh(mesh) => {
                    let (lines, mesh_kind) = outlines.mesh_pieces(object.id, mesh, look);
                    kind = mesh_kind;
                    lines
                }
                // A SubD shades from its smooth surface, or its cage shown flat, and draws its
                // creases and borders, or its cage's edges, as a body draws its edges (#324).
                Shape::SubD(subd) => {
                    let (lines, subd_kind) = outlines.subd_pieces(object.id, subd, look, cache);
                    kind = subd_kind;
                    lines
                }
                Shape::Annotation(annotation) => {
                    if matches!(annotation, curva_document::annotation::Annotation::Dot(_)) {
                        kind = PickKind::Point;
                    }
                    outlines.annotation_lines(state, object, *key, annotation, look, cache)
                }
            };
            let on_faces = shape.body().is_some() || shape.subd().is_some();
            let seam = |edge: Option<usize>| edge.is_some_and(|e| seam_edges.contains(&e));
            // Annotations' letters and marks keep only the points their shape needs.
            // A mesh's wires are a pixel wide whatever its print width, as Rhino 8 draws them
            // (mesh wire thickness 1), and drawn the cheap way ([`crate::WIRE_MM`]).
            let look = Look {
                coarse: matches!(shape, Shape::Annotation(_)),
                width: if matches!(shape, Shape::Mesh(_)) {
                    Some(crate::WIRE_MM)
                } else {
                    look.width
                },
                ..look
            };
            outlines.push_pieces(object.id, pieces, look, kind, &seam, on_faces);
        }
        outlines
    }

    /// A SubD's faces as [`crate::subds::drawing`] gives them, placed already, and its lines as
    /// pieces, with its kind: a solid when its cage is closed (#324).
    fn subd_pieces(
        &mut self,
        id: u64,
        subd: &curva_mesh::SubD,
        look: Look,
        cache: &mut MeshCache,
    ) -> (Pieces, PickKind) {
        let kind = if subd.cage().is_closed() {
            PickKind::Solid
        } else {
            PickKind::Surface
        };
        // Drawn from its live surface when it has one, so a drag needs no subdivision (#334).
        let held = cache.live.entry(id).or_default();
        let drawing = crate::subd_live::live_drawing(subd, held)
            .unwrap_or_else(|| crate::subds::drawing(subd, look.colour));
        // Shown smooth, the surface carries its faces' colours; shown flat, they are given here.
        let (drawn, given) = crate::meshes::body_mesh(&drawing.shown);
        let colours =
            given.or_else(|| crate::subds::face_colours(subd, &drawing, &drawn, look.colour));
        self.push_faces(id, look, std::sync::Arc::new(drawn), Some(IN_PLACE), kind);
        if let (Some(colours), Some(faces)) = (colours, self.faces.last_mut()) {
            faces.colours = Some(std::sync::Arc::new(colours));
        }
        // Its other cage edges draw in every view in its own colour, as Rhino 8 draws them (#472).
        let wires = drawing.wires.into_iter().map(|wire| (wire, None)).collect();
        let wire_look = look.drawn_as(crate::LineKind::SubdWire);
        self.push_pieces(id, wires, wire_look, kind, &|_| false, false);
        let lines = drawing.lines.into_iter().map(|line| (line, None)).collect();
        (lines, kind)
    }

    /// A body's faces, as meshed once, and its edges as pieces, with its kind and its seams by
    /// edge number; `None` if it could not be meshed.
    fn body_pieces(
        &mut self,
        id: u64,
        key: u64,
        body: &curva_kernel::Body,
        look: Look,
        cache: &mut MeshCache,
    ) -> Option<(Pieces, PickKind, Vec<usize>)> {
        let (mesh, solid) = cache.mesh(key, body)?;
        let kind = if solid {
            PickKind::Solid
        } else {
            PickKind::Surface
        };
        let seams = mesh
            .edge_ids
            .iter()
            .zip(&mesh.seams)
            .filter_map(|(id, seam)| seam.then_some(*id))
            .collect();
        let placement = body.placement();
        let edges = edge_curves(&mesh, &placement);
        self.push_faces(id, look, mesh, Some(placement), kind);
        Some((edges, kind, seams))
    }

    /// A mesh's faces from its own triangles, placed already, with its vertex colours if it has
    /// them, and its wires as pieces, with its kind (#267).
    fn mesh_pieces(&mut self, id: u64, mesh: &curva_mesh::Mesh, look: Look) -> (Pieces, PickKind) {
        let kind = if mesh.is_closed() {
            PickKind::Solid
        } else {
            PickKind::Surface
        };
        let (drawn, colours) = crate::meshes::body_mesh(mesh);
        self.push_faces(id, look, std::sync::Arc::new(drawn), Some(IN_PLACE), kind);
        if let (Some(colours), Some(faces)) = (colours, self.faces.last_mut()) {
            faces.colours = Some(std::sync::Arc::new(colours));
        }
        let lines = crate::meshes::wires(mesh)
            .into_iter()
            .map(|line| (line, None))
            .collect();
        (lines, kind)
    }

    /// A body's or annotation's filled faces, for shading: a body's `mesh` as made, shared
    /// with its copies, and where this copy sits.
    fn push_faces(
        &mut self,
        id: u64,
        look: Look,
        mesh: std::sync::Arc<curva_kernel::BodyMesh>,
        placement: Option<Placement>,
        kind: PickKind,
    ) {
        let made = mesh_box(&mesh);
        self.faces.push(Faces {
            id,
            colour: look.colour,
            locked: look.locked,
            around: placement.as_ref().map_or(made, |m| placed_box(m, made)),
            mesh,
            placement,
            colours: None,
            kind,
            face_kind: match look.line {
                crate::LineKind::Edge => crate::FaceKind::Nurbs,
                crate::LineKind::MeshWire => crate::FaceKind::Mesh,
                crate::LineKind::SubdEdge | crate::LineKind::SubdWire => crate::FaceKind::Subd,
                _ => crate::FaceKind::Given,
            },
        });
    }

    /// Annotation: its letters and arrowheads filled for shading, and its lines and outlines,
    /// returned, for wireframe, picking and snapping.
    fn annotation_lines(
        &mut self,
        state: &State,
        object: &Object,
        key: u64,
        annotation: &curva_document::annotation::Annotation,
        look: Look,
        cache: &mut MeshCache,
    ) -> Vec<(Curve, Option<usize>)> {
        // Fields are worked out for showing; the object keeps them as typed.
        let annotation = &shown_annotation(annotation, state, object.space, &cache.fields);
        let drawing = curva_document::shape::annotation_drawing(annotation, state, object.space);
        if let Some(mesh) = cache.annotation_mesh(key, &drawing.fills) {
            self.push_faces(object.id, look, mesh, None, PickKind::Curve);
        }
        drawing.lines.into_iter().map(|c| (c, None)).collect()
    }

    /// Samples each piece of an object and lists it, with its look, box and snap places.
    fn push_pieces(
        &mut self,
        id: u64,
        pieces: Vec<(Curve, Option<usize>)>,
        look: Look,
        kind: PickKind,
        seam: &dyn Fn(Option<usize>) -> bool,
        on_faces: bool,
    ) {
        for (outline, edge) in pieces {
            let Ok(points) = outline.sample() else {
                continue;
            };
            self.sites.extend(snap::sites_of(&outline, &points));
            // Snap places come from every sampled point; drawing, picking and boxes need fewer.
            let points = crate::simplify::fewer(
                points,
                crate::simplify::part_of_size(&outline, look.coarse),
            );
            let first = points.first().copied().unwrap_or_default();
            let around = snap::box_around(&points).unwrap_or((first, first));
            self.boxes.push(around);
            self.colours.push(look.colour);
            self.locked.push(look.locked);
            self.widths.push(look.width);
            self.dashes.push(look.dashes);
            self.on_faces.push(on_faces);
            self.seams.push(seam(edge));
            self.edge_of.push(edge);
            self.circles.push(crate::circles::axes_of(&outline));
            self.kinds.push(kind);
            self.line_kinds.push(look.line);
            self.objects.push((id, points.into()));
        }
    }
}

/// How an object's pieces draw: its colour, whether it is locked, its width and its dashes,
/// and whether its outlines keep only the points their shape needs ([`crate::simplify`]).
#[derive(Clone, Copy)]
struct Look {
    colour: [f32; 3],
    locked: bool,
    width: Option<f64>,
    dashes: &'static [f64],
    coarse: bool,
    /// The kind of line its pieces draw as.
    line: crate::LineKind,
}

impl Look {
    /// This look, its pieces drawn as `line`.
    fn drawn_as(self, line: crate::LineKind) -> Self {
        Self { line, ..self }
    }

    /// How `object`, on `layer`, draws: in its own colour or its layer's, locked or not, with its
    /// print width and its linetype's pattern, the document's own linetypes included (#170).
    fn of(state: &State, object: &Object, layer: Option<&Layer>) -> Self {
        Self {
            colour: if object.colour.is_some() || layer.is_some() {
                state.object_colour(object)
            } else {
                CURVE_COLOUR
            },
            locked: object.locked || state.layer_locked(object.layer),
            width: object.print_width.resolved(layer),
            dashes: crate::lines::kept_pattern(state.object_pattern(object)),
            coarse: false,
            line: crate::LineKind::Curve,
        }
    }
}

/// The kind of line `shape`'s pieces draw as ([`crate::LineKind`]).
fn line_kind_of(shape: &Shape) -> crate::LineKind {
    match shape {
        Shape::Body(_) => crate::LineKind::Edge,
        Shape::Mesh(_) => crate::LineKind::MeshWire,
        Shape::SubD(_) => crate::LineKind::SubdEdge,
        Shape::Annotation(_) => crate::LineKind::Annotation,
        _ => crate::LineKind::Curve,
    }
}

/// A mesh's edges as curves where `placement` puts them, each with its number among the body's
/// edges.
fn edge_curves(
    mesh: &curva_kernel::BodyMesh,
    placement: &Placement,
) -> Vec<(Curve, Option<usize>)> {
    let place = |p: &Point| {
        if *placement == IN_PLACE {
            *p
        } else {
            placed(placement, *p)
        }
    };
    mesh.edges
        .iter()
        .zip(&mesh.edge_ids)
        .filter_map(|(run, id)| match run.len() {
            0 | 1 => None,
            2 => Curve::line(place(&run[0]), place(&run[1]))
                .ok()
                .map(|c| (c, Some(*id))),
            _ => Curve::polyline(run.iter().map(place).collect(), false)
                .ok()
                .map(|c| (c, Some(*id))),
        })
        .collect()
}
