//! Hidden-line removal for Make2D and printed technical views (spec `make2d.md` §2): what lines
//! of bodies and curves show from a view, and which are hidden.
//!
//! The steps are the spec's: the lines to draw (bodies' edges, the silhouettes of their curved
//! faces, and curves) are projected into the view plane, split wherever they cross the projected
//! outlines that could cover them, and each piece is tested at its middle by a ray towards the
//! eye; pieces with the same visibility are joined again.
//!
//! The spec tests visibility with solidcore's B-rep ray tracer; it is not yet wrapped for Curva,
//! so the rays are tested against a tessellation of the bodies through a bounding-volume tree,
//! and silhouettes come from the tessellation, as Curva's `Silhouette` does. Pieces are exact
//! where the drawn curves are; their visibility is right to the tessellation's tolerance. Listed
//! as custom kernel operation `hidden-lines`.

use crate::{Body, Curve, KernelError, Point};

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
}

/// A line of the drawing, in the view plane: right and up from the view's centre line, in model
/// units (a perspective view is scaled to the size of the scene at `focus`).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLine {
    /// Its points in the view plane.
    pub points: Vec<[f64; 2]>,
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

fn lerp(a: Point, b: Point, t: f64) -> Point {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

impl View {
    /// A point in the view plane, or `None` behind a perspective eye. `focus` is the distance
    /// at which a perspective view keeps model size.
    fn project(&self, p: Point, focus: f64) -> Option<[f64; 2]> {
        let d = sub(p, self.eye);
        let (x, y) = (dot(d, self.right), dot(d, self.up));
        if self.parallel {
            return Some([x, y]);
        }
        let depth = dot(d, self.forward);
        (depth > 1e-9).then(|| [x * focus / depth, y * focus / depth])
    }

    /// The way to the eye from `p`, of length 1, and how far it goes.
    fn way_to_eye(&self, p: Point) -> (Point, f64) {
        if self.parallel {
            return (self.forward.map(|c| -c), f64::INFINITY);
        }
        let d = sub(self.eye, p);
        let length = dot(d, d).sqrt().max(1e-12);
        (d.map(|c| c / length), length)
    }

    /// Whether a triangle faces the eye; `None` when it is seen edge-on, so either way is
    /// rounding.
    fn faces(&self, [a, b, c]: [Point; 3]) -> Option<bool> {
        let normal = cross(sub(b, a), sub(c, a));
        let towards = if self.parallel {
            self.forward.map(|c| -c)
        } else {
            sub(self.eye, a)
        };
        let scale = (dot(normal, normal) * dot(towards, towards)).sqrt();
        let along = dot(normal, towards);
        (along.abs() > 1e-9 * scale).then_some(along > 0.0)
    }
}

/// A bounding-volume tree over triangles, for rays.
struct Tree {
    triangles: Vec<[Point; 3]>,
    nodes: Vec<Node>,
}

struct Node {
    low: Point,
    high: Point,
    /// Children, or the range of triangles in a leaf.
    kind: NodeKind,
}

enum NodeKind {
    Inner(usize, usize),
    Leaf(usize, usize),
}

/// Triangles per leaf.
const LEAF: usize = 8;

impl Tree {
    fn new(mut triangles: Vec<[Point; 3]>) -> Self {
        let mut nodes = Vec::new();
        if !triangles.is_empty() {
            let count = triangles.len();
            Self::build(&mut triangles, 0, count, &mut nodes);
        }
        Self { triangles, nodes }
    }

    fn bounds(triangles: &[[Point; 3]]) -> (Point, Point) {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for p in triangles.iter().flatten() {
            for i in 0..3 {
                low[i] = low[i].min(p[i]);
                high[i] = high[i].max(p[i]);
            }
        }
        (low, high)
    }

    fn build(
        triangles: &mut [[Point; 3]],
        start: usize,
        end: usize,
        nodes: &mut Vec<Node>,
    ) -> usize {
        let (low, high) = Self::bounds(&triangles[start..end]);
        let index = nodes.len();
        nodes.push(Node {
            low,
            high,
            kind: NodeKind::Leaf(start, end),
        });
        if end - start <= LEAF {
            return index;
        }
        // Split at the middle along the longest side, by the triangles' centres.
        let size = sub(high, low);
        let axis = (0..3)
            .max_by(|a, b| size[*a].total_cmp(&size[*b]))
            .unwrap_or(0);
        let centre = |t: &[Point; 3]| t[0][axis] + t[1][axis] + t[2][axis];
        triangles[start..end].sort_by(|a, b| centre(a).total_cmp(&centre(b)));
        let middle = start + (end - start) / 2;
        let left = Self::build(triangles, start, middle, nodes);
        let right = Self::build(triangles, middle, end, nodes);
        nodes[index].kind = NodeKind::Inner(left, right);
        index
    }

    /// Whether a ray from `from` along `way` meets a triangle between `near` and `far`.
    fn blocked(&self, from: Point, way: Point, near: f64, far: f64) -> bool {
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let Some(node) = self.nodes.get(index) else {
                continue;
            };
            if !slab(node.low, node.high, from, way, far) {
                continue;
            }
            match node.kind {
                NodeKind::Inner(left, right) => {
                    stack.push(left);
                    stack.push(right);
                }
                NodeKind::Leaf(start, end) => {
                    for triangle in &self.triangles[start..end] {
                        if let Some(t) = ray_triangle(from, way, *triangle)
                            && t > near
                            && t < far
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Whether a ray meets a box before `far`.
fn slab(low: Point, high: Point, from: Point, way: Point, far: f64) -> bool {
    let (mut t0, mut t1) = (0.0_f64, far);
    for i in 0..3 {
        if way[i].abs() < 1e-15 {
            if from[i] < low[i] || from[i] > high[i] {
                return false;
            }
            continue;
        }
        let (a, b) = ((low[i] - from[i]) / way[i], (high[i] - from[i]) / way[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Where a ray meets a triangle, as its distance along the ray (Möller and Trumbore).
fn ray_triangle(from: Point, way: Point, [a, b, c]: [Point; 3]) -> Option<f64> {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(way, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-18 {
        return None;
    }
    let s = sub(from, a);
    let u = dot(s, p) / det;
    // A hit on a triangle's border counts: a line exactly behind an edge is covered.
    let margin = 1e-9;
    if !(-margin..=1.0 + margin).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(way, q) / det;
    if v < -margin || u + v > 1.0 + margin {
        return None;
    }
    Some(dot(e2, q) / det)
}

/// Where two segments in the plane cross, as the share of the way along the first.
fn crossing(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let denom = r[0] * s[1] - r[1] * s[0];
    if denom.abs() < 1e-18 {
        return None;
    }
    let w = [c[0] - a[0], c[1] - a[1]];
    let t = (w[0] * s[1] - w[1] * s[0]) / denom;
    let u = (w[0] * r[1] - w[1] * r[0]) / denom;
    (t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
}

/// A grid over the plane's segments, to find which might cross a given one quickly.
struct Grid {
    low: [f64; 2],
    cell: f64,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<usize>>,
}

impl Grid {
    fn new(segments: &[([f64; 2], [f64; 2])]) -> Self {
        let mut low = [f64::INFINITY; 2];
        let mut high = [f64::NEG_INFINITY; 2];
        for (a, b) in segments {
            for p in [a, b] {
                for i in 0..2 {
                    low[i] = low[i].min(p[i]);
                    high[i] = high[i].max(p[i]);
                }
            }
        }
        let span = (high[0] - low[0]).max(high[1] - low[1]).max(1e-9);
        let side = ((segments.len() as f64).sqrt().ceil() as usize).clamp(1, 512);
        let cell = span / side as f64;
        let columns = (((high[0] - low[0]) / cell).floor() as usize + 1).max(1);
        let rows = (((high[1] - low[1]) / cell).floor() as usize + 1).max(1);
        let mut grid = Self {
            low,
            cell,
            columns,
            rows,
            cells: vec![Vec::new(); columns * rows],
        };
        for (index, (a, b)) in segments.iter().enumerate() {
            for cell in grid.cells_of(*a, *b) {
                grid.cells[cell].push(index);
            }
        }
        grid
    }

    fn cells_of(&self, a: [f64; 2], b: [f64; 2]) -> Vec<usize> {
        let at = |v: f64, i: usize, most: usize| {
            (((v - self.low[i]) / self.cell).floor().max(0.0) as usize).min(most - 1)
        };
        let (c0, c1) = (
            at(a[0].min(b[0]), 0, self.columns),
            at(a[0].max(b[0]), 0, self.columns),
        );
        let (r0, r1) = (
            at(a[1].min(b[1]), 1, self.rows),
            at(a[1].max(b[1]), 1, self.rows),
        );
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                out.push(r * self.columns + c);
            }
        }
        out
    }
}

/// A 3D line to draw, before splitting.
struct Line3 {
    points: Vec<Point>,
    kind: LineKind,
    source: usize,
}

/// A mesh edge by its corners' rounded positions, lowest first.
type MeshEdge = ([i64; 3], [i64; 3]);

/// Silhouette segments of a mesh: edges inside it between a triangle that faces the eye and one
/// that faces away. Corners are matched by position, so a face's seam still joins.
fn mesh_silhouettes(mesh: &crate::BodyMesh, view: &View, tolerance: f64) -> Vec<[Point; 2]> {
    let key = |p: Point| p.map(|c| (c / tolerance).round() as i64);
    let mut sides: std::collections::HashMap<MeshEdge, (usize, Vec<Option<bool>>)> =
        std::collections::HashMap::new();
    for (index, t) in mesh.triangles.iter().enumerate() {
        let corners = t.map(|i| mesh.positions[i as usize]);
        let facing = view.faces(corners);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (ka, kb) = (key(corners[i]), key(corners[j]));
            let edge = if ka < kb { (ka, kb) } else { (kb, ka) };
            let entry = sides.entry(edge).or_insert((index * 3 + i, Vec::new()));
            entry.1.push(facing);
        }
    }
    let mut out = Vec::new();
    for (first, facings) in sides.values() {
        if let [Some(one), Some(other)] = facings[..]
            && one != other
        {
            let t = mesh.triangles[first / 3];
            let i = first % 3;
            let (a, b) = (t[i] as usize, t[(i + 1) % 3] as usize);
            out.push([mesh.positions[a], mesh.positions[b]]);
        }
    }
    out
}

/// The lines of `bodies` and `curves` seen from `view`, split into visible and hidden pieces.
///
/// # Errors
///
/// When a body cannot be tessellated or a curve sampled.
// CUSTOM KERNEL OPERATION: hidden-lines
pub fn hidden_lines(
    bodies: &[Body],
    curves: &[Curve],
    view: &View,
) -> Result<Vec<DrawnLine>, KernelError> {
    let meshes: Vec<crate::BodyMesh> = bodies.iter().map(Body::mesh).collect::<Result<_, _>>()?;
    let mut lines: Vec<Line3> = Vec::new();
    let mut triangles = Vec::new();
    let mut everything: Vec<Point> = Vec::new();
    for (source, mesh) in meshes.iter().enumerate() {
        everything.extend(mesh.positions.iter().copied());
        triangles.extend(
            mesh.triangles
                .iter()
                .map(|t| t.map(|i| mesh.positions[i as usize])),
        );
        for edge in &mesh.edges {
            lines.push(Line3 {
                points: edge.clone(),
                kind: LineKind::Curve,
                source,
            });
        }
    }
    for (index, curve) in curves.iter().enumerate() {
        let points = curve.sample()?;
        everything.extend(points.iter().copied());
        lines.push(Line3 {
            points,
            kind: LineKind::Curve,
            source: bodies.len() + index,
        });
    }
    let Some(&first) = everything.first() else {
        return Ok(Vec::new());
    };
    let (low, high) = everything.iter().fold((first, first), |(lo, hi), p| {
        (
            std::array::from_fn(|i| lo[i].min(p[i])),
            std::array::from_fn(|i| hi[i].max(p[i])),
        )
    });
    let size = dot(sub(high, low), sub(high, low)).sqrt().max(1e-9);
    let tolerance = size * 1e-7;
    for (source, mesh) in meshes.iter().enumerate() {
        for [a, b] in mesh_silhouettes(mesh, view, tolerance) {
            lines.push(Line3 {
                points: vec![a, b],
                kind: LineKind::Silhouette,
                source,
            });
        }
    }
    let centre = lerp(low, high, 0.5);
    let focus = dot(sub(centre, view.eye), view.forward).max(1e-9);
    // Every projected segment, as outlines that may cover what is behind them.
    let mut outlines: Vec<([f64; 2], [f64; 2])> = Vec::new();
    let mut owner: Vec<(usize, usize)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        for (k, pair) in line.points.windows(2).enumerate() {
            if let (Some(a), Some(b)) = (view.project(pair[0], focus), view.project(pair[1], focus))
            {
                outlines.push((a, b));
                owner.push((index, k));
            }
        }
    }
    let grid = Grid::new(&outlines);
    let tree = Tree::new(triangles);
    // How far a ray starts from the line, past faces the line itself lies on.
    let near = size * 2e-4;
    let mut drawn = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let mut current: Option<DrawnLine> = None;
        for (k, pair) in line.points.windows(2).enumerate() {
            let (Some(a2), Some(b2)) = (view.project(pair[0], focus), view.project(pair[1], focus))
            else {
                if let Some(done) = current.take() {
                    drawn.push(done);
                }
                continue;
            };
            let mut cuts = vec![0.0, 1.0];
            let mut seen = std::collections::HashSet::new();
            for cell in grid.cells_of(a2, b2) {
                for &other in &grid.cells[cell] {
                    if !seen.insert(other) {
                        continue;
                    }
                    let (o_line, o_k) = owner[other];
                    if o_line == index && o_k.abs_diff(k) <= 1 {
                        continue;
                    }
                    let (c, d) = outlines[other];
                    if let Some(t) = crossing(a2, b2, c, d) {
                        cuts.push(t);
                    }
                }
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            for span in cuts.windows(2) {
                let (t0, t1) = (span[0], span[1]);
                let middle = lerp(pair[0], pair[1], (t0 + t1) / 2.0);
                let (way, far) = view.way_to_eye(middle);
                let visible = !tree.blocked(middle, way, near, far);
                let p0 = view
                    .project(lerp(pair[0], pair[1], t0), focus)
                    .unwrap_or(a2);
                let p1 = view
                    .project(lerp(pair[0], pair[1], t1), focus)
                    .unwrap_or(b2);
                match &mut current {
                    Some(run) if run.visible == visible => run.points.push(p1),
                    _ => {
                        if let Some(done) = current.take() {
                            drawn.push(done);
                        }
                        current = Some(DrawnLine {
                            points: vec![p0, p1],
                            visible,
                            kind: line.kind,
                            source: line.source,
                        });
                    }
                }
            }
        }
        if let Some(done) = current {
            drawn.push(done);
        }
    }
    Ok(drawn)
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

    fn length(lines: &[DrawnLine], visible: bool) -> f64 {
        lines
            .iter()
            .filter(|l| l.visible == visible)
            .flat_map(|l| {
                l.points
                    .windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            })
            .sum()
    }

    #[test]
    fn a_box_from_above_shows_its_top_and_hides_its_bottom() {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let lines = hidden_lines(&[cube], &[], &top()).unwrap();
        // The top square shows; the bottom square is hidden under it (the upright edges are
        // points from above).
        assert!(
            (length(&lines, true) - 40.0).abs() < 1e-6,
            "{}",
            length(&lines, true)
        );
        assert!(
            (length(&lines, false) - 40.0).abs() < 1e-6,
            "{}",
            length(&lines, false)
        );
    }

    #[test]
    fn a_line_under_a_box_is_hidden_where_the_box_covers_it() {
        let cube = Body::cuboid([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]).unwrap();
        let under = Curve::line([-10.0, 5.0, -1.0], [20.0, 5.0, -1.0]).unwrap();
        let lines = hidden_lines(&[cube], &[under], &top()).unwrap();
        let of_line: Vec<DrawnLine> = lines.into_iter().filter(|l| l.source == 1).collect();
        assert!((length(&of_line, false) - 10.0).abs() < 1e-6);
        assert!((length(&of_line, true) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn a_cylinder_from_the_side_has_silhouettes() {
        let side = View {
            eye: [0.0, -1000.0, 0.0],
            forward: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            parallel: true,
        };
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let cylinder = Body::extruded(&[circle], [0.0, 0.0, 1.0], 20.0, true).unwrap();
        let lines = hidden_lines(&[cylinder], &[], &side).unwrap();
        let outlines: Vec<&DrawnLine> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Silhouette && l.visible)
            .collect();
        assert!(!outlines.is_empty(), "no silhouette");
        // They run up the sides, at about x = ±5.
        assert!(
            outlines
                .iter()
                .all(|l| l.points.iter().all(|p| (p[0].abs() - 5.0).abs() < 0.2))
        );
    }
}
