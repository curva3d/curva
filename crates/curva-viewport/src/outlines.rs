//! What is drawn of each object, worked out from the model, with meshes kept until bodies change.

use curva_document::{Layer, Object, Shape, State};
use curva_kernel::{Curve, Point};

use crate::{CURVE_COLOUR, PickFilter, PickKind, mesh_box, snap};

/// A body's triangles, ready to shade.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Faces {
    pub(crate) id: u64,
    pub(crate) colour: [f32; 3],
    pub(crate) locked: bool,
    /// Shared with the mesh cache, so drawing again copies no triangles.
    pub(crate) mesh: std::sync::Arc<curva_kernel::BodyMesh>,
    /// The lowest and highest corners of the mesh, so a line of sight can skip it quickly.
    pub(crate) around: (Point, Point),
}

/// The drawable outline of each object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    /// The triangles of each surface and solid, for shaded views.
    pub(crate) faces: Vec<Faces>,
    /// Object id and points along it.
    pub objects: Vec<(u64, Vec<Point>)>,
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
    /// The body's mesh, and whether it is a closed solid.
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
        let mesh = std::sync::Arc::new(body.mesh().ok()?);
        let solid = body.is_solid();
        self.held
            .insert(id, (body.clone(), std::sync::Arc::clone(&mesh), solid));
        Some((mesh, solid))
    }

    /// An annotation's filled shapes (letters, arrowheads): each one's contours made into a flat
    /// face and meshed, all in one mesh, made once while the shapes are unchanged.
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
            let Ok(face) = curva_kernel::Body::planar(glyph) else {
                continue;
            };
            let Ok(mesh) = face.mesh() else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation)]
            let base = all.positions.len() as u32;
            all.positions.extend(&mesh.positions);
            all.normals.extend(&mesh.normals);
            all.triangles
                .extend(mesh.triangles.iter().map(|t| t.map(|i| i + base)));
            all.triangle_faces
                .extend(std::iter::repeat_n(0, mesh.triangles.len()));
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
                    std::sync::Arc::new(body.mesh().ok()?),
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
        {
            // A layer shows only with all its parents on (spec `sublayers.md` §1).
            if object.hidden || !state.layer_shown(object.layer) {
                return outlines;
            }
            let colour = layer.map_or(CURVE_COLOUR, |l| l.colour);
            let locked = object.locked || state.layer_locked(object.layer);
            let width = object.print_width.resolved(layer);
            let dashes = object.linetype.resolved(layer).pattern();
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
                        let edges = mesh
                            .edges
                            .iter()
                            .zip(&mesh.edge_ids)
                            .filter_map(|(run, id)| match run.len() {
                                0 | 1 => None,
                                2 => Curve::line(run[0], run[1]).ok().map(|c| (c, Some(*id))),
                                _ => Curve::polyline(run.clone(), false)
                                    .ok()
                                    .map(|c| (c, Some(*id))),
                            })
                            .collect();
                        outlines.faces.push(Faces {
                            id: object.id,
                            colour,
                            locked,
                            around: mesh_box(&mesh),
                            mesh,
                        });
                        edges
                    }
                    // Instances inside definitions are opened before this.
                    Shape::Instance(_) => continue,
                    // Annotation: its letters and arrowheads filled for shading, and its lines
                    // and outlines for wireframe, picking and snapping.
                    Shape::Annotation(annotation) => {
                        if matches!(annotation, curva_document::annotation::Annotation::Dot(_)) {
                            kind = PickKind::Point;
                        }
                        // Fields are worked out for showing; the object keeps them as typed.
                        let annotation =
                            &shown_annotation(annotation, state, object.space, &cache.fields);
                        let drawing = curva_document::shape::annotation_drawing(
                            annotation,
                            state,
                            object.space,
                        );
                        if let Some(mesh) = cache.annotation_mesh(*key, &drawing.fills) {
                            outlines.faces.push(Faces {
                                id: object.id,
                                colour,
                                locked,
                                around: mesh_box(&mesh),
                                mesh,
                            });
                        }
                        drawing.lines.into_iter().map(|c| (c, None)).collect()
                    }
                };
                for (outline, edge) in pieces {
                    let Ok(points) = outline.sample() else {
                        continue;
                    };
                    outlines.sites.extend(snap::sites_of(&outline, &points));
                    let first = points.first().copied().unwrap_or_default();
                    let around = snap::box_around(&points).unwrap_or((first, first));
                    outlines.boxes.push(around);
                    outlines.colours.push(colour);
                    outlines.locked.push(locked);
                    outlines.widths.push(width);
                    outlines.dashes.push(dashes);
                    outlines.on_faces.push(shape.body().is_some());
                    outlines
                        .seams
                        .push(edge.is_some_and(|e| seam_edges.contains(&e)));
                    outlines.edge_of.push(edge);
                    outlines.kinds.push(kind);
                    outlines.objects.push((object.id, points));
                }
            }
        }
        outlines
    }
}
