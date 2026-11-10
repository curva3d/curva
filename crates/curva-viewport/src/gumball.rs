//! The gumball: on-screen handles that move and turn the selected objects.

use crate::Camera;
use curva_kernel::{Curve, Point, Similarity};

/// How long an arrow is on screen, in pixels.
const ARROW_PIXELS: f64 = 90.0;
/// The radius of the turning arcs on screen, in pixels.
const ARC_PIXELS: f64 = 60.0;
/// How close the cursor must be to a handle to take it, in pixels.
const HANDLE_REACH: f64 = 7.0;
/// Straight pieces in the drawing of each turning arc.
const ARC_STEPS: usize = 48;
/// An axis pointing nearly at the viewer is too short on screen to be dragged.
const SHORTEST_ON_SCREEN: f64 = 1e-3;

const AXES: [Point; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A part of the gumball that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// The arrow along an axis, numbered X 0, Y 1, Z 2.
    Move(usize),
    /// The arc that turns around an axis.
    Turn(usize),
}

impl Handle {
    /// Which axis the handle belongs to.
    #[must_use]
    pub fn axis(self) -> usize {
        match self {
            Self::Move(axis) | Self::Turn(axis) => axis,
        }
    }
}

/// What a drag on the gumball does to the objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GumballChange {
    /// Moves them by a vector.
    Move(Point),
    /// Turns them.
    Turn(Similarity),
}

impl GumballChange {
    /// A curve after the change.
    #[must_use]
    pub fn applied(&self, curve: &Curve) -> Curve {
        match self {
            Self::Move(by) => curve.translated(*by),
            Self::Turn(change) => curve.mapped(change),
        }
    }

    /// A point after the change, for moving control points.
    #[must_use]
    pub fn moved_point(&self, p: Point) -> Point {
        match self {
            Self::Move(by) => plus(p, *by, 1.0),
            Self::Turn(change) => change.point(p),
        }
    }
}

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    [
        p[0] + direction[0] * amount,
        p[1] + direction[1] * amount,
        p[2] + direction[2] * amount,
    ]
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

/// The gumball on a selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gumball {
    /// Where its handles start.
    pub centre: Point,
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

    /// Every handle as a run of screen points, for drawing and for finding the one under the cursor.
    #[must_use]
    pub fn outlines(&self, camera: &Camera, size: [f64; 2]) -> Vec<(Handle, Vec<[f64; 2]>)> {
        let Some(unit) = self.units_per_pixel(camera, size) else {
            return Vec::new();
        };
        let Some(at) = camera.project(self.centre, size) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for (index, axis) in AXES.iter().enumerate() {
            if let Some(tip) = camera.project(plus(self.centre, *axis, ARROW_PIXELS * unit), size) {
                found.push((Handle::Move(index), vec![at, tip]));
            }
            let (u, v) = (AXES[(index + 1) % 3], AXES[(index + 2) % 3]);
            let radius = ARC_PIXELS * unit;
            let run: Option<Vec<[f64; 2]>> = (0..=ARC_STEPS)
                .map(|step| {
                    let angle = std::f64::consts::TAU * step as f64 / ARC_STEPS as f64;
                    let along_u = plus(self.centre, u, radius * angle.cos());
                    camera.project(plus(along_u, v, radius * angle.sin()), size)
                })
                .collect();
            found.extend(run.map(|run| (Handle::Turn(index), run)));
        }
        found
    }

    /// The handle under a screen position, if any.
    #[must_use]
    pub fn handle_at(&self, camera: &Camera, screen: [f64; 2], size: [f64; 2]) -> Option<Handle> {
        let mut best: Option<(f64, Handle)> = None;
        for (handle, run) in self.outlines(camera, size) {
            for pair in run.windows(2) {
                let distance = to_segment(screen, pair[0], pair[1]);
                if distance <= HANDLE_REACH && best.is_none_or(|(held, _)| distance < held) {
                    best = Some((distance, handle));
                }
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
        let at = camera.project(self.centre, size)?;
        match handle {
            Handle::Move(index) => {
                let axis = AXES[index];
                let tip = camera.project(plus(self.centre, axis, 1.0), size)?;
                let per_unit = [tip[0] - at[0], tip[1] - at[1]];
                let square = per_unit[0] * per_unit[0] + per_unit[1] * per_unit[1];
                if square < SHORTEST_ON_SCREEN {
                    return None;
                }
                let moved = (to[0] - from[0]) * per_unit[0] + (to[1] - from[1]) * per_unit[1];
                Some(GumballChange::Move(plus([0.0; 3], axis, moved / square)))
            }
            Handle::Turn(index) => {
                let angle_at = |p: [f64; 2]| (p[1] - at[1]).atan2(p[0] - at[0]);
                // Screen Y points down, so an increasing screen angle looks clockwise.
                let looks_counterclockwise = angle_at(from) - angle_at(to);
                let axis = AXES[index];
                let facing_viewer = curva_kernel::dot(axis, camera.forward()) < 0.0;
                let angle = if facing_viewer {
                    looks_counterclockwise
                } else {
                    -looks_counterclockwise
                };
                Some(GumballChange::Turn(Similarity::rotation(
                    self.centre,
                    axis,
                    angle,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StandardView;
    use curva_kernel::gap;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn at_origin() -> Gumball {
        Gumball { centre: [0.0; 3] }
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
}
