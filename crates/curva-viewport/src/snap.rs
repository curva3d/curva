//! Object snaps, ortho and grid snap: turning a cursor position into an exact point.

use crate::camera::{CPlane, Projector};
use crate::index::BoxTree;
use crate::{Camera, Outlines};

mod guides;
mod tangent;
use curva_kernel::{Curve, Point, closest_between};
pub use guides::Guides;
use std::f64::consts::TAU;

/// How close to something the cursor must be to snap to it, in pixels.
const SNAP_REACH: f64 = 12.0;
/// How much farther than the nearest place found a segment may lie on screen, in pixels, and
/// still be searched for meeting points and perpendicular feet: a meeting point lies on its
/// second segment only to within `MEETING_TOLERANCE`, which may show as a fraction of a pixel.
const SEGMENT_MARGIN: f64 = 1.0;
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
    /// Where a line from the last point touches a curve, as Rhino's Tan snap.
    Tangent,
    /// The closest point on a curve.
    Near,
    /// A point object, as Rhino's Point snap.
    Point,
    /// A quarter of the way round a circle or arc along its own axes, as Rhino's Quad snap.
    Quad,
    /// A point along a guideline from a held point, away from any crossing, as Rhino's
    /// SmartTrack runs the pointer along its tracking line (#512, #547). Shown by the guideline
    /// itself, with no name.
    Track,
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
    /// Lines from the last point touching a curve.
    pub tangent: bool,
    /// The closest point on a curve.
    pub near: bool,
    /// Point objects.
    pub point: bool,
    /// Quarter points of circles and arcs.
    pub quad: bool,
    /// All object snaps at once, as Rhino's Osnap switch: off leaves the kinds as they are but
    /// uses none of them.
    pub on: bool,
    /// Guidelines from held points, as Rhino's SmartTrack switch.
    pub track: bool,
    /// Snapped points moved straight onto the construction plane, as Rhino's Project. Applied by
    /// the app, which knows the plane.
    pub project: bool,
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
            tangent: false,
            near: false,
            point: true,
            quad: false,
            on: true,
            track: true,
            project: false,
            visible_only: false,
        }
    }
}

impl SnapSet {
    fn has(&self, kind: SnapKind) -> bool {
        let wanted = match kind {
            SnapKind::End => self.end,
            SnapKind::Mid => self.mid,
            SnapKind::Center => self.center,
            SnapKind::Intersection => self.intersection,
            SnapKind::Perpendicular => self.perpendicular,
            SnapKind::Tangent => self.tangent,
            SnapKind::Near => self.near,
            SnapKind::Point => self.point,
            SnapKind::Quad => self.quad,
            // Guidelines have their own switch, apart from the object snaps.
            SnapKind::Track => return self.track,
        };
        self.on && wanted
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

/// The quarter points of a circle or arc turning `sweep` from `x` towards `y`, short of a full
/// turn, measured along its own axes.
fn quarters(centre: Point, radius: f64, x: Point, y: Point, sweep: f64) -> Vec<Site> {
    (0..4)
        .map(|quarter| f64::from(quarter) * std::f64::consts::FRAC_PI_2)
        // An arc's own ends are End snaps already.
        .filter(|angle| *angle < sweep - 1e-9 && (*angle > 0.0 || sweep >= TAU - 1e-9))
        .map(|angle| {
            let (sin, cos) = angle.sin_cos();
            let at =
                std::array::from_fn(|axis| centre[axis] + radius * (cos * x[axis] + sin * y[axis]));
            site(SnapKind::Quad, at)
        })
        .collect()
}

/// The fixed snap places on a curve, given the points it is drawn through.
pub(crate) fn sites_of(curve: &Curve, points: &[Point]) -> Vec<Site> {
    match curve {
        Curve::Point { at } => vec![site(SnapKind::Point, *at)],
        Curve::Line { .. } | Curve::Polyline { .. } => points
            .iter()
            .map(|p| site(SnapKind::End, *p))
            .chain(
                points
                    .windows(2)
                    .map(|p| site(SnapKind::Mid, along(p[0], p[1], 0.5))),
            )
            .collect(),
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => std::iter::once(site(SnapKind::Center, *centre))
            .chain(quarters(*centre, *radius, *x, *y, TAU))
            .collect(),
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } => points
            .first()
            .into_iter()
            .chain(points.last())
            .map(|p| site(SnapKind::End, *p))
            .chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
            .chain(std::iter::once(site(SnapKind::Center, *centre)))
            .chain(quarters(*centre, *radius, *x, *y, *sweep))
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

/// Adds the segments of a run of points that are within `reach` pixels of the cursor on screen.
fn collect_near(
    found: &mut Vec<NearbySegment>,
    view: &Projector,
    (cursor, reach): ([f64; 2], f64),
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
            if gap <= reach {
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

/// How far `point` shows from the cursor on screen; nothing when it is behind the camera.
fn point_gap(view: &Projector, cursor: [f64; 2], point: Point) -> Option<f64> {
    view.project(point)
        .map(|at| (at[0] - cursor[0]).hypot(at[1] - cursor[1]))
}

/// No more than the distance on screen from the cursor to any point in a box: the distance to the
/// rectangle around its projected corners, which holds the whole projected box; nothing for a box
/// partly behind the camera. A hair is taken off, so rounding never puts it past a point inside.
fn box_gap(view: &Projector, (low, high): (Point, Point), cursor: [f64; 2]) -> f64 {
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
            return 0.0;
        };
        left = left.min(at[0]);
        right = right.max(at[0]);
        top = top.min(at[1]);
        bottom = bottom.max(at[1]);
    }
    let across = (left - cursor[0]).max(cursor[0] - right).max(0.0);
    let down = (top - cursor[1]).max(cursor[1] - bottom).max(0.0);
    (across.hypot(down) - 1e-9).max(0.0)
}

/// Whether a whole box lies more than `reach` pixels from the cursor on screen; never for a box
/// partly behind the camera.
pub(crate) fn box_beyond(
    view: &Projector,
    (low, high): (Point, Point),
    (cursor, reach): ([f64; 2], f64),
) -> bool {
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
    cursor[0] < left - reach
        || cursor[0] > right + reach
        || cursor[1] < top - reach
        || cursor[1] > bottom + reach
}

/// The objects' boxes and the snap places, each in a tree of boxes (#130), so picking and
/// snapping look only at what is near the cursor. Each tree is made once per model: by
/// [`Outlines::prepare_snapping`] away from the frame, or else the first time it is asked for.
/// Picking needs only the objects' tree, much smaller than the places'. Copies of the outlines
/// share the trees, and a question asked while a tree is being made waits for it.
#[derive(Clone, Debug, Default)]
pub(crate) struct SnapIndex {
    /// One item per entry of `Outlines::objects`, by its box.
    objects: std::sync::Arc<std::sync::OnceLock<BoxTree>>,
    /// One item per entry of `Outlines::sites`, by its point.
    sites: std::sync::Arc<std::sync::OnceLock<BoxTree>>,
}

/// Outlines compare by what they hold; the index is only a quicker way to search it.
impl PartialEq for SnapIndex {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Outlines {
    /// Starts making the trees picking and snapping search, on a thread of their own, so that the
    /// first snap after a change does not hold up a frame while they are made (half a second for
    /// a scene of 60,000 objects). Snapping before they are ready waits for them.
    pub fn prepare_snapping(&self) {
        let index = self.snap_index.clone();
        let boxes = self.object_boxes();
        let points = self.site_points();
        std::thread::spawn(move || {
            index.objects.get_or_init(|| BoxTree::new(boxes));
            index.sites.get_or_init(|| BoxTree::new(points));
        });
    }

    /// Each object's box, in order; an object without one is never skipped.
    fn object_boxes(&self) -> Vec<(Point, Point)> {
        let everywhere = ([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]);
        (0..self.objects.len())
            .map(|object| self.boxes.get(object).copied().unwrap_or(everywhere))
            .collect()
    }

    /// Each snap place as a box around its point, in order.
    fn site_points(&self) -> Vec<(Point, Point)> {
        self.sites
            .iter()
            .map(|site| (site.point, site.point))
            .collect()
    }

    fn object_tree(&self) -> &BoxTree {
        self.snap_index
            .objects
            .get_or_init(|| BoxTree::new(self.object_boxes()))
    }

    fn site_tree(&self) -> &BoxTree {
        self.snap_index
            .sites
            .get_or_init(|| BoxTree::new(self.site_points()))
    }

    /// The entries of `objects` whose boxes come within `reach` pixels of the cursor, in their own
    /// order; the rest cannot be picked or snapped to there.
    pub(crate) fn objects_near(
        &self,
        view: &Projector,
        (cursor, reach): ([f64; 2], f64),
    ) -> Vec<usize> {
        let mut near = self
            .object_tree()
            .query(|bounds| !box_beyond(view, bounds, (cursor, reach)));
        near.sort_unstable();
        near
    }

    /// The segments within `reach` pixels of the cursor, of the objects and of `drawing`.
    fn segments_near(
        &self,
        view: &Projector,
        (cursor, reach): ([f64; 2], f64),
        drawing: &[Point],
    ) -> Vec<NearbySegment> {
        let mut found = Vec::new();
        // A box inside a box out of reach is out of reach too (`box_beyond` tests the box's
        // projected outline), so the tree skips only what testing each object would skip.
        let near = self
            .object_tree()
            .query(|bounds| !box_beyond(view, bounds, (cursor, reach)));
        for object in near {
            if let Some((_, points)) = self.objects.get(object) {
                collect_near(&mut found, view, (cursor, reach), object, points);
            }
        }
        collect_near(&mut found, view, (cursor, reach), usize::MAX, drawing);
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
        let gap = |point: Point| point_gap(&view, cursor, point);
        let offer = |offered: &mut Vec<(f64, SnapHit)>, kind: SnapKind, point: Point| {
            if let Some(distance) = gap(point)
                && distance <= SNAP_REACH
            {
                offered.push((distance, SnapHit { kind, point }));
            }
        };
        // In a shaded view, a point behind a face cannot be seen and is not offered.
        let seen = |point: Point| !set.visible_only || !self.hidden(camera, size, point);
        // The nearest place seen so far (#130). A meeting point lies on both its segments and a
        // perpendicular foot on its own, so only segments nearer the cursor than that place can give
        // a nearer point: with the whole of a large scene in view, that skips almost all of them.
        let mut best = self.nearest_place(&view, cursor, set, drawing, &seen);
        let reach = best
            .as_ref()
            .map_or(SNAP_REACH, |(distance, _)| distance + SEGMENT_MARGIN);
        let segments = if set.intersection || set.perpendicular || set.tangent || set.near {
            self.segments_near(&view, (cursor, reach), drawing)
        } else {
            Vec::new()
        };
        let mut more: Vec<(f64, SnapHit)> = Vec::new();
        if set.intersection {
            for point in meeting_points(&segments) {
                offer(&mut more, SnapKind::Intersection, point);
            }
        }
        if set.perpendicular
            && let Some(from) = from
        {
            for foot in perpendicular_feet(&segments, from) {
                offer(&mut more, SnapKind::Perpendicular, foot);
            }
        }
        if set.tangent
            && let Some(from) = from
        {
            for point in tangent::tangent_points(&segments, from) {
                offer(&mut more, SnapKind::Tangent, point);
            }
        }
        more.sort_by(|a, b| a.0.total_cmp(&b.0));
        // A place wins a tie, as when all were sorted together with places first.
        if let Some(nearer) = more.into_iter().find(|(_, hit)| seen(hit.point))
            && best
                .as_ref()
                .is_none_or(|(distance, _)| nearer.0 < *distance)
        {
            best = Some(nearer);
        }
        if let Some((_, hit)) = best {
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

    /// The nearest place that `seen` lets through, an end, middle or centre of an object or a
    /// point of the curve being drawn, with its distance from the cursor on screen. The tree of
    /// places is searched outwards from the cursor and stops there (#130): with a whole large
    /// scene in view, thousands of places are within reach, but only those nearer than the answer
    /// are looked at. Between places equally near, the first in the model wins, then the object's
    /// over the drawing's, as when all were sorted together.
    fn nearest_place(
        &self,
        view: &Projector,
        cursor: [f64; 2],
        set: &SnapSet,
        drawing: &[Point],
        seen: &dyn Fn(Point) -> bool,
    ) -> Option<(f64, SnapHit)> {
        let place = self
            .site_tree()
            .nearest(
                SNAP_REACH,
                |bounds| box_gap(view, bounds, cursor),
                |at| {
                    let place = &self.sites[at];
                    set.has(place.kind)
                        .then(|| point_gap(view, cursor, place.point))
                        .flatten()
                },
                |at| seen(self.sites[at].point),
            )
            .map(|(at, distance)| {
                let place = &self.sites[at];
                let (kind, point) = (place.kind, place.point);
                (distance, SnapHit { kind, point })
            });
        let ends = drawing[..drawing.len().saturating_sub(1)]
            .iter()
            .filter(|_| set.end)
            .map(|&point| (SnapKind::End, point));
        let middles = drawing
            .windows(2)
            .filter(|_| set.mid)
            .map(|pair| (SnapKind::Mid, along(pair[0], pair[1], 0.5)));
        let mut offered: Vec<(f64, SnapHit)> = ends
            .chain(middles)
            .filter_map(|(kind, point)| {
                let distance = point_gap(view, cursor, point).filter(|d| *d <= SNAP_REACH)?;
                Some((distance, SnapHit { kind, point }))
            })
            .collect();
        offered.sort_by(|a, b| a.0.total_cmp(&b.0));
        let drawn = offered.into_iter().find(|(_, hit)| seen(hit.point));
        match (place, drawn) {
            (Some(place), Some(drawn)) if drawn.0 < place.0 => Some(drawn),
            (place, drawn) => place.or(drawn),
        }
    }
}

/// Where a line from `from` meets each segment at a right angle, in the segments' order, where
/// that is inside the segment and not `from` itself.
fn perpendicular_feet(segments: &[NearbySegment], from: Point) -> Vec<Point> {
    let mut feet = Vec::new();
    for segment in segments {
        let direction = sub(segment.b, segment.a);
        let square = dot(direction, direction);
        let t = dot(sub(from, segment.a), direction) / square.max(1e-12);
        let foot = along(segment.a, segment.b, t);
        // At the last point itself there is no right angle to draw.
        if t > 0.0 && t < 1.0 && length(sub(foot, from)) > 1e-9 {
            feet.push(foot);
        }
    }
    feet
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
            tangent: kind == SnapKind::Tangent,
            near: kind == SnapKind::Near,
            point: kind == SnapKind::Point,
            quad: kind == SnapKind::Quad,
            on: true,
            track: true,
            project: false,
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

    /// The snap as found before #130's shortcut: every place, meeting point and perpendicular
    /// foot within reach, from every segment within reach, sorted together, places first.
    fn snap_searching_everything(
        shapes: &Outlines,
        camera: &Camera,
        cursor: [f64; 2],
        set: &SnapSet,
        from: Option<Point>,
    ) -> Option<SnapHit> {
        let view = camera.projector(SIZE);
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
        for place in shapes.sites.iter().filter(|place| set.has(place.kind)) {
            offer(place.kind, place.point);
        }
        let segments = shapes.segments_near(&view, (cursor, SNAP_REACH), &[]);
        if set.intersection {
            for point in meeting_points(&segments) {
                offer(SnapKind::Intersection, point);
            }
        }
        if set.perpendicular
            && let Some(from) = from
        {
            for foot in perpendicular_feet(&segments, from) {
                offer(SnapKind::Perpendicular, foot);
            }
        }
        if set.tangent
            && let Some(from) = from
        {
            for point in tangent::tangent_points(&segments, from) {
                offer(SnapKind::Tangent, point);
            }
        }
        offered.sort_by(|a, b| a.0.total_cmp(&b.0));
        offered.into_iter().next().map(|(_, hit)| hit)
    }

    /// #130: searching only the segments nearer the cursor than the nearest place found gives
    /// the snap searching everything gave, all over a dense tangle of lines and circles, with
    /// every snap on but Near.
    #[test]
    fn the_shortcut_snaps_as_searching_everything() {
        let mut curves = Vec::new();
        for i in 0..12 {
            let t = f64::from(i);
            curves.push(line(
                [-40.0 + 3.0 * t, -30.0, 0.0],
                [35.0 - 2.0 * t, 30.0, 0.0],
            ));
            curves.push(line(
                [-40.0, -25.0 + 4.0 * t, 0.0],
                [40.0, 20.0 - 3.5 * t, 0.0],
            ));
            curves.push(Curve::circle([-20.0 + 4.0 * t, 5.0 - t, 0.0], 3.0 + t).unwrap());
        }
        let shapes = outlines(curves);
        let camera = Camera::default();
        let set = SnapSet {
            near: false,
            visible_only: false,
            ..SnapSet::default()
        };
        let mut compared = 0;
        for x in (0..800).step_by(11) {
            for y in (0..600).step_by(9) {
                let cursor = [f64::from(x), f64::from(y)];
                let from = Some([1.0, 2.0, 0.0]);
                let quick = shapes.snap(&camera, cursor, SIZE, &set, from);
                let everything = snap_searching_everything(&shapes, &camera, cursor, &set, from);
                assert_eq!(quick, everything, "at {cursor:?}");
                compared += usize::from(quick.is_some());
            }
        }
        assert!(compared > 100, "only {compared} snaps found");
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
    fn quad_finds_the_quarter_points_of_a_circle() {
        let circle = Curve::circle([0.0; 3], 10.0).unwrap();
        let shapes = outlines(vec![circle]);
        let hit = snap_by(&shapes, [0.0, 10.0, 0.0], only(SnapKind::Quad), None).unwrap();
        assert_eq!(hit.kind, SnapKind::Quad);
        assert!(close(hit.point, [0.0, 10.0, 0.0]));
    }

    #[test]
    fn point_finds_a_point_object() {
        let shapes = outlines(vec![Curve::Point {
            at: [5.0, 5.0, 0.0],
        }]);
        let hit = snap_by(&shapes, [5.0, 5.0, 0.0], only(SnapKind::Point), None).unwrap();
        assert_eq!(hit.kind, SnapKind::Point);
    }

    #[test]
    fn osnap_off_uses_no_object_snaps() {
        let shapes = outlines(vec![line([0.0; 3], [30.0, 0.0, 0.0])]);
        let set = SnapSet {
            on: false,
            ..only(SnapKind::End)
        };
        assert_eq!(snap_by(&shapes, [30.0, 0.0, 0.0], set, None), None);
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
    fn tan_finds_where_a_line_from_the_last_point_touches_a_circle() {
        let shapes = outlines(vec![Curve::circle([0.0; 3], 10.0).unwrap()]);
        let set = only(SnapKind::Tangent);
        let touch = [5.0, 75.0_f64.sqrt(), 0.0];
        let hit = snap_by(&shapes, touch, set, Some([20.0, 0.0, 0.0])).unwrap();
        assert_eq!(hit.kind, SnapKind::Tangent);
        assert!(close(hit.point, touch), "{:?}", hit.point);
        assert_eq!(snap_by(&shapes, touch, set, None), None);
    }

    #[test]
    fn tan_finds_nothing_at_a_polyline_corner() {
        let corner =
            Curve::polyline(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false).unwrap();
        let shapes = outlines(vec![corner]);
        let hit = snap_by(
            &shapes,
            [10.0, 0.0, 0.0],
            only(SnapKind::Tangent),
            Some([20.0, 5.0, 0.0]),
        );
        assert_eq!(hit, None);
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
    fn the_pointer_runs_along_a_guideline_away_from_any_crossing() {
        let shapes = outlines(Vec::new());
        let mut guides = Guides {
            held: Vec::new(),
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            diagonals: true,
        };
        guides.toggle([0.0; 3]);
        let camera = Camera::default();
        // A few pixels off the horizontal guideline through the held point, far along it.
        let at = camera.project([7.0, 0.0, 0.0], SIZE).unwrap();
        let hit = shapes
            .guide_snap(&camera, [at[0], at[1] - 3.0], SIZE, &guides)
            .expect("snaps onto the guideline");
        assert_eq!(
            hit.kind,
            SnapKind::Track,
            "shown by the guideline, not as Near"
        );
        assert!(hit.point[1].abs() < 1e-6, "{:?}", hit.point);
        assert!((hit.point[0] - 7.0).abs() < 0.5, "{:?}", hit.point);
        // Far from every guideline, nothing.
        let away = camera.project([20.0, 8.0, 0.0], SIZE).unwrap();
        assert!(shapes.guide_snap(&camera, away, SIZE, &guides).is_none());
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

    #[test]
    fn curves_snap_in_every_display_mode() {
        let shapes = outlines(vec![
            line([0.0; 3], [40.0, 0.0, 0.0]),
            Curve::polyline(
                vec![[0.0, 20.0, 0.0], [20.0, 20.0, 0.0], [20.0, 40.0, 0.0]],
                false,
            )
            .unwrap(),
        ]);
        for visible_only in [false, true] {
            let set = SnapSet {
                visible_only,
                ..SnapSet::default()
            };
            for (aim, kind) in [
                ([40.0, 0.0, 0.0], SnapKind::End),
                ([20.0, 0.0, 0.0], SnapKind::Mid),
                ([20.0, 20.0, 0.0], SnapKind::End),
            ] {
                let hit = snap_by(&shapes, aim, set, None)
                    .unwrap_or_else(|| panic!("{aim:?} {visible_only}"));
                assert_eq!(hit.kind, kind, "{aim:?}");
                assert!(close(hit.point, aim), "{aim:?} {:?}", hit.point);
            }
        }
    }

    /// #130: the index skips a whole box only when it is out of reach, so every box inside it
    /// must be out of reach too, or a snap point would be lost; and a box's gap is no more than
    /// any point inside it is from the cursor. Checked over many boxes, views
    /// and cursor positions, flat and in perspective, including boxes reaching behind the camera.
    #[test]
    fn a_box_inside_one_out_of_reach_is_out_of_reach_too() {
        let mut seed = 99_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / f64::from(1u32 << 31)
        };
        let size = [800.0, 600.0];
        let mut checked = 0;
        for view in [
            crate::StandardView::Top,
            crate::StandardView::Front,
            crate::StandardView::Perspective,
        ] {
            let mut camera = view.camera();
            camera.frame([-50.0; 3], [50.0; 3]);
            let projector = camera.projector(size);
            for _ in 0..400 {
                let low = [0, 1, 2].map(|_| next() * 400.0 - 200.0);
                let outer = (low, [0, 1, 2].map(|axis| low[axis] + next() * 100.0));
                let corner =
                    |t: f64, axis: usize| outer.0[axis] + (outer.1[axis] - outer.0[axis]) * t;
                let (a, b) = (next(), next());
                let inner = (
                    [0, 1, 2].map(|axis| corner(a.min(b), axis)),
                    [0, 1, 2].map(|axis| corner(a.max(b), axis)),
                );
                let cursor = [next() * size[0], next() * size[1]];
                // No point inside a box is nearer on screen than its gap, which the nearest-first
                // search through the places relies on.
                let inside = [0, 1, 2].map(|axis| corner(next(), axis));
                if let Some(at) = projector.project(inside) {
                    let gap = (at[0] - cursor[0]).hypot(at[1] - cursor[1]);
                    assert!(box_gap(&projector, outer, cursor) <= gap);
                }
                if box_beyond(&projector, outer, (cursor, SNAP_REACH)) {
                    assert!(box_beyond(&projector, inner, (cursor, SNAP_REACH)));
                    checked += 1;
                }
            }
        }
        // Enough of the boxes were out of reach for the check to mean something.
        assert!(checked > 100, "only {checked} checked");
    }

    /// #130: snapping through the index on a busy model of 900 circles still finds the centre
    /// of whichever circle is aimed at, near and far corners alike.
    #[test]
    fn snapping_through_the_index_finds_the_circle_aimed_at() {
        let circles = (0..30)
            .flat_map(|row| (0..30).map(move |column| (row, column)))
            .map(|(row, column)| {
                let centre = [f64::from(column) * 12.0, f64::from(row) * 12.0, 0.0];
                Curve::circle(centre, 5.0).unwrap()
            })
            .collect();
        let outlines = outlines(circles);
        let mut camera = crate::StandardView::Top.camera();
        let (low, high) = outlines.bounds().unwrap();
        camera.frame(low, high);
        let size = [800.0, 600.0];
        let set = SnapSet {
            center: true,
            ..SnapSet::default()
        };
        // Aim at the centres of a spread of circles: each snaps to exactly that centre.
        for (row, column) in [(0, 0), (29, 29), (15, 3), (7, 22)] {
            let centre = [f64::from(column) * 12.0, f64::from(row) * 12.0, 0.0];
            let on_screen = camera.project(centre, size).unwrap();
            let aim = [on_screen[0] + 2.0, on_screen[1] - 2.0];
            let hit = outlines.snap(&camera, aim, size, &set, None).unwrap();
            assert_eq!(hit.kind, SnapKind::Center);
            assert!(close(hit.point, centre), "{centre:?} {:?}", hit.point);
        }
    }
}
