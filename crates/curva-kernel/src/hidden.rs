//! Hidden-line removal for Make2D and printed technical views (spec `make2d.md` §2): the lines
//! of bodies and curves seen from a view, each visible or hidden, as clean curves.
//!
//! 1. **Lines to draw**, exact: bodies' edges as the engine holds them (seams left out, edges
//!    between smoothly joined faces only when asked for), the silhouettes of curved faces
//!    (cylinders and spheres worked out exactly and trimmed to their faces by the engine's
//!    point classification; other curved faces from their tessellation), and the chosen
//!    curves, polylines taken apart into their segments.
//! 2. **Projection** into the view plane: lines stay lines; in a parallel view every curve is
//!    mapped exactly (its control points); in perspective, curves are followed point by point.
//! 3. **Splitting** where the projected lines cross: lines exactly, other curves by Curva's
//!    crossing search.
//! 4. **Visibility** of each piece at its middle, by solidcore's B-rep ray firing towards the eye
//!    ([`crate::rays`]), faces' trims respected.
//! 5. **Cleaning**: pieces with the same visibility join back into one curve (a line stays one
//!    line), lines seen end-on are dropped, and so are hidden lines lying under visible ones and
//!    lines drawn twice. A square comes out as four lines; a box from above as its four outline
//!    lines.

use crate::rays::RayScene;
use crate::{Body, Curve, KernelError, Point};

mod silhouettes;
use silhouettes::{analytic_silhouettes, mesh_silhouettes, trimmed};

/// Where a view looks from (spec `make2d.md` §1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The eye: for a parallel view, any point behind everything along `forward`.
    pub eye: Point,
    /// The direction it looks, of length 1.
    pub forward: Point,
    /// Its right, of length 1, across `forward`.
    pub right: Point,
    /// Its up, of length 1, across both.
    pub up: Point,
    /// Whether the view is flat, with no perspective.
    pub parallel: bool,
}

/// What kind of line a drawn line is, for Make2D's layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// A curve, or an edge of a surface or solid.
    Curve,
    /// The outline of a curved face against what is behind it.
    Silhouette,
    /// An edge between two faces that join smoothly.
    Tangent,
}

/// What to draw besides edges, curves and silhouettes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Edges between faces that join smoothly (Rhino's Tangent edges).
    pub tangent_edges: bool,
    /// Inputs from this one on (bodies first, then curves) win where their lines lie on
    /// others': a line drawn twice keeps theirs, as clipping-plane cuts keep their own over the
    /// cut edges they lie on. `None` keeps the first drawn.
    pub first_from: Option<usize>,
}

/// A line of the drawing, in the view plane: a curve lying at z = 0, its x right and its y up
/// from the view's centre line, in model units (a perspective view is scaled to the size of the
/// scene at its middle).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLine {
    /// The curve in the view plane.
    pub curve: Curve,
    /// Whether it shows, rather than being hidden behind a face.
    pub visible: bool,
    /// What it is.
    pub kind: LineKind,
    /// Which input it comes from: bodies first, then curves, by their order.
    pub source: usize,
}

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: Point) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: Point) -> Option<Point> {
    let l = length(a);
    (l > 1e-12).then(|| a.map(|c| c / l))
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

impl View {
    /// A point in the view plane (z = 0), or `None` behind a perspective eye. `focus` is the
    /// distance at which a perspective view keeps model size.
    fn project(&self, p: Point, focus: f64) -> Option<Point> {
        let d = sub(p, self.eye);
        let (x, y) = (dot(d, self.right), dot(d, self.up));
        if self.parallel {
            return Some([x, y, 0.0]);
        }
        let depth = dot(d, self.forward);
        (depth > 1e-9).then(|| [x * focus / depth, y * focus / depth, 0.0])
    }

    /// The affine map of a parallel view, rows as `Curve::mapped_affine` takes them.
    fn flattening(&self) -> [[f64; 4]; 4] {
        [
            [self.right[0], self.up[0], 0.0, 0.0],
            [self.right[1], self.up[1], 0.0, 0.0],
            [self.right[2], self.up[2], 0.0, 0.0],
            [
                -dot(self.eye, self.right),
                -dot(self.eye, self.up),
                0.0,
                1.0,
            ],
        ]
    }

    /// The way to the eye from `p`, of length 1, and how far it goes.
    fn way_to_eye(&self, p: Point) -> (Point, f64) {
        if self.parallel {
            return (self.forward.map(|c| -c), f64::INFINITY);
        }
        let d = sub(self.eye, p);
        let l = length(d).max(1e-12);
        (d.map(|c| c / l), l)
    }

    /// The direction from the eye to `p`, of length 1.
    fn sight(&self, p: Point) -> Point {
        if self.parallel {
            self.forward
        } else {
            unit(sub(p, self.eye)).unwrap_or(self.forward)
        }
    }
}

/// A line to draw, exact in the model, with its projection.
struct Source {
    /// The curve in the model.
    model: Curve,
    /// Its projection at z = 0.
    flat: Curve,
    /// Whether it is a straight line.
    straight: bool,
    kind: LineKind,
    source: usize,
    /// For a silhouette taken from the tessellation, the faces it lies near: its points are
    /// moved onto the nearest of their exact surfaces before their visibility is tested.
    near_faces: Vec<usize>,
}

/// A curve taken apart into the pieces Make2D draws one by one: a polyline's segments, a joined
/// curve's parts.
fn pieces_of(curve: &Curve) -> Vec<Curve> {
    match curve {
        Curve::Polyline { points, closed } => {
            let mut corners = points.clone();
            if *closed && let Some(first) = points.first() {
                corners.push(*first);
            }
            corners
                .windows(2)
                .filter_map(|w| Curve::line(w[0], w[1]).ok())
                .collect()
        }
        Curve::Compound { segments } => segments.iter().flat_map(pieces_of).collect(),
        Curve::Point { .. } => Vec::new(),
        other => vec![other.clone()],
    }
}

/// The ends of a line.
fn ends(curve: &Curve) -> Option<(Point, Point)> {
    match curve {
        Curve::Line { start, end } => Some((*start, *end)),
        _ => None,
    }
}

/// The projection of a model curve: an exact line or mapped curve, or in perspective a
/// polyline through its points.
fn flatten(curve: &Curve, view: &View, focus: f64) -> Option<Curve> {
    if let Some((a, b)) = ends(curve) {
        let (a, b) = (view.project(a, focus)?, view.project(b, focus)?);
        return Curve::line(a, b).ok();
    }
    if view.parallel {
        return curve.mapped_affine(view.flattening()).ok();
    }
    let points: Option<Vec<Point>> = curve
        .sample()
        .ok()?
        .into_iter()
        .map(|p| view.project(p, focus))
        .collect();
    Curve::polyline(points?, false).ok()
}

/// The size of a curve's projection, to drop lines seen end-on.
fn extent(curve: &Curve) -> f64 {
    let Ok(points) = curve.sample() else {
        return 0.0;
    };
    let Some(&first) = points.first() else {
        return 0.0;
    };
    points
        .iter()
        .map(|p| length(sub(*p, first)))
        .fold(0.0, f64::max)
}

fn bounds2(curve: &Curve) -> ([f64; 2], [f64; 2]) {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for p in curve.sample().unwrap_or_default() {
        for i in 0..2 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    (low, high)
}

/// Where a flat line `a` crosses a flat line `b`, strictly inside `a` and anywhere on `b`
/// including its ends (a line passing behind another's end changes visibility there).
fn line_crossing(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let r = sub(a.1, a.0);
    let s = sub(b.1, b.0);
    let denom = r[0] * s[1] - r[1] * s[0];
    let scale = length(r) * length(s);
    if denom.abs() <= 1e-12 * scale {
        return None;
    }
    let w = sub(b.0, a.0);
    let t = (w[0] * s[1] - w[1] * s[0]) / denom;
    let u = (w[0] * r[1] - w[1] * r[0]) / denom;
    let inside = |v: f64| v > 1e-9 && v < 1.0 - 1e-9;
    let touching = |v: f64| (-1e-9..=1.0 + 1e-9).contains(&v);
    (inside(t) && touching(u)).then(|| plus(a.0, r, t))
}

/// The model point a point of a flat piece stands for: the point of `model` whose projection is
/// nearest `flat_point`.
fn model_point(model: &Curve, flat_point: Point, view: &View, focus: f64) -> Option<Point> {
    let samples: Vec<Point> = if let Some((a, b)) = ends(model) {
        (0..=64)
            .map(|k| plus(a, sub(b, a), f64::from(k) / 64.0))
            .collect()
    } else {
        model.sample().ok()?
    };
    let mut best: Option<(f64, Point)> = None;
    for pair in samples.windows(2) {
        let (Some(pa), Some(pb)) = (view.project(pair[0], focus), view.project(pair[1], focus))
        else {
            continue;
        };
        let along = sub(pb, pa);
        let span = dot(along, along);
        let t = if span > 0.0 {
            (dot(sub(flat_point, pa), along) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let off = length(sub(plus(pa, along, t), flat_point));
        if best.is_none_or(|(b, _)| off < b) {
            best = Some((off, plus(pair[0], sub(pair[1], pair[0]), t)));
        }
    }
    best.map(|(_, p)| p)
}

/// The middle of a curve.
fn middle(curve: &Curve) -> Option<Point> {
    if let Some((a, b)) = ends(curve) {
        return Some(plus(a, sub(b, a), 0.5));
    }
    let points = curve.sample().ok()?;
    points.get(points.len() / 2).copied()
}

/// Whether five points along `piece` all lie within `tolerance` of `other`.
fn lies_on(piece: &Curve, other: &Curve, tolerance: f64) -> bool {
    let picks: Vec<Point> = if let Some((a, b)) = ends(piece) {
        (0..=4)
            .map(|k| plus(a, sub(b, a), f64::from(k) / 4.0))
            .collect()
    } else {
        let Ok(points) = piece.sample() else {
            return false;
        };
        if points.len() < 5 {
            points
        } else {
            (0..5).map(|k| points[k * (points.len() - 1) / 4]).collect()
        }
    };
    picks.iter().all(|p| {
        other
            .closest_point(*p)
            .is_ok_and(|q| length(sub(q, *p)) <= tolerance)
    })
}

/// How big a scene is, with the tolerances and the perspective scale that follow from that.
struct Scale {
    size: f64,
    tolerance: f64,
    /// Lines shorter than this once projected count as points.
    flat: f64,
    /// The distance at which a perspective view keeps model size.
    focus: f64,
}

impl Scale {
    /// How far a silhouette's ray goes before it may be blocked. Its ray runs tangent to its
    /// surface, which falls away only as the square of the distance: it stops where the gap is
    /// well past the engine's zone, so grazing its own surface is never taken for a hit.
    fn grazing(&self) -> f64 {
        (2.0 * self.size * self.tolerance * 10.0).sqrt() * 4.0
    }
}

/// A line to draw, exact in the model: the curve, its kind, the input it comes from, and for a
/// silhouette taken from the tessellation the faces it outlines.
type ModelLine = (Curve, LineKind, usize, Vec<usize>);

/// The lines of `bodies` and `curves` seen from `view`, visible and hidden, as clean curves in
/// the view plane.
///
/// # Errors
///
/// When a body cannot be read or a curve sampled.
// CUSTOM KERNEL OPERATION: hidden-lines
pub fn hidden_lines(
    bodies: &[Body],
    curves: &[Curve],
    view: &View,
    options: Options,
) -> Result<Vec<DrawnLine>, KernelError> {
    let Some(scale) = scene_scale(bodies, curves, view)? else {
        return Ok(Vec::new());
    };

    // 1. The lines to draw, exact in the model.
    let mut models: Vec<ModelLine> = Vec::new();
    for (index, body) in bodies.iter().enumerate() {
        let edges = body.exact_edges()?;
        edge_lines(body, index, &edges, options, &mut models);
        silhouette_lines(body, index, &edges, view, &scale, &mut models)?;
    }
    for (index, curve) in curves.iter().enumerate() {
        for piece in pieces_of(curve) {
            models.push((piece, LineKind::Curve, bodies.len() + index, Vec::new()));
        }
    }

    // 2. Projected; lines seen end-on are dropped.
    let sources = projected(models, view, &scale);

    // 3. Where they cross.
    let cuts = crossing_points(&sources, scale.flat)?;

    // 4. Each piece's visibility; 5. runs of the same visibility joined.
    let mut scene = RayScene::new(bodies)?;
    scene.set_tolerance(scale.tolerance * 10.0);
    let mut drawn: Vec<DrawnLine> = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let runs = visibility_runs(source, &cuts[index], bodies, &mut scene, view, &scale)?;
        drawn_lines(source, runs, &mut drawn);
    }
    Ok(without_overlaps(drawn, options, scale.flat))
}

/// The scene's size and middle, for tolerances and a perspective view's scale; `None` when it
/// is empty.
fn scene_scale(
    bodies: &[Body],
    curves: &[Curve],
    view: &View,
) -> Result<Option<Scale>, KernelError> {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    let mut grow = |p: Point| {
        for i in 0..3 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    };
    for body in bodies {
        let (lo, hi) = body.bounds()?;
        grow(lo);
        grow(hi);
    }
    for curve in curves {
        for p in curve.sample()? {
            grow(p);
        }
    }
    if !low[0].is_finite() {
        return Ok(None);
    }
    let size = length(sub(high, low)).max(1e-9);
    let tolerance = (size * 1e-7).max(1e-9);
    let centre = plus(low, sub(high, low), 0.5);
    let focus = dot(sub(centre, view.eye), view.forward).max(1e-9);
    Ok(Some(Scale {
        size,
        tolerance,
        flat: tolerance * 10.0,
        focus,
    }))
}

/// The edges of body `index` as lines to draw: seams (the same face on both sides) left out,
/// and edges between faces that join smoothly only when `options` asks for them.
fn edge_lines(
    body: &Body,
    index: usize,
    edges: &[(Curve, Vec<usize>)],
    options: Options,
    models: &mut Vec<ModelLine>,
) {
    for (curve, faces) in edges {
        let kind = match faces[..] {
            // A seam: the same face on both sides.
            [a, b] if a == b => continue,
            [a, b] => {
                let at = middle(curve).unwrap_or_default();
                let smooth = match (body.face_normal_near(a, at), body.face_normal_near(b, at)) {
                    (Ok(na), Ok(nb)) => length(cross(na, nb)) < 1e-6,
                    _ => false,
                };
                if smooth {
                    if !options.tangent_edges {
                        continue;
                    }
                    LineKind::Tangent
                } else {
                    LineKind::Curve
                }
            }
            _ => LineKind::Curve,
        };
        models.push((curve.clone(), kind, index, Vec::new()));
    }
}

/// The silhouettes of body `index` as lines to draw: exact for cylinders and spheres, from the
/// tessellation for other curved faces; planes have none.
fn silhouette_lines(
    body: &Body,
    index: usize,
    edges: &[(Curve, Vec<usize>)],
    view: &View,
    scale: &Scale,
    models: &mut Vec<ModelLine>,
) -> Result<(), KernelError> {
    // The edges silhouettes end on: seams (the same face on both sides) left out.
    let edge_curves: Vec<Curve> = edges
        .iter()
        .filter(|(_, faces)| !matches!(faces[..], [a, b] if a == b))
        .map(|(c, _)| c.clone())
        .collect();
    let mut meshed_faces = Vec::new();
    for (face, surface) in body.face_surfaces()?.iter().enumerate() {
        use crate::record::Surface;
        if matches!(
            surface,
            Surface::Plane { .. } | Surface::PlaneThrough { .. }
        ) {
            continue;
        }
        match analytic_silhouettes(surface, view, scale.size * 2.0) {
            Some(candidates) => {
                for candidate in candidates {
                    for piece in trimmed(&candidate, body, &edge_curves)? {
                        models.push((piece, LineKind::Silhouette, index, Vec::new()));
                    }
                }
            }
            None => meshed_faces.push(face),
        }
    }
    if !meshed_faces.is_empty() {
        let mesh = body.mesh()?;
        for curve in mesh_silhouettes(&mesh, &meshed_faces, view, scale.tolerance) {
            models.push((curve, LineKind::Silhouette, index, meshed_faces.clone()));
        }
    }
    Ok(())
}

/// The lines projected into the view plane; lines seen end-on are dropped.
fn projected(models: Vec<ModelLine>, view: &View, scale: &Scale) -> Vec<Source> {
    models
        .into_iter()
        .filter_map(|(model, kind, source, near_faces)| {
            let flat = flatten(&model, view, scale.focus)?;
            (extent(&flat) > scale.flat).then(|| Source {
                straight: ends(&model).is_some(),
                model,
                flat,
                kind,
                source,
                near_faces,
            })
        })
        .collect()
}

/// Where each projected line crosses the others, as points on it.
fn crossing_points(sources: &[Source], flat: f64) -> Result<Vec<Vec<Point>>, KernelError> {
    let boxes: Vec<([f64; 2], [f64; 2])> = sources.iter().map(|s| bounds2(&s.flat)).collect();
    let overlap = |i: usize, j: usize| {
        let (a, b) = (boxes[i], boxes[j]);
        (0..2).all(|k| a.0[k] <= b.1[k] + flat && b.0[k] <= a.1[k] + flat)
    };
    let mut cuts: Vec<Vec<Point>> = vec![Vec::new(); sources.len()];
    for i in 0..sources.len() {
        for j in 0..sources.len() {
            if i == j || !overlap(i, j) {
                continue;
            }
            match (ends(&sources[i].flat), ends(&sources[j].flat)) {
                (Some(a), Some(b)) => {
                    if let Some(p) = line_crossing(a, b) {
                        cuts[i].push(p);
                    }
                }
                _ => {
                    if j > i {
                        for p in crate::crossings(&sources[i].flat, &sources[j].flat)? {
                            cuts[i].push(p);
                            cuts[j].push(p);
                        }
                    }
                }
            }
        }
    }
    Ok(cuts)
}

/// The line `source` cut where others cross it, its pieces in runs of the same visibility. A
/// closed curve's last run carries on into its first.
fn visibility_runs(
    source: &Source,
    cuts: &[Point],
    bodies: &[Body],
    scene: &mut RayScene,
    view: &View,
    scale: &Scale,
) -> Result<Vec<(bool, Vec<Curve>)>, KernelError> {
    let pieces = if cuts.is_empty() {
        vec![source.flat.clone()]
    } else {
        source.flat.cut_at(cuts)?
    };
    let mut runs: Vec<(bool, Vec<Curve>)> = Vec::new();
    for piece in pieces {
        if extent(&piece) <= scale.flat {
            continue;
        }
        let visible = piece_visible(source, &piece, bodies, scene, view, scale);
        match runs.last_mut() {
            Some((v, run)) if *v == visible => run.push(piece),
            _ => runs.push((visible, vec![piece])),
        }
    }
    let closed = source.flat.is_closed().unwrap_or(false);
    if closed && runs.len() > 1 && runs[0].0 == runs[runs.len() - 1].0 {
        let (_, last) = runs.pop().unwrap_or_default();
        runs[0].1.splice(0..0, last);
    }
    Ok(runs)
}

/// Whether a piece of the line `source` is seen: nothing blocks the way from the model point
/// under its middle to the eye.
fn piece_visible(
    source: &Source,
    piece: &Curve,
    bodies: &[Body],
    scene: &mut RayScene,
    view: &View,
    scale: &Scale,
) -> bool {
    match middle(piece).and_then(|m| model_point(&source.model, m, view, scale.focus)) {
        Some(at) => {
            let at = off_tessellation(source, at, bodies, scale.tolerance);
            let (way, far) = view.way_to_eye(at);
            let short = if source.kind == LineKind::Silhouette {
                scale.grazing()
            } else {
                0.0
            };
            let b = scene.blocked_short(at, way, far, short);
            !b
        }
        None => true,
    }
}

/// A point of the line `source` ready for its visibility test. A tessellation's silhouette lies
/// a little off the true one, inside the surface: its point is moved onto the exact surface,
/// then off it along the surface's normal by a few times that error, so the surface it outlines
/// is not taken for what hides it.
fn off_tessellation(source: &Source, at: Point, bodies: &[Body], tolerance: f64) -> Point {
    if source.near_faces.is_empty() {
        return at;
    }
    let body = &bodies[source.source];
    let nearest = source
        .near_faces
        .iter()
        .filter_map(|face| body.face_closest_point(*face, at).ok().map(|p| (*face, p)))
        .min_by(|a, b| length(sub(a.1, at)).total_cmp(&length(sub(b.1, at))));
    match nearest {
        Some((face, on)) => {
            let error = length(sub(on, at)).max(tolerance * 10.0);
            match body.face_normal_near(face, on) {
                Ok(normal) => plus(on, normal, error * 4.0),
                Err(_) => on,
            }
        }
        None => at,
    }
}

/// The runs of the line `source` as drawn lines: the whole line when it is all one run, a
/// straight line from end to end of each run, or each run's pieces joined.
fn drawn_lines(source: &Source, runs: Vec<(bool, Vec<Curve>)>, drawn: &mut Vec<DrawnLine>) {
    let whole = runs.len() == 1;
    for (visible, run) in runs {
        let curves: Vec<Curve> = if whole {
            vec![source.flat.clone()]
        } else if source.straight {
            match (run.first().and_then(ends), run.last().and_then(ends)) {
                (Some((a, _)), Some((_, b))) => Curve::line(a, b).into_iter().collect(),
                _ => run,
            }
        } else {
            crate::join_all(&run).unwrap_or(run)
        };
        for curve in curves {
            drawn.push(DrawnLine {
                curve,
                visible,
                kind: source.kind,
                source: source.source,
            });
        }
    }
}

/// The drawn lines without those drawn twice, or hidden ones lying under visible ones: visible
/// lines first, then each line keeps only what no line already kept covers.
fn without_overlaps(mut drawn: Vec<DrawnLine>, options: Options, flat: f64) -> Vec<DrawnLine> {
    let mut kept: Vec<DrawnLine> = Vec::new();
    drawn.sort_by_key(|line| {
        (
            !line.visible,
            options.first_from.is_none_or(|from| line.source < from),
        )
    });
    for line in drawn {
        let over: Vec<&Curve> = kept
            .iter()
            .filter(|k| k.visible || !line.visible)
            .map(|k| &k.curve)
            .collect();
        if let Some((a, b)) = ends(&line.curve) {
            for (p, q) in uncovered((a, b), &over, flat) {
                if let Ok(curve) = Curve::line(p, q) {
                    kept.push(DrawnLine {
                        curve,
                        ..line.clone()
                    });
                }
            }
        } else if !over.iter().any(|k| lies_on(&line.curve, k, flat)) {
            kept.push(line);
        }
    }
    kept
}

/// The parts of the line `a`–`b` that no line among `over` lying along it covers, each
/// longer than `tolerance`.
fn uncovered((a, b): (Point, Point), over: &[&Curve], tolerance: f64) -> Vec<(Point, Point)> {
    let span = sub(b, a);
    let total = length(span);
    if total <= tolerance {
        return Vec::new();
    }
    let dir = span.map(|c| c / total);
    // Where along `a`–`b` a point falls, and how far off the line it is.
    let place = |p: Point| {
        let t = dot(sub(p, a), dir);
        (t, length(sub(sub(p, a), dir.map(|c| c * t))))
    };
    let mut covers: Vec<(f64, f64)> = over
        .iter()
        .filter_map(|curve| ends(curve))
        .filter_map(|(p, q)| {
            let ((tp, op), (tq, oq)) = (place(p), place(q));
            (op <= tolerance && oq <= tolerance).then_some((tp.min(tq), tp.max(tq)))
        })
        .collect();
    covers.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut free = Vec::new();
    let mut at = 0.0;
    for (from, to) in covers {
        if from > at + tolerance {
            free.push((at, from.min(total)));
        }
        at = at.max(to);
        if at >= total - tolerance {
            break;
        }
    }
    if at < total - tolerance {
        free.push((at, total));
    }
    free.into_iter()
        .filter(|(s, e)| e - s > tolerance)
        .map(|(s, e)| {
            // The line's own ends exactly where it keeps them.
            let point = |t: f64| {
                if t <= tolerance {
                    a
                } else if t >= total - tolerance {
                    b
                } else {
                    plus(a, dir, t)
                }
            };
            (point(s), point(e))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top() -> View {
        View {
            eye: [0.0, 0.0, 1000.0],
            forward: [0.0, 0.0, -1.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            parallel: true,
        }
    }

    fn total(lines: &[DrawnLine], visible: bool) -> f64 {
        lines
            .iter()
            .filter(|l| l.visible == visible)
            .map(|l| l.curve.length().unwrap())
            .sum()
    }

    fn count(lines: &[DrawnLine], visible: bool) -> usize {
        lines.iter().filter(|l| l.visible == visible).count()
    }

    #[test]
    fn a_square_is_four_clean_connected_lines() {
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let lines = hidden_lines(&[], &[square], &top(), Options::default()).unwrap();
        assert_eq!(lines.len(), 4);
        assert!(
            lines
                .iter()
                .all(|l| l.visible && matches!(l.curve, Curve::Line { .. }))
        );
        // Each corner is where exactly two lines end.
        let mut corners: Vec<Point> = Vec::new();
        for line in &lines {
            let (a, b) = ends(&line.curve).unwrap();
            corners.push(a);
            corners.push(b);
        }
        for c in &corners {
            let meeting = corners
                .iter()
                .filter(|p| length(sub(**p, *c)) < 1e-12)
                .count();
            assert_eq!(meeting, 2);
        }
    }

    #[test]
    fn a_box_from_above_is_its_four_outline_lines() {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let lines = hidden_lines(&[cube], &[], &top(), Options::default()).unwrap();
        assert_eq!(count(&lines, true), 4, "{lines:?}");
        assert_eq!(count(&lines, false), 0, "the bottom lies under the top");
        assert!(lines.iter().all(|l| matches!(l.curve, Curve::Line { .. })));
        assert!((total(&lines, true) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn a_box_at_an_angle_has_nine_visible_and_three_hidden_edges() {
        let d = unit([-1.0, 1.2, -0.8]).unwrap();
        let right = unit(cross(d, [0.0, 0.0, 1.0])).unwrap();
        let view = View {
            eye: plus([5.0, 5.0, 5.0], d, -1000.0),
            forward: d,
            right,
            up: cross(right, d),
            parallel: true,
        };
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let lines = hidden_lines(&[cube], &[], &view, Options::default()).unwrap();
        assert_eq!(count(&lines, true), 9, "{lines:?}");
        assert_eq!(count(&lines, false), 3);
        assert!(lines.iter().all(|l| matches!(l.curve, Curve::Line { .. })));
    }

    #[test]
    fn a_line_under_a_box_is_hidden_only_where_covered() {
        let cube = Body::cuboid([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]).unwrap();
        let under = Curve::line([-10.0, 5.0, -1.0], [20.0, 5.0, -1.0]).unwrap();
        let lines = hidden_lines(&[cube], &[under], &top(), Options::default()).unwrap();
        let of_line: Vec<DrawnLine> = lines.into_iter().filter(|l| l.source == 1).collect();
        assert_eq!(of_line.len(), 3, "visible, hidden, visible");
        assert!((total(&of_line, false) - 10.0).abs() < 1e-6);
        assert!((total(&of_line, true) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn a_cylinder_from_the_side_has_two_straight_exact_silhouettes() {
        let side = View {
            eye: [0.0, -1000.0, 0.0],
            forward: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            parallel: true,
        };
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let cylinder = Body::extruded(&[circle], [0.0, 0.0, 1.0], 20.0, true).unwrap();
        let lines = hidden_lines(&[cylinder], &[], &side, Options::default()).unwrap();
        let outlines: Vec<&DrawnLine> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Silhouette && l.visible)
            .collect();
        assert_eq!(outlines.len(), 2, "{lines:?}");
        for line in outlines {
            let (a, b) = ends(&line.curve).expect("a straight line");
            assert!((a[0].abs() - 5.0).abs() < 1e-6 && (b[0].abs() - 5.0).abs() < 1e-6);
            assert!(
                (line.curve.length().unwrap() - 20.0).abs() < 1e-3,
                "{:?}",
                line.curve
            );
        }
    }

    #[test]
    fn a_box_partly_behind_another_is_hidden_where_covered() {
        let front = Body::cuboid([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]).unwrap();
        let back = Body::cuboid([5.0, 5.0, -20.0], [10.0, 10.0, 10.0]).unwrap();
        let lines = hidden_lines(&[front, back], &[], &top(), Options::default()).unwrap();
        // The front box's square; the back box's square, its corner under the front one
        // hidden: two hidden pieces, of 5 each.
        assert_eq!(count(&lines, false), 2, "{lines:?}");
        assert!((total(&lines, false) - 10.0).abs() < 1e-6);
        assert!((total(&lines, true) - (40.0 + 30.0)).abs() < 1e-6);
    }

    #[test]
    fn a_box_in_perspective_is_clean_lines() {
        let eye = [40.0, -50.0, 35.0];
        let forward = unit(sub([5.0, 5.0, 5.0], eye)).unwrap();
        let right = unit(cross(forward, [0.0, 0.0, 1.0])).unwrap();
        let view = View {
            eye,
            forward,
            right,
            up: cross(right, forward),
            parallel: false,
        };
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let lines = hidden_lines(&[cube], &[], &view, Options::default()).unwrap();
        assert_eq!(count(&lines, true), 9, "{lines:?}");
        assert_eq!(count(&lines, false), 3);
        assert!(lines.iter().all(|l| matches!(l.curve, Curve::Line { .. })));
    }

    #[test]
    fn a_sphere_from_above_is_a_round_outline() {
        let half = Curve::arc(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, -5.0],
            [0.0, 0.0, 5.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        );
        let Ok(half) = half else {
            return;
        };
        let sphere = Body::revolved(
            &[half],
            [0.0; 3],
            [0.0, 0.0, 1.0],
            std::f64::consts::TAU,
            true,
        )
        .unwrap();
        let lines = hidden_lines(&[sphere], &[], &top(), Options::default()).unwrap();
        let outline: Vec<&DrawnLine> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Silhouette && l.visible)
            .collect();
        assert!(!outline.is_empty(), "{lines:?}");
        for line in outline {
            for p in line.curve.sample().unwrap() {
                assert!((p[0].hypot(p[1]) - 5.0).abs() < 0.05, "{p:?}");
            }
        }
    }

    #[test]
    fn many_boxes_are_drawn_quickly() {
        let boxes: Vec<Body> = (0..5)
            .flat_map(|i| (0..5).map(move |j| (i, j)))
            .map(|(i, j)| {
                Body::cuboid(
                    [f64::from(i) * 15.0, f64::from(j) * 15.0, 0.0],
                    [10.0, 10.0, 10.0],
                )
                .unwrap()
            })
            .collect();
        let d = unit([-1.0, 1.2, -0.8]).unwrap();
        let right = unit(cross(d, [0.0, 0.0, 1.0])).unwrap();
        let view = View {
            eye: plus([30.0, 30.0, 5.0], d, -1000.0),
            forward: d,
            right,
            up: cross(right, d),
            parallel: true,
        };
        let started = std::time::Instant::now();
        let lines = hidden_lines(&boxes, &[], &view, Options::default()).unwrap();
        let took = started.elapsed();
        assert!(count(&lines, true) >= 25 * 6, "{}", count(&lines, true));
        // A debug build; release is several times faster.
        assert!(took.as_secs_f64() < 20.0, "{took:?}");
        eprintln!("25 boxes in {took:?}, {} lines", lines.len());
    }

    #[test]
    fn a_sphere_in_perspective_has_its_outline() {
        let half = Curve::arc(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 10.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        )
        .unwrap();
        let sphere = Body::revolved(
            &[half],
            [0.0; 3],
            [0.0, 0.0, 1.0],
            std::f64::consts::TAU,
            true,
        )
        .unwrap();
        eprintln!(
            "surfaces: {:?}",
            sphere
                .face_surfaces()
                .unwrap()
                .iter()
                .map(std::mem::discriminant)
                .collect::<Vec<_>>()
        );
        let eye = [100.0, -120.0, 80.0];
        let forward = unit(sub([0.0; 3], eye)).unwrap();
        let right = unit(cross(forward, [0.0, 0.0, 1.0])).unwrap();
        let view = View {
            eye,
            forward,
            right,
            up: cross(right, forward),
            parallel: false,
        };
        let lines = hidden_lines(&[sphere], &[], &view, Options::default()).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.kind == LineKind::Silhouette && l.visible)
        );
    }
}
