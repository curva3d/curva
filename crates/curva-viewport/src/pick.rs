//! Finding objects under the cursor or inside a box, and the boxes, centres and colours of objects.

use curva_kernel::Point;

use crate::{
    CURVE_COLOUR, Camera, HIDING_ALLOWANCE, LOCKED_COLOUR, Outlines, PICK_SEARCH_STEPS,
    SELECTED_COLOUR,
};

/// What sort of object a pickable outline belongs to, for the selection filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    /// A point object.
    Point,
    /// A curve.
    Curve,
    /// An open surface.
    Surface,
    /// A closed solid.
    Solid,
}

/// Which kinds of object clicking and boxing may pick, as Rhino's selection filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PickFilter {
    /// Whether the filter is in force; when off, everything can be picked.
    pub on: bool,
    /// Point objects.
    pub points: bool,
    /// Curves.
    pub curves: bool,
    /// Open surfaces.
    pub surfaces: bool,
    /// Closed solids.
    pub solids: bool,
}

impl Default for PickFilter {
    fn default() -> Self {
        Self {
            on: true,
            points: true,
            curves: true,
            surfaces: true,
            solids: true,
        }
    }
}

impl PickFilter {
    /// Whether an object of this kind can be picked.
    #[must_use]
    pub fn allows(&self, kind: PickKind) -> bool {
        !self.on
            || match kind {
                PickKind::Point => self.points,
                PickKind::Curve => self.curves,
                PickKind::Surface => self.surfaces,
                PickKind::Solid => self.solids,
            }
    }
}

impl Outlines {
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

    /// Sets which kinds of object clicking and boxing may pick.
    pub fn set_filter(&mut self, filter: PickFilter) {
        self.filter = filter;
    }

    /// The objects that can be picked: those not on a locked layer, and of a kind the
    /// selection filter lets through.
    pub(crate) fn selectable(&self) -> impl Iterator<Item = &(u64, std::sync::Arc<[Point]>)> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(index, _)| self.is_selectable(*index))
            .map(|(_, object)| object)
    }

    /// Whether entry `index` of `objects` can be picked: not on a locked layer, and of a kind
    /// the selection filter lets through.
    fn is_selectable(&self, index: usize) -> bool {
        !self.locked.get(index).copied().unwrap_or(false)
            && self
                .kinds
                .get(index)
                .is_none_or(|kind| self.filter.allows(*kind))
    }

    /// Point objects with the colour to mark them in.
    #[must_use]
    pub fn point_markers(&self, selected: &[u64]) -> Vec<(Point, [f32; 3])> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(index, (id, points))| match &points[..] {
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

    /// The colour to show an object in, as its points are marked: the selection's, locked
    /// grey, or its own; `None` when it is not drawn here (hidden, or in another space).
    #[must_use]
    pub fn colour_of(&self, id: u64, selected: &[u64]) -> Option<[f32; 3]> {
        let index = self.objects.iter().position(|(held, _)| *held == id)?;
        Some(if selected.contains(&id) {
            SELECTED_COLOUR
        } else if self.locked.get(index).copied().unwrap_or(false) {
            LOCKED_COLOUR
        } else {
            self.colours.get(index).copied().unwrap_or(CURVE_COLOUR)
        })
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
        let view = camera.projector(size);
        // Only objects whose boxes come within reach of the cursor (#130).
        for index in self.objects_near(&view, (screen, reach)) {
            if !self.is_selectable(index) {
                continue;
            }
            let (id, points) = &self.objects[index];
            let projected: Vec<Option<[f64; 2]>> =
                points.iter().map(|p| view.project(*p)).collect();
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
        // Curves, points and edges win within their few pixels; otherwise a click anywhere on a
        // face takes its surface or solid, the nearest if several are under the pointer.
        best.map(|(_, id)| id).or_else(|| {
            let (id, _) = self.pick_face(camera, screen, size)?;
            self.can_pick(id).then_some(id)
        })
    }

    /// Whether the object `id` can be picked: not on a locked layer, and of a kind the
    /// selection filter lets through. An object drawn by its faces alone, such as a smooth SubD
    /// without creases, is judged by its faces (#324).
    pub(crate) fn can_pick(&self, id: u64) -> bool {
        self.objects.iter().enumerate().any(|(index, (other, _))| {
            *other == id
                && !self.locked.get(index).copied().unwrap_or(false)
                && self
                    .kinds
                    .get(index)
                    .is_none_or(|kind| self.filter.allows(*kind))
        }) || self
            .faces
            .iter()
            .any(|faces| faces.id == id && !faces.locked && self.filter.allows(faces.kind))
    }

    /// The surfaces and solids a crossing box touches through their faces alone: those with a
    /// face under the box, though no edge of them reaches it.
    pub(crate) fn crossed_faces(
        &self,
        camera: &Camera,
        low: [f64; 2],
        high: [f64; 2],
        size: [f64; 2],
    ) -> Vec<u64> {
        let corners = [low, [high[0], low[1]], high, [low[0], high[1]]];
        self.faces
            .iter()
            .filter(|faces| !faces.locked && self.can_pick(faces.id))
            .filter(|faces| {
                let mesh = &faces.mesh;
                mesh.triangles.iter().any(|triangle| {
                    let on_screen: Vec<[f64; 2]> = triangle
                        .iter()
                        .filter_map(|&c| faces.corner(c))
                        .filter_map(|p| camera.project(p, size))
                        .collect();
                    let [a, b, c] = on_screen.as_slice() else {
                        return false;
                    };
                    on_screen.iter().any(|p| inside_box(*p, low, high))
                        || corners.iter().any(|q| inside_triangle(*q, *a, *b, *c))
                })
            })
            .map(|faces| faces.id)
            .collect()
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
        let view = camera.projector(size);
        let mut best: Option<(f64, (u64, usize))> = None;
        // Only meshes whose boxes reach the cursor on screen (#130).
        let near = self.faces.iter().filter(|faces| {
            !faces.locked && !crate::snap::box_beyond(&view, faces.around, (screen, 0.0))
        });
        for faces in near {
            let mesh = &faces.mesh;
            for (triangle, face) in mesh.triangles.iter().zip(&mesh.triangle_faces) {
                let corner = |k: usize| faces.corner(triangle[k]);
                let (Some(p), Some(q), Some(r)) = (corner(0), corner(1), corner(2)) else {
                    continue;
                };
                let corners = [p, q, r];
                let (Some(a), Some(b), Some(c)) =
                    (view.project(p), view.project(q), view.project(r))
                else {
                    continue;
                };
                if !inside_triangle(screen, a, b, c) {
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
                    let inside = faces
                        .corner(corner)
                        .and_then(|p| camera.project(p, size))
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
                        let corner = |i: usize| faces.corner(triangle[i]);
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
            .map(|((_, points), _)| &points[..])
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
        // A crossing box also takes a surface or solid whose faces it touches.
        if !window {
            for id in self.crossed_faces(camera, low, high, size) {
                if !chosen.contains(&id) {
                    chosen.push(id);
                }
            }
        }
        chosen
    }
}

/// Whether `p` is inside the triangle `a`, `b`, `c` on screen, whichever way round it runs.
pub(crate) fn inside_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let side =
        |u: [f64; 2], v: [f64; 2]| (v[0] - u[0]) * (p[1] - u[1]) - (v[1] - u[1]) * (p[0] - u[0]);
    let (ab, bc, ca) = (side(a, b), side(b, c), side(c, a));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}

pub(crate) fn inside_box(p: [f64; 2], low: [f64; 2], high: [f64; 2]) -> bool {
    (0..2).all(|axis| low[axis] <= p[axis] && p[axis] <= high[axis])
}

/// Whether a segment reaches a box, by clipping it to the box one axis at a time
/// (the Liang-Barsky method).
pub(crate) fn touches_box(a: [f64; 2], b: [f64; 2], low: [f64; 2], high: [f64; 2]) -> bool {
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

pub(crate) fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length < 1e-12 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

/// The lowest and highest corners of a mesh.
pub(crate) fn mesh_box(mesh: &curva_kernel::BodyMesh) -> (Point, Point) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in &mesh.positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    (low, high)
}

/// How far along the line from `origin` in `direction` it enters the box, if it meets it.
pub(crate) fn enters_box(
    origin: Point,
    direction: Point,
    (low, high): (Point, Point),
) -> Option<f64> {
    let (mut near, mut far) = (f64::NEG_INFINITY, f64::INFINITY);
    for axis in 0..3 {
        if direction[axis].abs() < 1e-15 {
            if origin[axis] < low[axis] || origin[axis] > high[axis] {
                return None;
            }
            continue;
        }
        let a = (low[axis] - origin[axis]) / direction[axis];
        let b = (high[axis] - origin[axis]) / direction[axis];
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && far >= 0.0).then_some(near)
}

/// How far along the line from `origin` in `direction` it crosses the triangle, if it does
/// (Möller and Trumbore's method).
pub(crate) fn crosses_triangle(
    origin: Point,
    direction: Point,
    [a, b, c]: [Point; 3],
) -> Option<f64> {
    let sub = |p: Point, q: Point| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let dot = |p: Point, q: Point| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let cross = |p: Point, q: Point| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let (ab, ac) = (sub(b, a), sub(c, a));
    let side = cross(direction, ac);
    let det = dot(ab, side);
    if det.abs() < 1e-15 {
        return None;
    }
    let to = sub(origin, a);
    let u = dot(to, side) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let up = cross(to, ab);
    let v = dot(direction, up) / det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(dot(ac, up) / det)
}

impl Outlines {
    /// Whether a face lies between the camera and `point`, so it cannot be seen in a shaded
    /// view. A face the point lies on does not hide it: the test allows a small part of the
    /// body's size for the mesh standing off the true surface.
    #[must_use]
    pub fn hidden(&self, camera: &Camera, size: [f64; 2], point: Point) -> bool {
        let Some(screen) = camera.project(point, size) else {
            return false;
        };
        let (origin, direction) = camera.ray(screen, size);
        let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length < 1e-15 {
            return false;
        }
        let direction = direction.map(|v| v / length);
        let reach: f64 = (0..3).map(|i| (point[i] - origin[i]) * direction[i]).sum();
        self.faces.iter().any(|faces| {
            let (low, high) = faces.around;
            let span = (0..3)
                .map(|i| (high[i] - low[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            let allowance = (span * HIDING_ALLOWANCE).max(1e-6);
            if enters_box(origin, direction, faces.around).is_none_or(|t| t > reach - allowance) {
                return false;
            }
            let mesh = &faces.mesh;
            mesh.triangles.iter().any(|triangle| {
                let corner = |i: usize| faces.corner(triangle[i]);
                let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
                    return false;
                };
                crosses_triangle(origin, direction, [a, b, c])
                    .is_some_and(|t| t > 0.0 && t < reach - allowance)
            })
        })
    }
}
