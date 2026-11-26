//! The gumball, as in Rhino (spec `docs/specs/gumball-rhino.md`): on-screen handles on the
//! selection's centre that move, extrude, turn and scale it. Along each axis: an arrow at the
//! tip to move, a circle midway to extrude (or stretch a solid along the axis); between
//! each pair of axes a quarter arc to turn and a plane square to move in that plane; and the
//! centre dot to move freely on the construction plane.

use crate::{CPlane, Camera};
use curva_kernel::{Curve, Point, Similarity, dot};

/// How long an arrow is on screen, in pixels.
const ARROW_PIXELS: f64 = 90.0;
/// The radius of the turning arcs on screen, in pixels.
const ARC_PIXELS: f64 = 60.0;
/// How close the cursor must be to a handle to take it, in pixels.
const HANDLE_REACH: f64 = 7.0;
/// Straight pieces in the drawing of each turning arc.
const ARC_STEPS: usize = 24;
/// An axis pointing nearly at the viewer is too short on screen to be dragged.
const SHORTEST_ON_SCREEN: f64 = 1e-3;
/// Where along an arrow its extrude circle sits.
const EXTRUDE_AT: f64 = 0.5;
/// How far out along both of its axes a plane square sits, as a part of an arrow's length.
const PLANE_AT: f64 = 0.32;
/// Half a plane square's side, as a part of an arrow's length.
const PLANE_HALF: f64 = 0.07;
/// Half a scale square's side, and the radius of the extrude circles and the centre, in pixels.
const MARK_PIXELS: f64 = 4.5;
/// An axis shorter on screen than this part of an arrow holds no handles.
const SHORTEST_SEEN: f64 = 0.25;
/// A plane whose normal is this close to across the view is seen edge on.
const PLANE_EDGE_ON: f64 = 0.15;

const AXES: [Point; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A part of the gumball that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    /// The arrow along an axis, numbered X 0, Y 1, Z 2: moves along it.
    Move(usize),
    /// The circle midway along an axis: extrudes along it.
    Extrude(usize),
    /// The quarter arc that turns around an axis.
    Turn(usize),
    /// The small square on the selection's extent, on the negative side of an axis, at the end of
    /// a dotted line from the centre: scales along it, or evenly with Shift (#693).
    Scale(usize),
    /// The square between the two axes other than this one: moves in their plane.
    Plane(usize),
    /// The dot at the centre: moves freely on the construction plane.
    Centre,
}

impl Handle {
    /// Which axis the handle belongs to, for its colour; the centre belongs to none.
    #[must_use]
    pub fn axis(self) -> Option<usize> {
        match self {
            Self::Move(axis)
            | Self::Extrude(axis)
            | Self::Turn(axis)
            | Self::Scale(axis)
            | Self::Plane(axis) => Some(axis),
            Self::Centre => None,
        }
    }

    /// Whether it is drawn as a filled shape rather than a line.
    #[must_use]
    pub fn is_filled(self) -> bool {
        matches!(self, Self::Plane(_) | Self::Centre | Self::Scale(_))
    }
}

/// What a drag on the gumball does to the objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GumballChange {
    /// Moves them by a vector.
    Move(Point),
    /// Turns them, or scales them evenly.
    Turn(Similarity),
    /// Scales them along one direction from a point.
    Stretch {
        /// What stays put.
        origin: Point,
        /// The direction, of length 1.
        axis: Point,
        /// How much longer they become that way.
        factor: f64,
    },
    /// Extrudes them along a direction: curves become surfaces, flat surfaces solids.
    Extrude {
        /// The direction, of length 1.
        axis: Point,
        /// How far.
        distance: f64,
    },
}

impl GumballChange {
    /// A curve after the change. An extrusion shows the curve where the far end will be.
    #[must_use]
    pub fn applied(&self, curve: &Curve) -> Curve {
        match self {
            Self::Move(by) => curve.translated(*by),
            Self::Turn(change) => curve.mapped(change),
            Self::Stretch {
                origin,
                axis,
                factor,
            } => curve
                .stretched(*origin, *axis, *factor)
                .unwrap_or_else(|_| curve.clone()),
            Self::Extrude { axis, distance } => curve.translated(plus([0.0; 3], *axis, *distance)),
        }
    }

    /// A point after the change, for moving control points.
    #[must_use]
    pub fn moved_point(&self, p: Point) -> Point {
        match self {
            Self::Move(by) => plus(p, *by, 1.0),
            Self::Turn(change) => change.point(p),
            Self::Stretch {
                origin,
                axis,
                factor,
            } => {
                let along = dot(sub(p, *origin), *axis);
                plus(p, *axis, along * (factor - 1.0))
            }
            Self::Extrude { axis, distance } => plus(p, *axis, *distance),
        }
    }

    /// The size of the change, to show beside the pointer: a distance, an angle in degrees, or
    /// a factor, with which it is.
    #[must_use]
    pub fn amount(&self) -> (f64, Amount) {
        match self {
            Self::Move(by) => (dot(*by, *by).sqrt(), Amount::Distance),
            Self::Extrude { distance, .. } => (*distance, Amount::Distance),
            Self::Turn(change) => {
                let factor = change.factor();
                if (factor - 1.0).abs() > 1e-12 {
                    (factor, Amount::Factor)
                } else {
                    (change.angle().to_degrees(), Amount::Angle)
                }
            }
            Self::Stretch { factor, .. } => (*factor, Amount::Factor),
        }
    }
}

/// Which sort of amount a change has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    /// A distance, in the model's unit.
    Distance,
    /// An angle, in degrees.
    Angle,
    /// A scale factor.
    Factor,
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [
        p[0] + direction[0] * amount,
        p[1] + direction[1] * amount,
        p[2] + direction[2] * amount,
    ]
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let square = dx * dx + dy * dy;
    let t = if square < 1e-12 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / square).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Whether `p` is inside the polygon `run` on screen.
fn inside(p: [f64; 2], run: &[[f64; 2]]) -> bool {
    let mut odd = false;
    for (i, a) in run.iter().enumerate() {
        let b = run[(i + 1) % run.len()];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                odd = !odd;
            }
        }
    }
    odd
}

/// A small circle or square around a screen point.
fn mark(at: [f64; 2], round: bool) -> Vec<[f64; 2]> {
    if round {
        (0..16)
            .map(|i| {
                let angle = std::f64::consts::TAU * f64::from(i) / 16.0;
                [
                    at[0] + MARK_PIXELS * angle.cos(),
                    at[1] + MARK_PIXELS * angle.sin(),
                ]
            })
            .collect()
    } else {
        let r = MARK_PIXELS;
        vec![
            [at[0] - r, at[1] - r],
            [at[0] + r, at[1] - r],
            [at[0] + r, at[1] + r],
            [at[0] - r, at[1] + r],
        ]
    }
}

/// The gumball on a selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gumball {
    /// Where its handles start.
    pub centre: Point,
    /// The lowest and highest corners of the box around the selection, which the scale squares
    /// sit on, one per axis on its negative side, as in Rhino 8 (#693); `None` shows no squares.
    pub extent: Option<(Point, Point)>,
}

impl Gumball {
    /// How many model units one pixel covers at the gumball, so its size stays the same on screen.
    fn units_per_pixel(&self, camera: &Camera, size: [f64; 2]) -> Option<f64> {
        let at = camera.project(self.centre, size)?;
        let pixels = AXES
            .iter()
            .filter_map(|axis| camera.project(plus(self.centre, *axis, 1.0), size))
            .map(|p| (p[0] - at[0]).hypot(p[1] - at[1]))
            .fold(0.0, f64::max);
        (pixels > SHORTEST_ON_SCREEN).then(|| 1.0 / pixels)
    }

    /// Every handle as a run of screen points, for drawing and for finding the one under the
    /// cursor. Filled handles' runs are closed outlines.
    #[must_use]
    pub fn outlines(&self, camera: &Camera, size: [f64; 2]) -> Vec<(Handle, Vec<[f64; 2]>)> {
        let Some(unit) = self.units_per_pixel(camera, size) else {
            return Vec::new();
        };
        let Some(at) = camera.project(self.centre, size) else {
            return Vec::new();
        };
        let length = ARROW_PIXELS * unit;
        let mut found = Vec::new();
        for (index, axis) in AXES.iter().enumerate() {
            let along = |part: f64| camera.project(plus(self.centre, *axis, length * part), size);
            // An axis pointing nearly at the viewer is too short to hold its handles.
            let seen = along(1.0).is_some_and(|tip| {
                (tip[0] - at[0]).hypot(tip[1] - at[1]) > ARROW_PIXELS * SHORTEST_SEEN
            });
            if seen && let Some(tip) = along(1.0) {
                found.push((Handle::Move(index), vec![at, tip]));
            }
            if seen && let Some(middle) = along(EXTRUDE_AT) {
                found.push((Handle::Extrude(index), mark(middle, true)));
            }
            let square = self.square_point(camera, size, index);
            if seen && let Some(square) = square.and_then(|p| camera.project(p, size)) {
                found.push((Handle::Scale(index), mark(square, false)));
            }
            let (u, v) = (AXES[(index + 1) % 3], AXES[(index + 2) % 3]);
            // A quarter of a circle, from one of the other axes to the next.
            let radius = ARC_PIXELS * unit;
            let run: Option<Vec<[f64; 2]>> = (0..=ARC_STEPS)
                .map(|step| {
                    let angle = std::f64::consts::FRAC_PI_2 * step as f64 / ARC_STEPS as f64;
                    let along_u = plus(self.centre, u, radius * angle.cos());
                    camera.project(plus(along_u, v, radius * angle.sin()), size)
                })
                .collect();
            found.extend(run.map(|run| (Handle::Turn(index), run)));
            // The plane square, between the two other axes.
            let middle = plus(
                plus(self.centre, u, length * PLANE_AT),
                v,
                length * PLANE_AT,
            );
            let half = length * PLANE_HALF;
            let corners: Option<Vec<[f64; 2]>> =
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .iter()
                    .map(|(a, b)| {
                        camera.project(plus(plus(middle, u, half * a), v, half * b), size)
                    })
                    .collect();
            // A plane seen edge on has no square to hold.
            if dot(*axis, camera.forward()).abs() > PLANE_EDGE_ON {
                found.extend(corners.map(|run| (Handle::Plane(index), run)));
            }
        }
        found.push((Handle::Centre, mark(at, true)));
        found
    }

    /// Where axis `index`'s scale square sits in the model (#693): on the selection's extent on
    /// the negative side of the axis, as Rhino 8 places it, but at least half an arrow from the
    /// centre, so a flat selection's square stays apart from the centre dot. `None` with no
    /// extent.
    #[must_use]
    pub fn square_point(&self, camera: &Camera, size: [f64; 2], index: usize) -> Option<Point> {
        let (low, _) = self.extent?;
        let shortest = 3.0 * MARK_PIXELS * self.units_per_pixel(camera, size)?;
        let back = (self.centre[index] - low[index]).max(shortest);
        Some(plus(self.centre, AXES[index], -back))
    }

    /// The dotted line from the centre to each scale square, on screen, for each axis in order.
    #[must_use]
    pub fn square_lines(&self, camera: &Camera, size: [f64; 2]) -> Vec<(usize, [[f64; 2]; 2])> {
        let Some(at) = camera.project(self.centre, size) else {
            return Vec::new();
        };
        (0..3)
            .filter_map(|index| {
                let square = self.square_point(camera, size, index)?;
                Some((index, [at, camera.project(square, size)?]))
            })
            .collect()
    }

    /// The handle under a screen position, if any. Inside a filled handle counts as on it.
    #[must_use]
    pub fn handle_at(&self, camera: &Camera, screen: [f64; 2], size: [f64; 2]) -> Option<Handle> {
        let mut best: Option<(f64, Handle)> = None;
        for (handle, run) in self.outlines(camera, size) {
            let closed = !matches!(handle, Handle::Move(_) | Handle::Turn(_));
            let mut distance = if closed && run.len() > 2 && inside(screen, &run) {
                0.0
            } else {
                f64::INFINITY
            };
            for pair in run.windows(2) {
                distance = distance.min(to_segment(screen, pair[0], pair[1]));
            }
            if closed && let (Some(first), Some(last)) = (run.first(), run.last()) {
                distance = distance.min(to_segment(screen, *last, *first));
            }
            // The small marks sit on the arrows; they win where they are.
            let lean = if closed { -1.0 } else { 0.0 };
            if distance <= HANDLE_REACH && best.is_none_or(|(held, _)| distance + lean < held) {
                best = Some((distance + lean, handle));
            }
        }
        best.map(|(_, handle)| handle)
    }

    /// What dragging a handle from one screen position to another does.
    #[must_use]
    pub fn change(
        &self,
        camera: &Camera,
        size: [f64; 2],
        handle: Handle,
        from: [f64; 2],
        to: [f64; 2],
    ) -> Option<GumballChange> {
        self.change_with(camera, size, handle, from, to, false, None)
    }

    /// As [`Self::change`]; with `even`, a scale square scales evenly, as Shift does in Rhino;
    /// with `turn_step`, a turn snaps to whole steps of that many degrees (Shift, #529).
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn change_with(
        &self,
        camera: &Camera,
        size: [f64; 2],
        handle: Handle,
        from: [f64; 2],
        to: [f64; 2],
        even: bool,
        turn_step: Option<f64>,
    ) -> Option<GumballChange> {
        let at = camera.project(self.centre, size)?;
        // How far a drag goes along an axis, in model units.
        let along_axis = |index: usize| -> Option<f64> {
            let axis = AXES[index];
            let tip = camera.project(plus(self.centre, axis, 1.0), size)?;
            let per_unit = [tip[0] - at[0], tip[1] - at[1]];
            let square = per_unit[0] * per_unit[0] + per_unit[1] * per_unit[1];
            if square < SHORTEST_ON_SCREEN {
                return None;
            }
            Some(((to[0] - from[0]) * per_unit[0] + (to[1] - from[1]) * per_unit[1]) / square)
        };
        match handle {
            Handle::Move(index) => Some(GumballChange::Move(plus(
                [0.0; 3],
                AXES[index],
                along_axis(index)?,
            ))),
            Handle::Extrude(index) => Some(GumballChange::Extrude {
                axis: AXES[index],
                distance: along_axis(index)?,
            }),
            Handle::Turn(index) => Some(self.turn_change(camera, at, index, [from, to], turn_step)),
            Handle::Scale(index) => self.scale_change(camera, size, at, index, [from, to], even),
            Handle::Plane(_) | Handle::Centre => {
                let index = match handle {
                    Handle::Plane(index) => index,
                    _ => 2,
                };
                let plane = CPlane {
                    origin: self.centre,
                    x: AXES[(index + 1) % 3],
                    y: AXES[(index + 2) % 3],
                    normal: AXES[index],
                };
                let start = camera.plane_point(from, size, &plane)?;
                let end = camera.plane_point(to, size, &plane)?;
                Some(GumballChange::Move(sub(end, start)))
            }
        }
    }

    /// Dragging a turn circle from `from` to `to` on screen, the gumball's centre being `at`.
    fn turn_change(
        &self,
        camera: &Camera,
        at: [f64; 2],
        index: usize,
        [from, to]: [[f64; 2]; 2],
        step: Option<f64>,
    ) -> GumballChange {
        let angle_at = |p: [f64; 2]| (p[1] - at[1]).atan2(p[0] - at[0]);
        // Screen Y points down, so an increasing screen angle looks clockwise.
        let looks_counterclockwise = angle_at(from) - angle_at(to);
        let axis = AXES[index];
        let facing_viewer = dot(axis, camera.forward()) < 0.0;
        let angle = if facing_viewer {
            looks_counterclockwise
        } else {
            -looks_counterclockwise
        };
        // With a step in degrees (Shift held), the angle snaps to whole steps (#529).
        let angle = step.map_or(angle, |degrees| snapped(angle, degrees));
        GumballChange::Turn(Similarity::rotation(self.centre, axis, angle))
    }

    /// Dragging a scale square from `from` to `to` on screen, the gumball's centre being `at`:
    /// the factor is how much further from the centre, along the axis, the pointer is than the
    /// square (where the drag began, with no square), as Rhino 8 scales (#693). With `even`, it
    /// scales evenly.
    fn scale_change(
        &self,
        camera: &Camera,
        size: [f64; 2],
        at: [f64; 2],
        index: usize,
        [from, to]: [[f64; 2]; 2],
        even: bool,
    ) -> Option<GumballChange> {
        let axis = AXES[index];
        let tip = camera.project(plus(self.centre, axis, 1.0), size)?;
        let per_unit = [tip[0] - at[0], tip[1] - at[1]];
        let reach = |p: [f64; 2]| (p[0] - at[0]) * per_unit[0] + (p[1] - at[1]) * per_unit[1];
        let square = self
            .square_point(camera, size, index)
            .and_then(|p| camera.project(p, size));
        let start = reach(square.unwrap_or(from));
        if start.abs() < SHORTEST_ON_SCREEN {
            return None;
        }
        let factor = reach(to) / start;
        if !factor.is_finite() || factor <= 1e-9 {
            return None;
        }
        Some(if even {
            GumballChange::Turn(Similarity::scaling(self.centre, factor))
        } else {
            GumballChange::Stretch {
                origin: self.centre,
                axis,
                factor,
            }
        })
    }

    /// Where an axis handle sits in the model: the tip of an arrow, the middle of a circle.
    #[must_use]
    pub fn handle_point(&self, camera: &Camera, size: [f64; 2], handle: Handle) -> Option<Point> {
        let unit = self.units_per_pixel(camera, size)?;
        if let Handle::Scale(index) = handle
            && let Some(square) = self.square_point(camera, size, index)
        {
            return Some(square);
        }
        let (index, part) = match handle {
            Handle::Move(index) => (index, 1.0),
            Handle::Extrude(index) | Handle::Scale(index) => (index, EXTRUDE_AT),
            _ => return None,
        };
        Some(plus(self.centre, AXES[index], ARROW_PIXELS * unit * part))
    }

    /// The change a value typed for a handle makes: a distance for an arrow, a circle or a plane
    /// square (along its first axis), an angle in degrees for an arc, a factor for a square.
    #[must_use]
    pub fn typed(&self, handle: Handle, value: f64, even: bool) -> Option<GumballChange> {
        if !value.is_finite() {
            return None;
        }
        Some(match handle {
            Handle::Move(index) => GumballChange::Move(plus([0.0; 3], AXES[index], value)),
            Handle::Extrude(index) => GumballChange::Extrude {
                axis: AXES[index],
                distance: value,
            },
            Handle::Turn(index) => GumballChange::Turn(Similarity::rotation(
                self.centre,
                AXES[index],
                value.to_radians(),
            )),
            Handle::Scale(index) if value > 0.0 => {
                if even {
                    GumballChange::Turn(Similarity::scaling(self.centre, value))
                } else {
                    GumballChange::Stretch {
                        origin: self.centre,
                        axis: AXES[index],
                        factor: value,
                    }
                }
            }
            Handle::Plane(index) => {
                GumballChange::Move(plus([0.0; 3], AXES[(index + 1) % 3], value))
            }
            Handle::Scale(_) | Handle::Centre => return None,
        })
    }
}

/// `angle` in radians snapped to the nearest whole step of `degrees`.
fn snapped(angle: f64, degrees: f64) -> f64 {
    let step = degrees.to_radians();
    if step <= 0.0 {
        return angle;
    }
    (angle / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_snaps_to_whole_steps_of_five_degrees() {
        let near = |degrees: f64| super::snapped(degrees.to_radians(), 5.0).to_degrees();
        assert!((near(12.4) - 10.0).abs() < 1e-9);
        assert!((near(12.6) - 15.0).abs() < 1e-9);
        assert!((near(-33.0) + 35.0).abs() < 1e-9);
    }
    use crate::StandardView;
    use curva_kernel::gap;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn at_origin() -> Gumball {
        Gumball {
            centre: [0.0; 3],
            extent: None,
        }
    }

    fn close(a: Point, b: Point) -> bool {
        gap(a, b) < 1e-6
    }

    #[test]
    fn the_cursor_finds_an_arrow() {
        let camera = StandardView::Top.camera();
        let found = at_origin().handle_at(&camera, [480.0, 300.0], SIZE);
        assert_eq!(found, Some(Handle::Move(0)));
    }

    #[test]
    fn the_cursor_finds_a_turning_arc() {
        let camera = StandardView::Top.camera();
        let at = [400.0 + 60.0 * 0.6, 300.0 - 60.0 * 0.8];
        assert_eq!(
            at_origin().handle_at(&camera, at, SIZE),
            Some(Handle::Turn(2))
        );
    }

    #[test]
    fn away_from_the_handles_there_is_nothing() {
        let camera = StandardView::Top.camera();
        assert_eq!(at_origin().handle_at(&camera, [20.0, 20.0], SIZE), None);
    }

    #[test]
    fn the_gumball_stays_the_same_size_on_screen() {
        let mut camera = StandardView::Top.camera();
        camera.zoom(4.0);
        let found = at_origin().handle_at(&camera, [480.0, 300.0], SIZE);
        assert_eq!(found, Some(Handle::Move(0)));
    }

    #[test]
    fn dragging_an_arrow_moves_only_along_its_axis() {
        let camera = StandardView::Top.camera();
        let pixels = camera.project([1.0, 0.0, 0.0], SIZE).unwrap()[0] - 400.0;
        let change = at_origin()
            .change(
                &camera,
                SIZE,
                Handle::Move(0),
                [450.0, 300.0],
                [450.0 + pixels * 5.0, 340.0],
            )
            .unwrap();
        let GumballChange::Move(by) = change else {
            panic!("expected a move");
        };
        assert!(close(by, [5.0, 0.0, 0.0]), "{by:?}");
    }

    #[test]
    fn a_turn_seen_counterclockwise_is_positive_when_the_axis_faces_the_viewer() {
        let camera = StandardView::Top.camera();
        let change = at_origin()
            .change(
                &camera,
                SIZE,
                Handle::Turn(2),
                [460.0, 300.0],
                [400.0, 240.0],
            )
            .unwrap();
        let line = Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
        let start = change.applied(&line).sample().unwrap()[0];
        assert!(close(start, [0.0, 1.0, 0.0]), "{start:?}");
    }

    #[test]
    fn a_turn_is_about_the_gumball_centre() {
        let camera = StandardView::Top.camera();
        let gumball = Gumball {
            centre: [5.0, 0.0, 0.0],
            extent: None,
        };
        let at = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        let change = gumball
            .change(
                &camera,
                SIZE,
                Handle::Turn(2),
                [at[0] + 50.0, at[1]],
                [at[0] - 50.0, at[1] + 1e-9],
            )
            .unwrap();
        let line = Curve::line([6.0, 0.0, 0.0], [7.0, 0.0, 0.0]).unwrap();
        let start = change.applied(&line).sample().unwrap()[0];
        assert!(gap(start, [4.0, 0.0, 0.0]) < 1e-3, "{start:?}");
    }

    #[test]
    fn an_axis_pointing_at_the_viewer_cannot_be_dragged() {
        let camera = StandardView::Top.camera();
        let change = at_origin().change(
            &camera,
            SIZE,
            Handle::Move(2),
            [450.0, 300.0],
            [500.0, 300.0],
        );
        assert_eq!(change, None);
    }

    #[test]
    fn every_handle_is_found_at_its_place() {
        let camera = StandardView::Top.camera();
        let gumball = at_origin();
        for (handle, run) in gumball.outlines(&camera, SIZE) {
            if handle == Handle::Turn(0)
                || handle == Handle::Turn(1)
                || handle == Handle::Plane(0)
                || handle == Handle::Plane(1)
                || matches!(
                    handle,
                    Handle::Move(2) | Handle::Extrude(2) | Handle::Scale(2)
                )
            {
                continue; // Seen edge on from above.
            }
            let middle = run[run.len() / 2];
            let centre = if handle.is_filled() || matches!(handle, Handle::Extrude(_)) {
                let n = run.len() as f64;
                [
                    run.iter().map(|p| p[0]).sum::<f64>() / n,
                    run.iter().map(|p| p[1]).sum::<f64>() / n,
                ]
            } else {
                middle
            };
            assert_eq!(
                gumball.handle_at(&camera, centre, SIZE),
                Some(handle),
                "{handle:?}"
            );
        }
    }

    #[test]
    fn a_plane_square_moves_in_its_plane_and_a_scale_square_along_its_axis() {
        let camera = StandardView::Top.camera();
        let gumball = at_origin();
        let moved = gumball
            .change(
                &camera,
                SIZE,
                Handle::Plane(2),
                [420.0, 280.0],
                [470.0, 230.0],
            )
            .unwrap();
        let GumballChange::Move(by) = moved else {
            panic!("expected a move");
        };
        assert!(by[2].abs() < 1e-9 && by[0] > 0.0 && by[1] > 0.0, "{by:?}");
        let stretch = gumball
            .change(
                &camera,
                SIZE,
                Handle::Scale(0),
                [470.0, 300.0],
                [540.0, 330.0],
            )
            .unwrap();
        let line = Curve::line([1.0, 1.0, 0.0], [2.0, 1.0, 0.0]).unwrap();
        let end = stretch.applied(&line).sample().unwrap()[1];
        assert!(
            (end[0] - 4.0).abs() < 1e-6 && (end[1] - 1.0).abs() < 1e-9,
            "{end:?}"
        );
        // Typed values are exact.
        let typed = gumball.typed(Handle::Turn(2), 90.0, false).unwrap();
        let start = typed.applied(&line).sample().unwrap()[0];
        assert!(close(start, [-1.0, 1.0, 0.0]), "{start:?}");
        assert_eq!(typed.amount(), (90.0, Amount::Angle));
    }

    /// Rhino 8's measured case (#693): a 10 × 5 × 3 box at the origin, the gumball at its centre.
    fn on_a_box() -> (Gumball, Camera) {
        let gumball = Gumball {
            centre: [5.0, 2.5, 1.5],
            extent: Some(([0.0; 3], [10.0, 5.0, 3.0])),
        };
        let mut camera = StandardView::Top.camera();
        camera.frame([-10.0, -10.0, 0.0], [20.0, 15.0, 3.0]);
        (gumball, camera)
    }

    /// Each square sits on the box's side on the negative side of its axis, at the end of its
    /// dotted line, and is found there; none without an extent.
    #[test]
    fn scale_squares_sit_on_the_extent_on_the_negative_side() {
        let (gumball, camera) = on_a_box();
        let x_square = gumball.square_point(&camera, SIZE, 0).unwrap();
        assert!(close(x_square, [0.0, 2.5, 1.5]), "{x_square:?}");
        let y_square = gumball.square_point(&camera, SIZE, 1).unwrap();
        assert!(close(y_square, [5.0, 0.0, 1.5]), "{y_square:?}");
        for (index, model) in [(0, x_square), (1, y_square)] {
            let screen = camera.project(model, SIZE).unwrap();
            assert_eq!(
                gumball.handle_at(&camera, screen, SIZE),
                Some(Handle::Scale(index))
            );
        }
        let lines = gumball.square_lines(&camera, SIZE);
        let x_line = lines.iter().find(|(index, _)| *index == 0).unwrap().1;
        let x_screen = camera.project(x_square, SIZE).unwrap();
        assert!((x_line[1][0] - x_screen[0]).abs() < 1e-9);
        // Seen from above, Z points at the viewer: no square for it.
        let handles: Vec<Handle> = gumball
            .outlines(&camera, SIZE)
            .into_iter()
            .map(|h| h.0)
            .collect();
        assert!(!handles.contains(&Handle::Scale(2)));
        let bare = Gumball {
            extent: None,
            ..gumball
        };
        assert!(bare.square_point(&camera, SIZE, 0).is_none());
        assert!(
            !bare
                .outlines(&camera, SIZE)
                .iter()
                .any(|h| matches!(h.0, Handle::Scale(_)))
        );
    }

    /// Dragging the X square from x = 0 to x = −5 doubles its distance from the centre: the box
    /// scales by 2 along X about the centre, to −5 … 15, as Rhino 8 measured; evenly with Shift.
    #[test]
    fn dragging_a_scale_square_scales_by_the_distance_ratio_about_the_centre() {
        let (gumball, camera) = on_a_box();
        let screen = |p: Point| camera.project(p, SIZE).unwrap();
        let (from, to) = (screen([0.0, 2.5, 1.5]), screen([-5.0, 2.5, 1.5]));
        // A pointer a pixel off the square still gives the measured factor.
        let grabbed = [from[0] + 1.0, from[1] - 1.0];
        let change = gumball
            .change(&camera, SIZE, Handle::Scale(0), grabbed, to)
            .unwrap();
        assert!(close(change.moved_point([0.0, 0.0, 0.0]), [-5.0, 0.0, 0.0]));
        assert!(close(
            change.moved_point([10.0, 5.0, 3.0]),
            [15.0, 5.0, 3.0]
        ));
        assert_eq!(change.amount().1, Amount::Factor);
        assert!((change.amount().0 - 2.0).abs() < 1e-9);
        let even = gumball
            .change_with(&camera, SIZE, Handle::Scale(0), grabbed, to, true, None)
            .unwrap();
        assert!(close(even.moved_point([10.0, 5.0, 3.0]), [15.0, 7.5, 4.5]));
        // A typed factor is exact.
        let typed = gumball.typed(Handle::Scale(0), 2.0, false).unwrap();
        assert!(close(typed.moved_point([0.0; 3]), [-5.0, 0.0, 0.0]));
        assert!(close(
            gumball
                .handle_point(&camera, SIZE, Handle::Scale(0))
                .unwrap(),
            [0.0, 2.5, 1.5]
        ));
    }
}
