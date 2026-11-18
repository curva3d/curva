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
    /// Which kinds clicking and boxing may pick.
    pub(crate) filter: PickFilter,
    /// The objects and snap places indexed by where they are, made the first time snapping asks.
    pub(crate) snap_index: snap::SnapIndex,
}

/// Mesh-cache keys from here up are instances' parts.
pub(crate) const INSTANCE_KEYS: u64 = 1 << 52;

/// Whether a shape reads things besides itself and so is drawn again each time: text holding
/// fields (`%<...>%`), and dimensions, whose look follows their style and the model's unit.
pub(crate) fn has_fields(shape: &Shape) -> bool {
    match shape {
        Shape::Annotation(curva_document::annotation::Annotation::Text(text)) => {
            text.content.contains("%<")
        }
        Shape::Annotation(curva_document::annotation::Annotation::Leader(leader)) => {
            leader.content.contains("%<")
        }
        Shape::Annotation(curva_document::annotation::Annotation::Dot(_)) => false,
        Shape::Annotation(_) => true,
        _ => false,
    }
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
            // An instance draws its definition, and text with fields reads other things, which
            // can change without them, so those are always drawn again; any other object only
            // when it or its layers changed.
            let part = match earlier.remove(&object.id) {
                Some((was, was_chain, part))
                    if std::sync::Arc::ptr_eq(&was, object)
                        && was_chain == chain
                        && !matches!(object.shape, Shape::Instance(_))
                        && !has_fields(&object.shape) =>
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
        let look = Look {
            colour: layer.map_or(CURVE_COLOUR, |l| l.colour),
            locked: object.locked || state.layer_locked(object.layer),
            width: object.print_width.resolved(layer),
            dashes: object.linetype.resolved(layer).pattern(),
            coarse: false,
        };
        // An instance draws its definition's objects, placed, all under its own id.
        let parts: Vec<(Shape, u64)> = match &object.shape {
            Shape::Instance(instance) => state
                .instance_shapes(instance)
                .into_iter()
                .enumerate()
                .map(|(index, (shape, _))| {
                    (shape, INSTANCE_KEYS + (object.id << 12) + index as u64)
                })
                .collect(),
            shape => vec![(shape.clone(), object.id)],
        };
        for (shape, key) in &parts {
            // A body is meshed once: its triangles for shading, and its edges, each listed
            // under the body's id, for wireframe, picking and snapping.
            let mut kind = PickKind::Curve;
            // A body's seams, by edge number, which shaded views leave out.
            let mut seam_edges: Vec<usize> = Vec::new();
            // Whether its lines are a mesh's wires, all left out where faces show.
            let mut wires = false;
            let pieces: Vec<(Curve, Option<usize>)> = match shape {
                Shape::Curve(curve) => {
                    if matches!(curve, Curve::Point { .. }) {
                        kind = PickKind::Point;
                    }
                    vec![(curve.clone(), None)]
                }
                Shape::Body(body) => {
                    let Some((mesh, solid)) = cache.mesh(*key, body) else {
                        continue;
                    };
                    kind = if solid {
                        PickKind::Solid
                    } else {
                        PickKind::Surface
                    };
                    seam_edges = mesh
                        .edge_ids
                        .iter()
                        .zip(&mesh.seams)
                        .filter_map(|(id, seam)| seam.then_some(*id))
                        .collect();
                    let placement = body.placement();
                    let edges = edge_curves(&mesh, &placement);
                    outlines.push_faces(object.id, look, mesh, Some(placement));
                    edges
                }
                // Instances inside definitions are opened before this.
                Shape::Instance(_) => continue,
                // A mesh shades from its own triangles, placed already, and draws its wires,
                // which views showing faces leave out as Rhino does (#267).
                Shape::Mesh(mesh) => {
                    kind = if mesh.is_closed() {
                        PickKind::Solid
                    } else {
                        PickKind::Surface
                    };
                    wires = true;
                    let drawn = std::sync::Arc::new(crate::meshes::body_mesh(mesh));
                    outlines.push_faces(object.id, look, drawn, Some(IN_PLACE));
                    curva_document::shape::mesh_edges(mesh)
                        .into_iter()
                        .map(|line| (line, None))
                        .collect()
                }
                // SubDs are drawn by the display stream's #324.
                Shape::SubD(_) => continue,
                Shape::Annotation(annotation) => {
                    if matches!(annotation, curva_document::annotation::Annotation::Dot(_)) {
                        kind = PickKind::Point;
                    }
                    outlines.annotation_lines(state, object, *key, annotation, look, cache)
                }
            };
            let on_faces = shape.body().is_some() || wires;
            let seam = |edge: Option<usize>| wires || edge.is_some_and(|e| seam_edges.contains(&e));
            // Annotations' letters and marks keep only the points their shape needs.
            let look = Look {
                coarse: matches!(shape, Shape::Annotation(_)),
                ..look
            };
            outlines.push_pieces(object.id, pieces, look, kind, &seam, on_faces);
        }
        outlines
    }

    /// A body's or annotation's filled faces, for shading: a body's `mesh` as made, shared
    /// with its copies, and where this copy sits.
    fn push_faces(
        &mut self,
        id: u64,
        look: Look,
        mesh: std::sync::Arc<curva_kernel::BodyMesh>,
        placement: Option<Placement>,
    ) {
        let made = mesh_box(&mesh);
        self.faces.push(Faces {
            id,
            colour: look.colour,
            locked: look.locked,
            around: placement.as_ref().map_or(made, |m| placed_box(m, made)),
            mesh,
            placement,
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
            self.push_faces(object.id, look, mesh, None);
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
