//! Curva's 3D viewport: what to draw, how to pick it, and drawing it on the graphics card.

mod camera;
mod gpu;
mod gumball;
mod snap;

use curva_document::{Shape, State};
use curva_kernel::{Curve, Point};

pub use camera::{CPlane, Camera, StandardView};
pub use gpu::{DEPTH_BITS, Drawing, LineRenderer, Picture, VIEW_SLOTS, save_picture};
pub use gumball::{Gumball, GumballChange, Handle};
pub use snap::{SnapHit, SnapKind, SnapSet, constrained};

const GRID_HALF_SIZE: i32 = 100;
const GRID_STEP: i32 = 10;
/// Steps of the search along a segment for a picked point.
const PICK_SEARCH_STEPS: usize = 48;
const GRID_COLOUR: [f32; 3] = [0.30, 0.31, 0.34];
const CURVE_COLOUR: [f32; 3] = [0.92, 0.92, 0.95];
const SELECTED_COLOUR: [f32; 3] = [1.0, 0.85, 0.2];
const LOCKED_COLOUR: [f32; 3] = [0.45, 0.45, 0.48];
/// Edges of surfaces and solids in shaded views.
const EDGE_ON_FACES_COLOUR: [f32; 3] = [0.08, 0.08, 0.1];
/// The colour every preview is drawn in.
pub const PREVIEW_COLOUR: [f32; 3] = [0.45, 0.75, 1.0];
const X_AXIS_COLOUR: [f32; 3] = [0.85, 0.3, 0.3];
const Y_AXIS_COLOUR: [f32; 3] = [0.35, 0.8, 0.4];

/// One end of a drawn line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in model space.
    pub position: [f32; 3],
    /// Colour, red, green and blue from 0 to 1.
    pub colour: [f32; 3],
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
}

/// A body's triangles, ready to shade.
#[derive(Clone, Debug, PartialEq)]
struct Faces {
    id: u64,
    colour: [f32; 3],
    locked: bool,
    mesh: curva_kernel::BodyMesh,
}

/// The drawable outline of each object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    /// The triangles of each surface and solid, for shaded views.
    faces: Vec<Faces>,
    /// Object id and points along it.
    pub objects: Vec<(u64, Vec<Point>)>,
    /// The fixed places that can be snapped to.
    sites: Vec<snap::Site>,
    /// The colour of each object, in the same order.
    colours: Vec<[f32; 3]>,
    /// Whether each object is locked, in the same order.
    locked: Vec<bool>,
    /// Whether each entry is an edge of a surface or solid, in the same order.
    on_faces: Vec<bool>,
    /// For an edge of a surface or solid, its number among the body's edges, in the same order.
    edge_of: Vec<Option<usize>>,
    /// The box around each object, in the same order, so snapping can skip far-off ones.
    boxes: Vec<(Point, Point)>,
}

/// Meshes of bodies already worked out, so a body is meshed again only when it changes.
#[derive(Debug, Default)]
pub struct MeshCache {
    held: std::collections::HashMap<u64, (curva_kernel::Body, curva_kernel::BodyMesh)>,
}

impl MeshCache {
    fn mesh(&mut self, id: u64, body: &curva_kernel::Body) -> Option<curva_kernel::BodyMesh> {
        if let Some((held, mesh)) = self.held.get(&id)
            && held == body
        {
            return Some(mesh.clone());
        }
        let mesh = body.mesh().ok()?;
        self.held.insert(id, (body.clone(), mesh.clone()));
        Some(mesh)
    }

    /// Forgets bodies that are gone, and meshes new or changed ones all at once, on every
    /// core, so that opening a model of many solids does not mesh them one by one.
    fn refresh(&mut self, state: &State) {
        use rayon::prelude::*;
        self.held
            .retain(|id, _| state.objects.iter().any(|object| object.id == *id));
        let wanted: Vec<(u64, &curva_kernel::Body)> = state
            .objects
            .iter()
            .filter(|object| !object.hidden)
            .filter_map(|object| Some((object.id, object.shape.body()?)))
            .filter(|(id, body)| self.held.get(id).is_none_or(|(held, _)| held != *body))
            .collect();
        let made: Vec<_> = wanted
            .par_iter()
            .filter_map(|(id, body)| Some((*id, (*body).clone(), body.mesh().ok()?)))
            .collect();
        for (id, body, mesh) in made {
            self.held.insert(id, (body, mesh));
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
        cache.refresh(state);
        let mut outlines = Self::default();
        for object in &state.objects {
            let layer = state.layers.iter().find(|l| l.id == object.layer);
            if object.hidden || layer.is_some_and(|l| !l.visible) {
                continue;
            }
            let colour = layer.map_or(CURVE_COLOUR, |l| l.colour);
            let locked = layer.is_some_and(|l| l.locked);
            // A body is meshed once: its triangles for shading, and its edges, each listed
            // under the body's id, for wireframe, picking and snapping.
            let pieces: Vec<(Curve, Option<usize>)> = match &object.shape {
                Shape::Curve(curve) => vec![(curve.clone(), None)],
                Shape::Body(body) => {
                    let Some(mesh) = cache.mesh(object.id, body) else {
                        continue;
                    };
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
                        mesh,
                    });
                    edges
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
                outlines.on_faces.push(object.shape.body().is_some());
                outlines.edge_of.push(edge);
                outlines.objects.push((object.id, points));
            }
        }
        outlines
    }

    /// The lowest and highest corners around everything, if there is anything.
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut points = self.objects.iter().flat_map(|(_, list)| list.iter());
        let first = *points.next()?;
        Some(points.fold((first, first), |(mut low, mut high), p| {
            for axis in 0..3 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
            (low, high)
        }))
    }

    /// The exact point on the curve under a screen position, if one is within `reach` pixels.
    #[must_use]
    pub fn pick_point(
        &self,
        camera: &Camera,
        screen: [f64; 2],
        size: [f64; 2],
        reach: f64,
    ) -> Option<Point> {
        let view = camera.projector(size);
        let mut best: Option<(f64, Point, Point)> = None;
        for (_, points) in self.selectable() {
            for pair in points.windows(2) {
                if let (Some(a), Some(b)) = (view.project(pair[0]), view.project(pair[1])) {
                    let distance = to_segment(screen, a, b);
                    if distance <= reach && best.is_none_or(|(held, ..)| distance < held) {
                        best = Some((distance, pair[0], pair[1]));
                    }
                }
            }
        }
        let (_, a, b) = best?;
        let at = |t: f64| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        let gap = |t: f64| {
            view.project(at(t)).map_or(f64::INFINITY, |p| {
                (p[0] - screen[0]).hypot(p[1] - screen[1])
            })
        };
        let (mut low, mut high) = (0.0_f64, 1.0_f64);
        for _ in 0..PICK_SEARCH_STEPS {
            let third = (high - low) / 3.0;
            if gap(low + third) <= gap(high - third) {
                high -= third;
            } else {
                low += third;
            }
        }
        Some(at((low + high) / 2.0))
    }

    /// The middle of the box around chosen objects, if any of them can be drawn.
    #[must_use]
    pub fn centre_of(&self, ids: &[u64]) -> Option<Point> {
        let mut around: Option<(Point, Point)> = None;
        for (index, (id, _)) in self.objects.iter().enumerate() {
            if ids.binary_search(id).is_err() {
                continue;
            }
            let (low, high) = *self.boxes.get(index)?;
            around = Some(around.map_or((low, high), |(held_low, held_high)| {
                (
                    std::array::from_fn(|axis| held_low[axis].min(low[axis])),
                    std::array::from_fn(|axis| held_high[axis].max(high[axis])),
                )
            }));
        }
        around.map(|(low, high)| std::array::from_fn(|axis| (low[axis] + high[axis]) / 2.0))
    }

    /// The objects that can be picked: those not on a locked layer.
    fn selectable(&self) -> impl Iterator<Item = &(u64, Vec<Point>)> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.locked.get(*index).copied().unwrap_or(false))
            .map(|(_, object)| object)
    }

    /// Point objects with the colour to mark them in.
    #[must_use]
    pub fn point_markers(&self, selected: &[u64]) -> Vec<(Point, [f32; 3])> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(index, (id, points))| match points.as_slice() {
                [only] => {
                    let colour = if selected.contains(id) {
                        SELECTED_COLOUR
                    } else if self.locked.get(index).copied().unwrap_or(false) {
                        LOCKED_COLOUR
                    } else {
                        self.colours.get(index).copied().unwrap_or(CURVE_COLOUR)
                    };
                    Some((*only, colour))
                }
                _ => None,
            })
            .collect()
    }

    /// The lowest and highest corners around chosen objects, if any of them can be drawn.
    #[must_use]
    pub fn bounds_of(&self, ids: &[u64]) -> Option<(Point, Point)> {
        let mut around: Option<(Point, Point)> = None;
        for (index, (id, _)) in self.objects.iter().enumerate() {
            if !ids.contains(id) {
                continue;
            }
            let (low, high) = *self.boxes.get(index)?;
            around = Some(around.map_or((low, high), |(held_low, held_high)| {
                (
                    std::array::from_fn(|axis| held_low[axis].min(low[axis])),
                    std::array::from_fn(|axis| held_high[axis].max(high[axis])),
                )
            }));
        }
        around
    }

    /// The object nearest a screen position, if one is within `reach` pixels.
    #[must_use]
    pub fn pick(
        &self,
        camera: &Camera,
        screen: [f64; 2],
        size: [f64; 2],
        reach: f64,
    ) -> Option<u64> {
        let mut best: Option<(f64, u64)> = None;
        for (id, points) in self.selectable() {
            let projected: Vec<Option<[f64; 2]>> =
                points.iter().map(|p| camera.project(*p, size)).collect();
            if let [Some(only)] = projected.as_slice() {
                let distance = (only[0] - screen[0]).hypot(only[1] - screen[1]);
                if distance <= reach && best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, *id));
                }
            }
            for pair in projected.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    let distance = to_segment(screen, *a, *b);
                    if distance <= reach && best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, *id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// The edge of a surface or solid nearest a screen position, if one is within `reach`
    /// pixels, as the object and the edge's number.
    #[must_use]
    pub fn pick_edge(
        &self,
        camera: &Camera,
        screen: [f64; 2],
        size: [f64; 2],
        reach: f64,
    ) -> Option<(u64, usize)> {
        let mut best: Option<(f64, (u64, usize))> = None;
        for (index, (id, points)) in self.objects.iter().enumerate() {
            let Some(Some(edge)) = self.edge_of.get(index) else {
                continue;
            };
            if self.locked.get(index).copied().unwrap_or(false) {
                continue;
            }
            let projected: Vec<Option<[f64; 2]>> =
                points.iter().map(|p| camera.project(*p, size)).collect();
            for pair in projected.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    let distance = to_segment(screen, *a, *b);
                    if distance <= reach && best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, (*id, *edge)));
                    }
                }
            }
        }
        best.map(|(_, pick)| pick)
    }

    /// The edges of surfaces and solids a box takes: wholly inside for a window dragged
    /// rightwards, touched for a crossing box dragged leftwards, as for objects.
    #[must_use]
    pub fn edges_in_box(
        &self,
        camera: &Camera,
        from: [f64; 2],
        to: [f64; 2],
        size: [f64; 2],
    ) -> Vec<(u64, usize)> {
        let low = [from[0].min(to[0]), from[1].min(to[1])];
        let high = [from[0].max(to[0]), from[1].max(to[1])];
        let window = to[0] >= from[0];
        self.objects
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.locked.get(*index).copied().unwrap_or(false))
            .filter_map(|(index, (id, points))| {
                let edge = (*self.edge_of.get(index)?)?;
                let projected: Vec<Option<[f64; 2]>> =
                    points.iter().map(|p| camera.project(*p, size)).collect();
                let taken = if window {
                    projected
                        .iter()
                        .all(|p| p.is_some_and(|p| inside_box(p, low, high)))
                } else {
                    projected.windows(2).any(|pair| match pair {
                        [Some(a), Some(b)] => touches_box(*a, *b, low, high),
                        _ => false,
                    })
                };
                taken.then_some((*id, edge))
            })
            .collect()
    }

    /// The face of a surface or solid under a screen position, the nearest if several are, as
    /// the object and the face's number.
    #[must_use]
    pub fn pick_face(
        &self,
        camera: &Camera,
        screen: [f64; 2],
        size: [f64; 2],
    ) -> Option<(u64, usize)> {
        let forward = camera.forward();
        let mut best: Option<(f64, (u64, usize))> = None;
        for faces in self.faces.iter().filter(|faces| !faces.locked) {
            let mesh = &faces.mesh;
            for (triangle, face) in mesh.triangles.iter().zip(&mesh.triangle_faces) {
                let corners: Vec<Point> = triangle
                    .iter()
                    .filter_map(|&c| mesh.positions.get(c as usize).copied())
                    .collect();
                let on_screen: Vec<[f64; 2]> = corners
                    .iter()
                    .filter_map(|p| camera.project(*p, size))
                    .collect();
                let [a, b, c] = on_screen.as_slice() else {
                    continue;
                };
                if !inside_triangle(screen, *a, *b, *c) {
                    continue;
                }
                // How far along the view the triangle's middle is, to keep the nearest.
                let depth = corners
                    .iter()
                    .map(|p| p[0] * forward[0] + p[1] * forward[1] + p[2] * forward[2])
                    .sum::<f64>()
                    / 3.0;
                if best.is_none_or(|(d, _)| depth < d) {
                    best = Some((depth, (faces.id, *face)));
                }
            }
        }
        best.map(|(_, pick)| pick)
    }

    /// The faces of surfaces and solids a box takes: wholly inside for a window dragged
    /// rightwards, partly inside for a crossing box dragged leftwards.
    #[must_use]
    pub fn faces_in_box(
        &self,
        camera: &Camera,
        from: [f64; 2],
        to: [f64; 2],
        size: [f64; 2],
    ) -> Vec<(u64, usize)> {
        let low = [from[0].min(to[0]), from[1].min(to[1])];
        let high = [from[0].max(to[0]), from[1].max(to[1])];
        let window = to[0] >= from[0];
        let mut taken = Vec::new();
        for faces in self.faces.iter().filter(|faces| !faces.locked) {
            let mesh = &faces.mesh;
            let mut verdicts: std::collections::BTreeMap<usize, (bool, bool)> =
                std::collections::BTreeMap::new();
            for (triangle, face) in mesh.triangles.iter().zip(&mesh.triangle_faces) {
                for &corner in triangle {
                    let inside = mesh
                        .positions
                        .get(corner as usize)
                        .and_then(|p| camera.project(*p, size))
                        .is_some_and(|p| inside_box(p, low, high));
                    let entry = verdicts.entry(*face).or_insert((true, false));
                    entry.0 &= inside;
                    entry.1 |= inside;
                }
            }
            for (face, (all, any)) in verdicts {
                if (window && all) || (!window && any) {
                    taken.push((faces.id, face));
                }
            }
        }
        taken
    }

    /// The triangles of a picked face, for drawing it highlighted.
    #[must_use]
    pub fn face_triangles(&self, pick: (u64, usize)) -> Vec<[Point; 3]> {
        self.faces
            .iter()
            .filter(|faces| faces.id == pick.0)
            .flat_map(|faces| {
                let mesh = &faces.mesh;
                mesh.triangles
                    .iter()
                    .zip(&mesh.triangle_faces)
                    .filter(move |(_, face)| **face == pick.1)
                    .filter_map(move |(triangle, _)| {
                        let corner = |i: usize| mesh.positions.get(triangle[i] as usize).copied();
                        Some([corner(0)?, corner(1)?, corner(2)?])
                    })
            })
            .collect()
    }

    /// The points along a picked edge, for drawing it highlighted.
    #[must_use]
    pub fn edge_points(&self, pick: (u64, usize)) -> Option<&[Point]> {
        self.objects
            .iter()
            .zip(&self.edge_of)
            .find(|((id, _), edge)| *id == pick.0 && **edge == Some(pick.1))
            .map(|((_, points), _)| points.as_slice())
    }

    /// The objects chosen by a box dragged on screen from `from` to `to`, as in Rhino.
    ///
    /// Dragged rightwards it is a window, taking objects wholly inside. Dragged leftwards it is
    /// a crossing box, also taking objects it touches. Parts behind the camera are off screen,
    /// so they stop an object counting as wholly inside and cannot be touched.
    #[must_use]
    pub fn in_box(
        &self,
        camera: &Camera,
        from: [f64; 2],
        to: [f64; 2],
        size: [f64; 2],
    ) -> Vec<u64> {
        let low = [from[0].min(to[0]), from[1].min(to[1])];
        let high = [from[0].max(to[0]), from[1].max(to[1])];
        let window = to[0] >= from[0];
        let passes: Vec<(u64, bool)> = self
            .selectable()
            .map(|(id, points)| {
                let projected: Vec<Option<[f64; 2]>> =
                    points.iter().map(|p| camera.project(*p, size)).collect();
                let pass = if window {
                    !projected.is_empty()
                        && projected
                            .iter()
                            .all(|p| p.is_some_and(|p| inside_box(p, low, high)))
                } else {
                    matches!(projected.as_slice(), [Some(p)] if inside_box(*p, low, high))
                        || projected.windows(2).any(|pair| match pair {
                            [Some(a), Some(b)] => touches_box(*a, *b, low, high),
                            _ => false,
                        })
                };
                (*id, pass)
            })
            .collect();
        // An object drawn in several pieces, such as a body's edges, is taken by a window only
        // when every piece is inside, and by a crossing box when any piece is touched.
        let mut chosen: Vec<u64> = Vec::new();
        for (id, _) in &passes {
            if chosen.contains(id) {
                continue;
            }
            let mut pieces = passes.iter().filter(|(other, _)| other == id);
            let taken = if window {
                pieces.all(|(_, pass)| *pass)
            } else {
                pieces.any(|(_, pass)| *pass)
            };
            if taken {
                chosen.push(*id);
            }
        }
        chosen
    }
}

/// Whether `p` is inside the triangle `a`, `b`, `c` on screen, whichever way round it runs.
fn inside_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let side =
        |u: [f64; 2], v: [f64; 2]| (v[0] - u[0]) * (p[1] - u[1]) - (v[1] - u[1]) * (p[0] - u[0]);
    let (ab, bc, ca) = (side(a, b), side(b, c), side(c, a));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}

fn inside_box(p: [f64; 2], low: [f64; 2], high: [f64; 2]) -> bool {
    (0..2).all(|axis| low[axis] <= p[axis] && p[axis] <= high[axis])
}

/// Whether a segment reaches a box, by clipping it to the box one axis at a time
/// (the Liang-Barsky method).
fn touches_box(a: [f64; 2], b: [f64; 2], low: [f64; 2], high: [f64; 2]) -> bool {
    let (mut start, mut end) = (0.0_f64, 1.0_f64);
    for axis in 0..2 {
        let step = b[axis] - a[axis];
        if step.abs() < 1e-12 {
            if a[axis] < low[axis] || a[axis] > high[axis] {
                return false;
            }
            continue;
        }
        let (t0, t1) = ((low[axis] - a[axis]) / step, (high[axis] - a[axis]) / step);
        start = start.max(t0.min(t1));
        end = end.min(t0.max(t1));
        if start > end {
            return false;
        }
    }
    true
}

fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length < 1e-12 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn segment(out: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], colour: [f32; 3]) {
    out.push(Vertex {
        position: a,
        colour,
    });
    out.push(Vertex {
        position: b,
        colour,
    });
}

fn narrow(p: Point) -> [f32; 3] {
    p.map(|value| value as f32)
}

fn polyline(out: &mut Vec<Vertex>, points: &[Point], colour: [f32; 3]) {
    for pair in points.windows(2) {
        segment(out, narrow(pair[0]), narrow(pair[1]), colour);
    }
}

/// The grid and axes of a plane as line segments.
#[must_use]
pub fn grid_lines(plane: &CPlane) -> Vec<Vertex> {
    let on_plane = |u: f64, v: f64| {
        let along = |axis: usize| plane.origin[axis] + plane.x[axis] * u + plane.y[axis] * v;
        narrow([along(0), along(1), along(2)])
    };
    let mut out = Vec::new();
    let reach = f64::from(GRID_HALF_SIZE);
    for step in (-GRID_HALF_SIZE..=GRID_HALF_SIZE).step_by(GRID_STEP as usize) {
        let at = f64::from(step);
        if step != 0 {
            segment(
                &mut out,
                on_plane(at, -reach),
                on_plane(at, reach),
                GRID_COLOUR,
            );
            segment(
                &mut out,
                on_plane(-reach, at),
                on_plane(reach, at),
                GRID_COLOUR,
            );
        }
    }
    segment(
        &mut out,
        on_plane(-reach, 0.0),
        on_plane(reach, 0.0),
        X_AXIS_COLOUR,
    );
    segment(
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
    let mut out = grid_lines(&CPlane::WORLD_XY);
    out.extend(object_lines(outlines, selected));
    out
}

/// The grid, axes and objects as line segments for a shaded view.
#[must_use]
pub fn shaded_scene_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    let mut out = grid_lines(&CPlane::WORLD_XY);
    out.extend(shaded_object_lines(outlines, selected));
    out
}

/// The objects as line segments.
#[must_use]
pub fn object_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    lines_of(outlines, selected, false)
}

/// The objects as line segments for a shaded view, where the edges of surfaces and solids
/// are dark so they show against the faces, as in Rhino. Curves keep their layer's colour.
#[must_use]
pub fn shaded_object_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    lines_of(outlines, selected, true)
}

fn lines_of(outlines: &Outlines, selected: &[u64], shaded: bool) -> Vec<Vertex> {
    let mut out = Vec::new();
    for (index, (id, points)) in outlines.objects.iter().enumerate() {
        let on_faces = outlines.on_faces.get(index).copied().unwrap_or(false);
        let colour = if selected.contains(id) {
            SELECTED_COLOUR
        } else if outlines.locked.get(index).copied().unwrap_or(false) {
            LOCKED_COLOUR
        } else if shaded && on_faces {
            EDGE_ON_FACES_COLOUR
        } else {
            outlines.colours.get(index).copied().unwrap_or(CURVE_COLOUR)
        };
        polyline(&mut out, points, colour);
    }
    out
}

/// The surfaces and solids as shaded triangles, selected ones in the selection colour.
#[must_use]
pub fn face_vertices(outlines: &Outlines, selected: &[u64]) -> Vec<MeshVertex> {
    let mut out = Vec::new();
    for faces in &outlines.faces {
        let colour = if selected.contains(&faces.id) {
            SELECTED_COLOUR
        } else if faces.locked {
            LOCKED_COLOUR
        } else {
            faces.colour
        };
        let mesh = &faces.mesh;
        for triangle in &mesh.triangles {
            for &corner in triangle {
                let index = corner as usize;
                let (Some(position), Some(normal)) =
                    (mesh.positions.get(index), mesh.normals.get(index))
                else {
                    continue;
                };
                out.push(MeshVertex {
                    position: narrow(*position),
                    normal: narrow(*normal),
                    colour,
                });
            }
        }
    }
    out
}

/// How a command's preview is drawn. Every preview in Curva comes through here, so that they
/// all look the same.
#[must_use]
pub fn ghost_lines(curves: &[Curve]) -> Vec<Vertex> {
    curves
        .iter()
        .filter_map(|curve| curve.sample().ok())
        .flat_map(|points| preview_lines(&points, None))
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

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::Document;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn scene() -> (Outlines, u64, u64) {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::circle([0.0, 40.0, 0.0], 5.0).unwrap());
        (Outlines::of(doc.state()), a, b)
    }

    #[test]
    fn clicking_on_a_curve_picks_it() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let on_line = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, on_line, SIZE, 6.0), Some(line));
    }

    #[test]
    fn a_pick_finds_the_exact_point_on_the_curve() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let aim = [7.0, 0.0, 0.0];
        let at = camera.project(aim, SIZE).unwrap();
        let picked = outlines.pick_point(&camera, at, SIZE, 6.0).unwrap();
        assert!(
            picked[1].abs() < 1e-9 && (picked[0] - 7.0).abs() < 1e-3,
            "{picked:?}"
        );
    }

    #[test]
    fn a_pick_over_nothing_finds_nothing() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick_point(&camera, at, SIZE, 6.0), None);
    }

    #[test]
    fn hidden_layers_are_not_drawn_or_picked() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Hidden".to_owned());
        let line = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        doc.set_layer_visible(layer, false);
        let outlines = Outlines::of(doc.state());
        assert!(outlines.objects.is_empty());
    }

    #[test]
    fn locked_layers_are_drawn_grey_and_cannot_be_picked() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Locked".to_owned());
        let line = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        doc.set_layer_locked(layer, true);
        let outlines = Outlines::of(doc.state());
        let camera = Camera::default();
        let at = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), None);
        assert_eq!(outlines.pick_point(&camera, at, SIZE, 6.0), None);
        assert!(
            object_lines(&outlines, &[])
                .iter()
                .all(|v| v.colour == LOCKED_COLOUR)
        );
    }

    #[test]
    fn solids_give_faces_and_dark_edges_only_in_shaded_views() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.add(Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        // Twelve box edges and the line, each its own entry.
        assert_eq!(outlines.objects.len(), 13);
        let faces = face_vertices(&outlines, &[]);
        assert!(faces.len() >= 36 && faces.len().is_multiple_of(3));
        assert!(faces.iter().all(|v| v.colour == CURVE_COLOUR));
        assert!(
            object_lines(&outlines, &[])
                .iter()
                .all(|v| v.colour == CURVE_COLOUR)
        );
        let shaded = shaded_object_lines(&outlines, &[]);
        let dark = shaded
            .iter()
            .filter(|v| v.colour == EDGE_ON_FACES_COLOUR)
            .count();
        assert!(dark >= 24, "every piece of the box's twelve edges is dark");
        assert_eq!(shaded.len() - dark, 2, "the line keeps its colour");
        let picked = face_vertices(&outlines, &[solid]);
        assert!(picked.iter().all(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn edges_are_picked_by_click_and_by_box() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        doc.add(Curve::line([10.0, 0.0, 0.0], [12.0, 0.0, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = |p: Point| camera.project(p, SIZE).unwrap();
        // A click on the box's edge along X at y = 0 picks a box edge; one on the line, nothing.
        let picked = outlines.pick_edge(&camera, at([2.0, 0.0, 0.0]), SIZE, 6.0);
        assert!(picked.is_some_and(|(id, _)| id == solid));
        let points = outlines.edge_points(picked.unwrap()).unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
        assert_eq!(
            outlines.pick_edge(&camera, at([11.0, 0.0, 0.0]), SIZE, 6.0),
            None
        );
        // A window round the whole box, seen from above, takes all twelve edges.
        let (a, b) = (at([-1.0, -1.0, 0.0]), at([5.0, 5.0, 0.0]));
        let all = outlines.edges_in_box(&camera, [a[0], b[1]], [b[0], a[1]], SIZE);
        assert_eq!(all.len(), 12);
        assert!(all.iter().all(|(id, _)| *id == solid));
    }

    #[test]
    fn the_nearest_face_under_the_cursor_is_picked() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = |p: Point| camera.project(p, SIZE).unwrap();
        // Seen from above, the middle of the box shows its top face, not its bottom.
        let (id, face) = outlines
            .pick_face(&camera, at([2.0, 2.0, 4.0]), SIZE)
            .unwrap();
        assert_eq!(id, solid);
        let triangles = outlines.face_triangles((id, face));
        assert!(!triangles.is_empty());
        assert!(
            triangles
                .iter()
                .flatten()
                .all(|p| (p[2] - 4.0).abs() < 1e-9)
        );
        assert_eq!(
            outlines.pick_face(&camera, at([20.0, 20.0, 0.0]), SIZE),
            None
        );
        // A window round the whole box takes all six faces.
        let (a, b) = (at([-1.0, -1.0, 0.0]), at([5.0, 5.0, 0.0]));
        assert_eq!(
            outlines
                .faces_in_box(&camera, [a[0], b[1]], [b[0], a[1]], SIZE)
                .len(),
            6
        );
    }

    #[test]
    fn a_window_takes_a_solid_only_with_all_its_edges() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let corner = |p: Point| camera.project(p, SIZE).unwrap();
        let (a, b) = (corner([-1.0, -1.0, 0.0]), corner([5.0, 5.0, 0.0]));
        let window = |from: [f64; 2], to: [f64; 2]| outlines.in_box(&camera, from, to, SIZE);
        assert_eq!(window([a[0], b[1]], [b[0], a[1]]), [solid]);
        // A window over half the box takes nothing; a crossing box over it takes the solid.
        let half = corner([2.0, 5.0, 0.0]);
        assert!(window([a[0], half[1]], [half[0], a[1]]).is_empty());
        assert_eq!(window([half[0], a[1]], [a[0], half[1]]), [solid]);
    }

    #[test]
    fn objects_take_the_colour_of_their_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(layer, [1.0, 0.0, 0.0]);
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        let lines = object_lines(&Outlines::of(doc.state()), &[]);
        assert!(lines.iter().all(|v| v.colour == [1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_selected_object_is_highlighted_whatever_its_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(layer, [1.0, 0.0, 0.0]);
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        let lines = object_lines(&Outlines::of(doc.state()), &[line]);
        assert!(lines.iter().all(|v| v.colour == SELECTED_COLOUR));
    }

    fn one_point() -> (Outlines, u64) {
        let mut doc = Document::default();
        let id = doc.add(Curve::Point {
            at: [5.0, 5.0, 0.0],
        });
        (Outlines::of(doc.state()), id)
    }

    #[test]
    fn a_point_can_be_clicked() {
        let (outlines, id) = one_point();
        let camera = Camera::default();
        let at = camera.project([5.0, 5.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), Some(id));
    }

    #[test]
    fn a_point_is_caught_by_a_box_either_way() {
        let (outlines, id) = one_point();
        let camera = Camera::default();
        let at = camera.project([5.0, 5.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert_eq!(outlines.in_box(&camera, low, high, SIZE), [id]);
        assert_eq!(outlines.in_box(&camera, high, low, SIZE), [id]);
    }

    #[test]
    fn a_box_elsewhere_misses_a_point() {
        let (outlines, _) = one_point();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert!(outlines.in_box(&camera, high, low, SIZE).is_empty());
    }

    #[test]
    fn a_selected_point_is_marked_as_selected() {
        let (outlines, id) = one_point();
        assert_eq!(outlines.point_markers(&[id])[0].1, SELECTED_COLOUR);
        assert_eq!(outlines.point_markers(&[])[0].1, CURVE_COLOUR);
    }
    #[test]
    fn clicking_on_nothing_picks_nothing() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let empty = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, empty, SIZE, 6.0), None);
    }

    /// Screen corners a margin outside two model points, left corner first.
    fn around(camera: &Camera, a: Point, b: Point, margin: f64) -> ([f64; 2], [f64; 2]) {
        let (a, b) = (
            camera.project(a, SIZE).unwrap(),
            camera.project(b, SIZE).unwrap(),
        );
        (
            [a[0].min(b[0]) - margin, a[1].min(b[1]) - margin],
            [a[0].max(b[0]) + margin, a[1].max(b[1]) + margin],
        )
    }

    #[test]
    fn a_window_takes_an_object_wholly_inside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [20.0, 0.0, 0.0], 3.0);
        assert!(outlines.in_box(&camera, left, right, SIZE).contains(&line));
    }

    #[test]
    fn a_window_leaves_an_object_partly_outside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [0.0, 0.0, 0.0], 3.0);
        assert!(!outlines.in_box(&camera, left, right, SIZE).contains(&line));
    }

    #[test]
    fn the_same_box_dragged_leftwards_takes_the_partly_inside_object() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [0.0, 0.0, 0.0], 3.0);
        assert!(outlines.in_box(&camera, right, left, SIZE).contains(&line));
    }

    #[test]
    fn a_crossing_box_catches_a_segment_with_no_sample_inside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let middle = camera.project([0.0, 0.0, 0.0], SIZE).unwrap();
        let (low, high) = (
            [middle[0] - 2.0, middle[1] - 2.0],
            [middle[0] + 2.0, middle[1] + 2.0],
        );
        assert_eq!(outlines.in_box(&camera, high, low, SIZE), [line]);
    }

    #[test]
    fn a_box_over_empty_space_takes_nothing_in_either_direction() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert!(outlines.in_box(&camera, low, high, SIZE).is_empty());
        assert!(outlines.in_box(&camera, high, low, SIZE).is_empty());
    }

    #[test]
    fn selected_objects_change_colour() {
        let (outlines, line, _) = scene();
        let plain = scene_lines(&outlines, &[]);
        let marked = scene_lines(&outlines, &[line]);
        assert_eq!(plain.len(), marked.len());
        assert!(marked.iter().any(|v| v.colour == SELECTED_COLOUR));
        assert!(!plain.iter().any(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn bounds_surround_everything() {
        let (outlines, _, _) = scene();
        let (low, high) = outlines.bounds().unwrap();
        assert!(low[0] <= -20.0 && high[0] >= 20.0 && high[1] >= 45.0 - 1e-6);
    }

    #[test]
    fn preview_runs_through_its_points_to_the_cursor() {
        let lines = preview_lines(&[[0.0; 3], [1.0, 0.0, 0.0]], Some([2.0, 0.0, 0.0]));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3].position, [2.0, 0.0, 0.0]);
        assert!(preview_lines(&[], Some([2.0, 0.0, 0.0])).is_empty());
    }
}
