//! Object snaps, ortho and grid snap: turning a cursor position into an exact point.

use crate::camera::{CPlane, Projector};
use crate::{Camera, Outlines};
use curva_kernel::{Curve, Point, closest_between};

/// How close to something the cursor must be to snap to it, in pixels.
const SNAP_REACH: f64 = 12.0;
/// How close two curves must come, in millimetres, to count as meeting.
const MEETING_TOLERANCE: f64 = 1e-3;
/// The spacing grid snap rounds to, in millimetres.
const GRID_SNAP: f64 = 1.0;
/// Steps of the search along a segment; each keeps two thirds of what is left.
const NEAR_SEARCH_STEPS: usize = 48;

/// What a point snapped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// An end of a curve or a corner.
    End,
    /// The middle of a segment or smooth curve.
    Mid,
    /// The centre of a circle.
    Center,
    /// Where two curves meet.
    Intersection,
    /// The foot of a right angle from the last point.
    Perpendicular,
    /// The closest point on a curve.
    Near,
}

/// Which object snaps are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // One switch per checkbox is what this is.
pub struct SnapSet {
    /// Ends and corners.
    pub end: bool,
    /// Middles.
    pub mid: bool,
    /// Centres.
    pub center: bool,
    /// Where curves meet.
    pub intersection: bool,
    /// Right angles from the last point.
    pub perpendicular: bool,
    /// The closest point on a curve.
    pub near: bool,
    /// Only points that can be seen, not hidden behind a face: on in shaded and ghosted views,
    /// off in wireframe ones, where everything can be seen. Set by the view, not a checkbox.
    pub visible_only: bool,
}

impl Default for SnapSet {
    /// Everything but Near, which would otherwise hide the others.
    fn default() -> Self {
        Self {
            end: true,
            mid: true,
            center: true,
            intersection: true,
            perpendicular: true,
            near: false,
            visible_only: false,
        }
    }
}

impl SnapSet {
    fn has(&self, kind: SnapKind) -> bool {
        match kind {
            SnapKind::End => self.end,
            SnapKind::Mid => self.mid,
            SnapKind::Center => self.center,
            SnapKind::Intersection => self.intersection,
            SnapKind::Perpendicular => self.perpendicular,
            SnapKind::Near => self.near,
        }
    }
}

/// A point the cursor snapped to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapHit {
    /// What it snapped to.
    pub kind: SnapKind,
    /// The exact point.
    pub point: Point,
}

/// A fixed place on an object that can be snapped to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Site {
    kind: SnapKind,
    point: Point,
}

fn along(a: Point, b: Point, t: f64) -> Point {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: Point) -> f64 {
    dot(a, a).sqrt()
}

fn site(kind: SnapKind, point: Point) -> Site {
    Site { kind, point }
}

/// The point half way along a run of points, by distance.
fn half_way(points: &[Point]) -> Option<Point> {
    let total: f64 = points.windows(2).map(|p| length(sub(p[1], p[0]))).sum();
    let mut left = total / 2.0;
    for pair in points.windows(2) {
        let step = length(sub(pair[1], pair[0]));
        if step >= left && step > 0.0 {
            return Some(along(pair[0], pair[1], left / step));
        }
        left -= step;
    }
    None
}

/// The fixed snap places on a curve, given the points it is drawn through.
pub(crate) fn sites_of(curve: &Curve, points: &[Point]) -> Vec<Site> {
    match curve {
        Curve::Point { at } => vec![site(SnapKind::End, *at)],
        Curve::Line { .. } | Curve::Polyline { .. } => points
            .iter()
            .map(|p| site(SnapKind::End, *p))
            .chain(
                points
                    .windows(2)
                    .map(|p| site(SnapKind::Mid, along(p[0], p[1], 0.5))),
            )
            .collect(),
        Curve::Circle { centre, .. } => vec![site(SnapKind::Center, *centre)],
        Curve::Arc { centre, .. } => points
            .first()
            .into_iter()
            .chain(points.last())
            .map(|p| site(SnapKind::End, *p))
            .chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
            .chain(std::iter::once(site(SnapKind::Center, *centre)))
            .collect(),
        Curve::Nurbs { .. } => {
            let open = points.first() != points.last();
            let ends = points
                .first()
                .into_iter()
                .chain(points.last())
                .filter(|_| open)
                .map(|p| site(SnapKind::End, *p));
            ends.chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
                .collect()
        }
        Curve::Compound { segments } => segments
            .iter()
            .filter_map(|part| Some(sites_of(part, &part.sample().ok()?)))
            .flatten()
            .collect(),
        Curve::Interpolated { closed, .. } => {
            let ends = points
                .first()
                .into_iter()
                .chain(points.last())
                .filter(|_| !closed)
                .map(|p| site(SnapKind::End, *p));
            ends.chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
                .collect()
        }
    }
}

struct NearbySegment {
    object: usize,
    a: Point,
    b: Point,
    /// How far from the cursor it is on screen, in pixels.
    gap: f64,
}

fn screen_gap(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let square = dx * dx + dy * dy;
    let t = if square < 1e-12 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / square).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Adds the segments of a run of points that are within reach of the cursor on screen.
fn collect_near(
    found: &mut Vec<NearbySegment>,
    view: &Projector,
    cursor: [f64; 2],
    object: usize,
    points: &[Point],
) {
    let mut previous: Option<(Point, Option<[f64; 2]>)> = None;
    for point in points {
        let screen = view.project(*point);
        if let Some((a, Some(from))) = previous
            && let Some(to) = screen
        {
            let gap = screen_gap(cursor, from, to);
            if gap <= SNAP_REACH {
                found.push(NearbySegment {
                    object,
                    a,
                    b: *point,
                    gap,
                });
            }
        }
        previous = Some((*point, screen));
    }
}

/// Where segments of different objects meet. Segments are sorted along X and only those that
/// overlap there are compared, as comparing every pair is far too slow when zoomed out.
fn meeting_points(segments: &[NearbySegment]) -> Vec<Point> {
    let spans = |segment: &NearbySegment| {
        let span = |axis: usize| {
            (
                segment.a[axis].min(segment.b[axis]) - MEETING_TOLERANCE,
                segment.a[axis].max(segment.b[axis]) + MEETING_TOLERANCE,
            )
        };
        [span(0), span(1), span(2)]
    };
    let mut sorted: Vec<(&NearbySegment, [(f64, f64); 3])> =
        segments.iter().map(|s| (s, spans(s))).collect();
    sorted.sort_by(|a, b| a.1[0].0.total_cmp(&b.1[0].0));
    let mut found = Vec::new();
    for (index, (one, one_spans)) in sorted.iter().enumerate() {
        for (other, other_spans) in sorted[index + 1..]
            .iter()
            .take_while(|(_, spans)| spans[0].0 <= one_spans[0].1)
        {
            let overlap = (1..3).all(|axis| {
                one_spans[axis].0 <= other_spans[axis].1 && other_spans[axis].0 <= one_spans[axis].1
            });
            if overlap && other.object != one.object {
                let (s, t) = closest_between((one.a, one.b), (other.a, other.b));
                let point = along(one.a, one.b, s);
                if length(sub(point, along(other.a, other.b, t))) <= MEETING_TOLERANCE {
                    found.push(point);
                }
            }
        }
    }
    found
}

/// The lowest and highest corners around a run of points.
pub(crate) fn box_around(points: &[Point]) -> Option<(Point, Point)> {
    let first = *points.first()?;
    Some(
        points
            .iter()
            .fold((first, first), |(mut low, mut high), p| {
                for axis in 0..3 {
                    low[axis] = low[axis].min(p[axis]);
                    high[axis] = high[axis].max(p[axis]);
                }
                (low, high)
            }),
    )
}

/// Whether a whole box is too far from the cursor on screen to matter, which skips most curves
/// without looking at their points. A box partly behind the camera is never skipped.
fn box_out_of_reach(view: &Projector, (low, high): (Point, Point), cursor: [f64; 2]) -> bool {
    let (mut left, mut right) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
    for corner in 0..8 {
        let pick = |axis: usize| {
            if corner & (1 << axis) == 0 {
                low[axis]
            } else {
                high[axis]
            }
        };
        let Some(at) = view.project([pick(0), pick(1), pick(2)]) else {
            return false;
        };
        left = left.min(at[0]);
        right = right.max(at[0]);
        top = top.min(at[1]);
        bottom = bottom.max(at[1]);
    }
    cursor[0] < left - SNAP_REACH
        || cursor[0] > right + SNAP_REACH
        || cursor[1] < top - SNAP_REACH
        || cursor[1] > bottom + SNAP_REACH
}

impl Outlines {
    fn segments_near(
        &self,
        view: &Projector,
        cursor: [f64; 2],
        drawing: &[Point],
    ) -> Vec<NearbySegment> {
        let mut found = Vec::new();
        for (object, (_, points)) in self.objects.iter().enumerate() {
            if !self
                .boxes
                .get(object)
                .is_some_and(|around| box_out_of_reach(view, *around, cursor))
            {
                collect_near(&mut found, view, cursor, object, points);
            }
        }
        collect_near(&mut found, view, cursor, usize::MAX, drawing);
        found
    }

    /// The object snap for a cursor position, if one is in reach. `from` is the last point,
    /// which Perpendicular measures from.
    #[must_use]
    pub fn snap(
        &self,
        camera: &Camera,
        cursor: [f64; 2],
        size: [f64; 2],
        set: &SnapSet,
        from: Option<Point>,
    ) -> Option<SnapHit> {
        self.snap_while_drawing(camera, cursor, size, set, from, &[])
    }

    /// As [`Self::snap`], also snapping to the points of the curve still being drawn, so that a
    /// polyline can snap to itself. The newest point is left out, as a curve cannot go from a
    /// point to that same point.
    #[must_use]
    pub fn snap_while_drawing(
        &self,
        camera: &Camera,
        cursor: [f64; 2],
        size: [f64; 2],
        set: &SnapSet,
        from: Option<Point>,
        drawing: &[Point],
    ) -> Option<SnapHit> {
        let view = camera.projector(size);
        let gap = |point: Point| {
            view.project(point)
                .map(|at| (at[0] - cursor[0]).hypot(at[1] - cursor[1]))
        };
        let mut offered: Vec<(f64, SnapHit)> = Vec::new();
        let mut offer = |kind: SnapKind, point: Point| {
            if let Some(distance) = gap(point)
                && distance <= SNAP_REACH
            {
                offered.push((distance, SnapHit { kind, point }));
            }
        };
        // In a shaded view, a point behind a face cannot be seen and is not offered.
        let seen = |point: Point| !set.visible_only || !self.hidden(camera, size, point);
        for place in self.sites.iter().filter(|place| set.has(place.kind)) {
            offer(place.kind, place.point);
        }
        if set.end {
            for point in &drawing[..drawing.len().saturating_sub(1)] {
                offer(SnapKind::End, *point);
            }
        }
        if set.mid {
            for pair in drawing.windows(2) {
                offer(SnapKind::Mid, along(pair[0], pair[1], 0.5));
            }
        }
        let segments = if set.intersection || set.perpendicular || set.near {
            self.segments_near(&view, cursor, drawing)
        } else {
            Vec::new()
        };
        if set.intersection {
            for point in meeting_points(&segments) {
                offer(SnapKind::Intersection, point);
            }
        }
        if set.perpendicular
            && let Some(from) = from
        {
            for segment in &segments {
                let direction = sub(segment.b, segment.a);
                let square = dot(direction, direction);
                let t = dot(sub(from, segment.a), direction) / square.max(1e-12);
                let foot = along(segment.a, segment.b, t);
                // At the last point itself there is no right angle to draw.
                if t > 0.0 && t < 1.0 && length(sub(foot, from)) > 1e-9 {
                    offer(SnapKind::Perpendicular, foot);
                }
            }
        }
        offered.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, hit)) = offered.into_iter().find(|(_, hit)| seen(hit.point)) {
            return Some(hit);
        }
        if !set.near {
            return None;
        }
        // The closest segment on screen holds the closest point, so only that one needs searching.
        segments
            .iter()
            .min_by(|a, b| a.gap.total_cmp(&b.gap))
            .map(|segment| SnapHit {
                kind: SnapKind::Near,
                point: along(segment.a, segment.b, nearest_along(segment, &gap)),
            })
            .filter(|hit| seen(hit.point))
    }
}

/// Where along a segment is closest to the cursor on screen. A search in 3D, as perspective
/// means a fraction along the segment on screen is not the same fraction in the model.
fn nearest_along(segment: &NearbySegment, gap: &impl Fn(Point) -> Option<f64>) -> f64 {
    let at = |t: f64| gap(along(segment.a, segment.b, t)).unwrap_or(f64::INFINITY);
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..NEAR_SEARCH_STEPS {
        let third = (high - low) / 3.0;
        if at(low + third) <= at(high - third) {
            high -= third;
        } else {
            low += third;
        }
    }
    (low + high) / 2.0
}

/// A position on a plane with grid snap and ortho applied. Ortho keeps to the plane's X or Y from
/// `base`, at the height of `base`, whichever axis the cursor has moved furthest along.
#[must_use]
pub fn constrained(
    plane: &CPlane,
    cursor: Point,
    base: Option<Point>,
    ortho: bool,
    grid: bool,
) -> Point {
    let local = |p: Point| {
        let offset = sub(p, plane.origin);
        [
            dot(offset, plane.x),
            dot(offset, plane.y),
            dot(offset, plane.normal),
        ]
    };
    let rounded = |value: f64| {
        if grid {
            (value / GRID_SNAP).round() * GRID_SNAP
        } else {
            value
        }
    };
    let at = local(cursor);
    let (x, y) = (rounded(at[0]), rounded(at[1]));
    let [u, v, height] = match base.map(local) {
        Some(base) if ortho => {
            if (at[0] - base[0]).abs() >= (at[1] - base[1]).abs() {
                [x, base[1], base[2]]
            } else {
                [base[0], y, base[2]]
            }
        }
        _ => [x, y, at[2]],
    };
    let world = |axis: usize| {
        plane.origin[axis] + plane.x[axis] * u + plane.y[axis] * v + plane.normal[axis] * height
    };
    [world(0), world(1), world(2)]
}

/// Points held by resting on them, and the guidelines they send out, as Rhino's SmartTrack and
/// ArchiCAD's guide lines do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guides {
    /// The held points, oldest first.
    pub held: Vec<Point>,
    /// The construction plane's X and Y directions, of length 1, which the guidelines follow.
    pub axes: [Point; 2],
    /// Whether the guidelines also run at 45°; off while ortho is on.
    pub diagonals: bool,
}

/// How far each way a guideline is taken to run, in millimetres: further than any model.
const GUIDE_REACH: f64 = 1e7;

impl Guides {
    /// Most points held at once.
    pub const MOST_HELD: usize = 8;

    /// Each guideline, as a point on it and its direction of length 1.
    #[must_use]
    pub fn lines(&self) -> Vec<(Point, Point)> {
        let [x, y] = self.axes;
        let mut directions = vec![x, y];
        if self.diagonals {
            let half = std::f64::consts::FRAC_1_SQRT_2;
            directions.push(std::array::from_fn(|i| (x[i] + y[i]) * half));
            directions.push(std::array::from_fn(|i| (x[i] - y[i]) * half));
        }
        self.held
            .iter()
            .flat_map(|p| directions.iter().map(move |d| (*p, *d)))
            .collect()
    }

    /// Holds `point`, or lets it go if it is held already. The oldest goes past the most held.
    pub fn toggle(&mut self, point: Point) {
        if let Some(index) = self.held.iter().position(|p| length(sub(*p, point)) < 1e-9) {
            self.held.remove(index);
            return;
        }
        if self.held.len() >= Self::MOST_HELD {
            self.held.remove(0);
        }
        self.held.push(point);
    }
}

impl Outlines {
    /// The snap the held points' guidelines give near the cursor, if one is in reach: where two
    /// guidelines cross, where a guideline crosses an object (both shown as intersections), and
    /// midway between two held points.
    #[must_use]
    pub fn guide_snap(
        &self,
        camera: &Camera,
        cursor: [f64; 2],
        size: [f64; 2],
        guides: &Guides,
    ) -> Option<SnapHit> {
        if guides.held.is_empty() {
            return None;
        }
        let view = camera.projector(size);
        let gap = |point: Point| {
            view.project(point)
                .map(|at| (at[0] - cursor[0]).hypot(at[1] - cursor[1]))
        };
        let mut candidates: Vec<(SnapKind, Point)> = Vec::new();
        let span = |(p, d): (Point, Point)| {
            (
                std::array::from_fn(|i| p[i] - d[i] * GUIDE_REACH),
                std::array::from_fn(|i| p[i] + d[i] * GUIDE_REACH),
            )
        };
        let lines = guides.lines();
        // Two lines meet where their closest points coincide.
        let meeting = |a: (Point, Point), b: (Point, Point)| -> Option<Point> {
            let (s, t) = closest_between(a, b);
            let (p, q) = (along(a.0, a.1, s), along(b.0, b.1, t));
            (length(sub(p, q)) < 1e-6 * (1.0 + length(p))).then_some(p)
        };
        for (i, first) in lines.iter().enumerate() {
            for second in &lines[i + 1..] {
                if length(sub(first.0, second.0)) < 1e-9 {
                    continue; // Lines from the same held point meet only there.
                }
                candidates.extend(
                    meeting(span(*first), span(*second)).map(|p| (SnapKind::Intersection, p)),
                );
            }
        }
        for segment in self.segments_near(&view, cursor, &[]) {
            for line in &lines {
                candidates.extend(
                    meeting(span(*line), (segment.a, segment.b))
                        .map(|p| (SnapKind::Intersection, p)),
                );
            }
        }
        for (i, a) in guides.held.iter().enumerate() {
            for b in &guides.held[i + 1..] {
                candidates.push((SnapKind::Mid, along(*a, *b, 0.5)));
            }
        }
        candidates
            .into_iter()
            .filter_map(|(kind, point)| Some((gap(point)?, kind, point)))
            .filter(|(distance, ..)| *distance <= SNAP_REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, kind, point)| SnapHit { kind, point })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::Document;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn outlines(curves: Vec<Curve>) -> Outlines {
        let mut doc = Document::default();
        for curve in curves {
            doc.add(curve);
        }
        Outlines::of(doc.state())
    }

    fn line(a: Point, b: Point) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn only(kind: SnapKind) -> SnapSet {
        SnapSet {
            end: kind == SnapKind::End,
            mid: kind == SnapKind::Mid,
            center: kind == SnapKind::Center,
            intersection: kind == SnapKind::Intersection,
            perpendicular: kind == SnapKind::Perpendicular,
            near: kind == SnapKind::Near,
            visible_only: false,
        }
    }

    fn close(a: Point, b: Point) -> bool {
        length(sub(a, b)) < 1e-6
    }

    /// The snap found with the cursor a few pixels off a model point.
    fn snap_by(
        shapes: &Outlines,
        aim: Point,
        set: SnapSet,
        from: Option<Point>,
    ) -> Option<SnapHit> {
        let camera = Camera::default();
        let at = camera.project(aim, SIZE).unwrap();
        shapes.snap(&camera, [at[0] + 4.0, at[1] - 3.0], SIZE, &set, from)
    }

    #[test]
    fn end_finds_the_exact_end_of_a_line() {
        let shapes = outlines(vec![line([0.0; 3], [30.0, 0.0, 0.0])]);
        let hit = snap_by(&shapes, [30.0, 0.0, 0.0], only(SnapKind::End), None).unwrap();
        assert_eq!(hit.kind, SnapKind::End);
        assert_eq!(hit.point, [30.0, 0.0, 0.0]);
    }

    #[test]
    fn end_finds_a_corner_above_the_ground() {
        let corner = [10.0, 10.0, 25.0];
        let polyline = Curve::polyline(vec![[0.0; 3], corner, [20.0, 0.0, 0.0]], false).unwrap();
        let hit = snap_by(&outlines(vec![polyline]), corner, only(SnapKind::End), None).unwrap();
        assert_eq!(hit.point, corner);
    }

    #[test]
    fn mid_finds_the_middle_of_each_segment() {
        let polyline =
            Curve::polyline(vec![[0.0; 3], [20.0, 0.0, 0.0], [20.0, 40.0, 0.0]], false).unwrap();
        let shapes = outlines(vec![polyline]);
        let first = snap_by(&shapes, [10.0, 0.0, 0.0], only(SnapKind::Mid), None).unwrap();
        let second = snap_by(&shapes, [20.0, 20.0, 0.0], only(SnapKind::Mid), None).unwrap();
        assert_eq!(first.point, [10.0, 0.0, 0.0]);
        assert_eq!(second.point, [20.0, 20.0, 0.0]);
    }

    #[test]
    fn mid_of_a_smooth_curve_is_half_way_along_it() {
        let points = vec![[0.0; 3], [20.0, 0.0, 0.0], [40.0, 0.0, 0.0]];
        let curve = Curve::interpolated(points, false).unwrap();
        let hit = snap_by(
            &outlines(vec![curve]),
            [20.0, 0.0, 0.0],
            only(SnapKind::Mid),
            None,
        )
        .unwrap();
        assert!(close(hit.point, [20.0, 0.0, 0.0]), "{:?}", hit.point);
    }

    #[test]
    fn center_finds_the_centre_of_a_circle() {
        let centre = [15.0, -5.0, 0.0];
        let circle = Curve::circle(centre, 8.0).unwrap();
        let hit = snap_by(
            &outlines(vec![circle]),
            centre,
            only(SnapKind::Center),
            None,
        )
        .unwrap();
        assert_eq!(hit.point, centre);
    }

    fn quarter_arc() -> Outlines {
        let x = [1.0, 0.0, 0.0];
        let y = [0.0, 1.0, 0.0];
        outlines(vec![
            Curve::arc([0.0; 3], [20.0, 0.0, 0.0], [0.0, 20.0, 0.0], x, y).unwrap(),
        ])
    }

    #[test]
    fn an_arc_can_be_snapped_to_at_its_end_centre_and_middle() {
        let shapes = quarter_arc();
        let end = snap_by(&shapes, [0.0, 20.0, 0.0], only(SnapKind::End), None).unwrap();
        assert!(close(end.point, [0.0, 20.0, 0.0]), "{:?}", end.point);
        let centre = snap_by(&shapes, [0.0; 3], only(SnapKind::Center), None).unwrap();
        assert_eq!(centre.point, [0.0; 3]);
        let half = 20.0 * std::f64::consts::FRAC_PI_4.cos();
        let middle = snap_by(&shapes, [half, half, 0.0], only(SnapKind::Mid), None).unwrap();
        assert!(
            length(sub(middle.point, [half, half, 0.0])) < 1e-3,
            "{:?}",
            middle.point
        );
    }

    #[test]
    fn a_nurbs_curve_can_be_snapped_to_at_its_ends_and_middle() {
        let curve = Curve::nurbs(
            1,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![[0.0; 3], [40.0, 0.0, 0.0]],
            vec![],
        )
        .unwrap();
        let shapes = outlines(vec![curve]);
        let end = snap_by(&shapes, [40.0, 0.0, 0.0], only(SnapKind::End), None).unwrap();
        assert!(close(end.point, [40.0, 0.0, 0.0]), "{:?}", end.point);
        let middle = snap_by(&shapes, [20.0, 0.0, 0.0], only(SnapKind::Mid), None).unwrap();
        assert!(close(middle.point, [20.0, 0.0, 0.0]), "{:?}", middle.point);
    }

    #[test]
    fn a_joined_curve_can_be_snapped_to_at_its_joints_and_parts() {
        let line = line([0.0; 3], [30.0, 0.0, 0.0]);
        let arc = Curve::arc(
            [30.0, 20.0, 0.0],
            [30.0, 0.0, 0.0],
            [50.0, 20.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let shapes = outlines(vec![Curve::compound(vec![line, arc]).unwrap()]);
        let joint = snap_by(&shapes, [30.0, 0.0, 0.0], only(SnapKind::End), None).unwrap();
        assert!(close(joint.point, [30.0, 0.0, 0.0]), "{:?}", joint.point);
        let centre = snap_by(&shapes, [30.0, 20.0, 0.0], only(SnapKind::Center), None).unwrap();
        assert_eq!(centre.point, [30.0, 20.0, 0.0]);
    }
    #[test]
    fn intersection_finds_where_two_lines_cross() {
        let shapes = outlines(vec![
            line([-20.0, 7.0, 0.0], [20.0, 7.0, 0.0]),
            line([3.0, -20.0, 0.0], [3.0, 20.0, 0.0]),
        ]);
        let hit = snap_by(&shapes, [3.0, 7.0, 0.0], only(SnapKind::Intersection), None).unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(close(hit.point, [3.0, 7.0, 0.0]), "{:?}", hit.point);
    }

    #[test]
    fn lines_that_only_look_crossed_do_not_intersect() {
        let shapes = outlines(vec![
            line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]),
            line([0.0, -20.0, 30.0], [0.0, 20.0, 30.0]),
        ]);
        let camera = Camera {
            pitch: 1.5,
            ..Camera::default()
        };
        let at = camera.project([0.0; 3], SIZE).unwrap();
        let set = only(SnapKind::Intersection);
        assert_eq!(shapes.snap(&camera, at, SIZE, &set, None), None);
    }

    #[test]
    fn perpendicular_finds_the_foot_of_a_right_angle() {
        let shapes = outlines(vec![line([0.0; 3], [40.0, 0.0, 0.0])]);
        let set = only(SnapKind::Perpendicular);
        let hit = snap_by(&shapes, [12.0, 0.0, 0.0], set, Some([12.0, 30.0, 0.0])).unwrap();
        assert_eq!(hit.kind, SnapKind::Perpendicular);
        assert!(close(hit.point, [12.0, 0.0, 0.0]), "{:?}", hit.point);
    }

    #[test]
    fn perpendicular_needs_a_last_point() {
        let shapes = outlines(vec![line([0.0; 3], [40.0, 0.0, 0.0])]);
        let set = only(SnapKind::Perpendicular);
        assert_eq!(snap_by(&shapes, [12.0, 0.0, 0.0], set, None), None);
    }

    #[test]
    fn near_lands_on_the_curve_under_the_cursor() {
        let shapes = outlines(vec![line([0.0; 3], [40.0, 0.0, 0.0])]);
        let camera = Camera::default();
        let aim = [13.0, 0.0, 0.0];
        let at = camera.project(aim, SIZE).unwrap();
        let hit = shapes
            .snap(&camera, at, SIZE, &only(SnapKind::Near), None)
            .unwrap();
        assert_eq!(hit.kind, SnapKind::Near);
        assert!(length(sub(hit.point, aim)) < 1e-3, "{:?}", hit.point);
    }

    #[test]
    fn an_exact_snap_beats_near() {
        let shapes = outlines(vec![line([0.0; 3], [40.0, 0.0, 0.0])]);
        let set = SnapSet {
            near: true,
            ..SnapSet::default()
        };
        let hit = snap_by(&shapes, [40.0, 0.0, 0.0], set, None).unwrap();
        assert_eq!(hit.kind, SnapKind::End);
    }

    #[test]
    fn the_closest_exact_snap_wins() {
        let shapes = outlines(vec![line([0.0; 3], [2.0, 0.0, 0.0])]);
        let camera = Camera::default();
        let end = camera.project([2.0, 0.0, 0.0], SIZE).unwrap();
        let hit = shapes
            .snap(&camera, end, SIZE, &SnapSet::default(), None)
            .unwrap();
        assert_eq!(hit.point, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn a_snap_that_is_off_is_ignored() {
        let shapes = outlines(vec![line([0.0; 3], [30.0, 0.0, 0.0])]);
        assert_eq!(
            snap_by(&shapes, [30.0, 0.0, 0.0], only(SnapKind::Mid), None),
            None
        );
    }

    #[test]
    fn nothing_in_reach_means_no_snap() {
        let shapes = outlines(vec![line([0.0; 3], [30.0, 0.0, 0.0])]);
        assert_eq!(
            snap_by(&shapes, [70.0, 60.0, 0.0], SnapSet::default(), None),
            None
        );
    }

    /// A snap found while a curve through `drawing` is still being drawn, in an empty model.
    fn snap_to_self(aim: Point, set: SnapSet, drawing: &[Point]) -> Option<SnapHit> {
        let camera = Camera::default();
        let at = camera.project(aim, SIZE).unwrap();
        Outlines::default().snap_while_drawing(
            &camera,
            [at[0] + 4.0, at[1] - 3.0],
            SIZE,
            &set,
            drawing.last().copied(),
            drawing,
        )
    }

    const IN_PROGRESS: [Point; 3] = [[0.0; 3], [40.0, 0.0, 0.0], [40.0, 30.0, 0.0]];

    #[test]
    fn a_curve_can_snap_to_its_own_first_point() {
        let hit = snap_to_self(IN_PROGRESS[0], only(SnapKind::End), &IN_PROGRESS).unwrap();
        assert_eq!(hit.kind, SnapKind::End);
        assert_eq!(hit.point, IN_PROGRESS[0]);
    }

    #[test]
    fn a_curve_can_snap_to_the_middle_of_its_own_segment() {
        let hit = snap_to_self([20.0, 0.0, 0.0], only(SnapKind::Mid), &IN_PROGRESS).unwrap();
        assert_eq!(hit.point, [20.0, 0.0, 0.0]);
    }

    #[test]
    fn a_curve_can_snap_near_itself() {
        let hit = snap_to_self([13.0, 0.0, 0.0], only(SnapKind::Near), &IN_PROGRESS).unwrap();
        assert_eq!(hit.kind, SnapKind::Near);
        assert!(hit.point[1].abs() < 1e-6);
    }

    #[test]
    fn a_curve_can_snap_perpendicular_to_itself() {
        let hit = snap_to_self(
            [40.0, 0.0, 0.0],
            only(SnapKind::Perpendicular),
            &[[0.0; 3], [80.0, 0.0, 0.0], [40.0, 30.0, 0.0]],
        )
        .unwrap();
        assert!(close(hit.point, [40.0, 0.0, 0.0]), "{:?}", hit.point);
    }

    #[test]
    fn the_newest_point_is_not_snapped_to() {
        assert_eq!(
            snap_to_self(IN_PROGRESS[2], only(SnapKind::End), &IN_PROGRESS),
            None
        );
    }

    #[test]
    fn a_curve_with_only_one_point_has_nothing_to_snap_to() {
        assert_eq!(
            snap_to_self([0.0; 3], SnapSet::default(), &[[0.0; 3]]),
            None
        );
    }

    #[test]
    fn near_is_off_at_the_start() {
        assert!(!SnapSet::default().near);
        assert!(SnapSet::default().end);
    }

    #[test]
    fn ortho_keeps_to_the_axis_moved_furthest() {
        let base = Some([5.0, 5.0, 2.0]);
        assert_eq!(
            constrained(&CPlane::WORLD_XY, [20.0, 8.0, 0.0], base, true, false),
            [20.0, 5.0, 2.0]
        );
        assert_eq!(
            constrained(&CPlane::WORLD_XY, [7.0, -30.0, 0.0], base, true, false),
            [5.0, -30.0, 2.0]
        );
    }

    #[test]
    fn ortho_needs_a_last_point() {
        let ground = [20.0, 8.0, 0.0];
        assert_eq!(
            constrained(&CPlane::WORLD_XY, ground, None, true, false),
            ground
        );
    }

    #[test]
    fn grid_snap_rounds_to_whole_units() {
        assert_eq!(
            constrained(&CPlane::WORLD_XY, [1.4, -2.6, 0.0], None, false, true),
            [1.0, -3.0, 0.0]
        );
    }

    #[test]
    fn grid_snap_applies_along_the_ortho_axis() {
        let base = Some([0.5, 0.5, 0.0]);
        assert_eq!(
            constrained(&CPlane::WORLD_XY, [9.7, 1.0, 0.0], base, true, true),
            [10.0, 0.5, 0.0]
        );
    }

    #[test]
    fn grid_snap_rounds_within_the_front_plane() {
        assert_eq!(
            constrained(&CPlane::WORLD_XZ, [1.4, 0.0, 7.6], None, false, true),
            [1.0, 0.0, 8.0]
        );
    }

    #[test]
    fn ortho_goes_straight_up_in_the_front_plane() {
        let base = Some([5.0, 0.0, 5.0]);
        assert_eq!(
            constrained(&CPlane::WORLD_XZ, [7.0, 0.0, 30.0], base, true, false),
            [5.0, 0.0, 30.0]
        );
    }

    #[test]
    fn ortho_follows_the_right_plane() {
        let base = Some([0.0, 2.0, 2.0]);
        assert_eq!(
            constrained(&CPlane::WORLD_YZ, [0.0, 40.0, 3.0], base, true, false),
            [0.0, 40.0, 2.0]
        );
    }
    #[test]
    fn with_nothing_on_the_point_is_unchanged() {
        let ground = [1.4, -2.6, 0.0];
        assert_eq!(
            constrained(&CPlane::WORLD_XY, ground, Some([0.0; 3]), false, false),
            ground
        );
    }

    #[test]
    fn held_points_give_crossings_midpoints_and_hits_on_objects() {
        let shapes = outlines(vec![line([20.0, -5.0, 0.0], [20.0, 5.0, 0.0])]);
        let mut guides = Guides {
            held: Vec::new(),
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            diagonals: true,
        };
        guides.toggle([0.0; 3]);
        guides.toggle([10.0, 10.0, 0.0]);
        let camera = Camera::default();
        let near = |p: Point| {
            let at = camera.project(p, SIZE).unwrap();
            shapes.guide_snap(&camera, [at[0] + 3.0, at[1] - 2.0], SIZE, &guides)
        };
        let crossing = near([10.0, 0.0, 0.0]).unwrap();
        assert_eq!(crossing.kind, SnapKind::Intersection);
        assert!(close(crossing.point, [10.0, 0.0, 0.0]));
        let middle = near([5.0, 5.0, 0.0]).unwrap();
        assert!(close(middle.point, [5.0, 5.0, 0.0]));
        let on_line = near([20.0, 0.0, 0.0]).unwrap();
        assert!(close(on_line.point, [20.0, 0.0, 0.0]));
        // Resting on a held point again lets it go.
        guides.toggle([10.0, 10.0, 0.0]);
        assert_eq!(guides.held.len(), 1);
    }
}
