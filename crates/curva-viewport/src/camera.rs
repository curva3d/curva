//! The viewport camera: a perspective view orbiting a target, with Z up as in Rhino.

use crate::StandardView;
use curva_document::SavedView;
use curva_kernel::Point;

/// The perspective view's lens to start with, as Rhino's standard: 50 mm on 35 mm film (36 × 24 mm),
/// whose shorter side, 24 mm, spans the view's height: 2·atan(12 / 50), about 27°.
pub(crate) const LENS_LENGTH: f64 = 50.0;
/// How far up and down a flat view sees, against how far out it is, as before the lens
/// followed Rhino's, so flat views keep their scale.
const FLAT_FIELD: f64 = 0.7;
/// Keeps the camera from flipping over the top.
const MOST_PITCH: f64 = 1.55;
/// Radians turned per pixel dragged.
const ORBIT_SPEED: f64 = 0.008;
/// How far out a flat view starts, which shows about the whole grid.
pub(crate) const PARALLEL_DISTANCE: f64 = 330.0;
/// How far, in multiples of the viewing distance, a flat view sees in front of and behind its
/// target before depth runs out.
const FLAT_DEPTH_RANGE: f64 = 1000.0;
/// How far along the depth drawn Bevy's backdrop sits: just inside the farthest distance. Curva's
/// lines take what lies at it or beyond, less a hundredth of the depth, as nothing drawn (#344).
pub(crate) const BACKDROP_DEPTH: f64 = 0.95;

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: Point) -> Point {
    let length = dot(a, a).sqrt().max(1e-12);
    [a[0] / length, a[1] / length, a[2] / length]
}

/// Where the viewport is looking from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The point orbited around.
    pub target: Point,
    /// Turn around the vertical, in radians.
    pub yaw: f64,
    /// Height of the view above the horizon, in radians.
    pub pitch: f64,
    /// Distance from the target, in millimetres.
    pub distance: f64,
    /// Forward, right and up for a view that looks along a fixed direction. `None` means they
    /// come from `yaw` and `pitch`.
    pub(crate) fixed_axes: Option<[Point; 3]>,
    /// Whether the view is flat, with no perspective.
    pub(crate) parallel: bool,
    /// The perspective lens's length in millimetres on 35 mm film ([`Camera::lens_length`]).
    pub(crate) lens_length: f64,
    /// Whether the view is a two-point perspective: always level, so verticals stay upright and
    /// parallel on screen, as Rhino's `TwoPointPerspective`.
    pub(crate) two_point: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: [0.0; 3],
            yaw: -1.0,
            pitch: 0.6,
            // Framing what the earlier, wider lens showed from 150.
            distance: 228.0,
            fixed_axes: None,
            parallel: false,
            lens_length: LENS_LENGTH,
            two_point: false,
        }
    }
}

/// A flat plane to draw on, with its own X and Y directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CPlane {
    /// Where its X and Y start.
    pub origin: Point,
    /// Its X direction, of length 1.
    pub x: Point,
    /// Its Y direction, of length 1.
    pub y: Point,
    /// The direction straight out of it, of length 1.
    pub normal: Point,
}

impl CPlane {
    /// The world's ground plane.
    pub const WORLD_XY: Self = Self {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    /// The plane of the Front view.
    pub const WORLD_XZ: Self = Self {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        normal: [0.0, -1.0, 0.0],
    };
    /// The plane of the Right view.
    pub const WORLD_YZ: Self = Self {
        origin: [0.0; 3],
        x: [0.0, 1.0, 0.0],
        y: [0.0, 0.0, 1.0],
        normal: [1.0, 0.0, 0.0],
    };
}

/// A camera's values worked out once, as snapping projects many points on every frame.
pub(crate) struct Projector {
    eye: Point,
    forward: Point,
    right: Point,
    up: Point,
    focal: f64,
    size: [f64; 2],
    /// The fixed depth of a flat view, which has no perspective.
    parallel_depth: Option<f64>,
}

impl Projector {
    /// Where a model point lands on screen, or `None` behind the camera.
    pub(crate) fn project(&self, p: Point) -> Option<[f64; 2]> {
        let offset = sub(p, self.eye);
        let depth = self
            .parallel_depth
            .unwrap_or_else(|| dot(offset, self.forward));
        if depth <= 1e-9 {
            return None;
        }
        let x = dot(offset, self.right) / depth * self.focal / (self.size[0] / self.size[1]);
        let y = dot(offset, self.up) / depth * self.focal;
        Some([
            (x + 1.0) / 2.0 * self.size[0],
            (1.0 - y) / 2.0 * self.size[1],
        ])
    }
}

impl Camera {
    /// The direction the view looks along.
    pub fn forward(&self) -> Point {
        self.axes().0
    }

    /// Whether the view is flat, with no perspective.
    #[must_use]
    pub fn is_parallel(&self) -> bool {
        self.parallel
    }

    fn eye(&self) -> Point {
        if let Some([forward, ..]) = self.fixed_axes {
            return [
                self.target[0] - forward[0] * self.distance,
                self.target[1] - forward[1] * self.distance,
                self.target[2] - forward[2] * self.distance,
            ];
        }
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        [
            self.target[0] + self.distance * cp * self.yaw.cos(),
            self.target[1] + self.distance * cp * self.yaw.sin(),
            self.target[2] + self.distance * sp,
        ]
    }

    /// Forward, right and up directions.
    fn axes(&self) -> (Point, Point, Point) {
        if let Some([forward, right, up]) = self.fixed_axes {
            return (forward, right, up);
        }
        let forward = unit(sub(self.target, self.eye()));
        let right = unit(cross(forward, [0.0, 0.0, 1.0]));
        (forward, right, cross(right, forward))
    }

    /// How far out the view is against half its height seen there.
    fn focal(&self) -> f64 {
        if self.parallel {
            1.0 / (FLAT_FIELD / 2.0).tan()
        } else {
            self.lens_length / 12.0
        }
    }

    /// How far up and down the view sees, in radians.
    fn field_of_view(&self) -> f64 {
        2.0 * (1.0 / self.focal()).atan()
    }

    /// The same view as [`Camera::matrix`], as the parts another engine's camera is made of
    /// (the Bevy views, decision 0029).
    pub(crate) fn lens(&self) -> Lens {
        let (forward, _, up) = self.axes();
        let (near, far) = if self.parallel {
            // As `matrix`: depth runs over a long range either side of the target.
            let half = FLAT_DEPTH_RANGE * self.distance;
            (self.distance - half, self.distance + half)
        } else {
            (self.distance * 0.01, self.distance * 100.0)
        };
        Lens {
            eye: self.eye(),
            forward,
            up,
            parallel: self.parallel,
            fov: self.field_of_view(),
            half_height: self.distance / self.focal(),
            near,
            far,
        }
    }

    /// The same view of a scene measured from `origin` ([`crate::SceneParts::origin`], #225): its
    /// target moved by `-origin`, worked out in double precision, so its matrix and lens place
    /// positions measured from `origin` exactly.
    #[must_use]
    pub fn measured_from(&self, origin: Point) -> Self {
        let mut moved = *self;
        moved.target = std::array::from_fn(|k| self.target[k] - origin[k]);
        moved
    }

    /// The combined view and projection matrix, by columns, for the graphics card.
    #[must_use]
    pub fn matrix(&self, aspect: f64) -> [[f32; 4]; 4] {
        let (f, r, u) = self.axes();
        let e = self.eye();
        let near = self.distance * 0.01;
        let far = self.distance * 100.0;
        let view = [
            [r[0], r[1], r[2], -dot(r, e)],
            [u[0], u[1], u[2], -dot(u, e)],
            [-f[0], -f[1], -f[2], dot(f, e)],
        ];
        let (fx, fy) = (self.focal() / aspect, self.focal());
        let (a, b) = (far / (near - far), near * far / (near - far));
        let parallel = self.parallel;
        let flat = 1.0 / self.distance;
        let row = |i: usize| match i {
            0 if parallel => view[0].map(|v| v * fx * flat),
            1 if parallel => view[1].map(|v| v * fy * flat),
            // A flat view shows everything along its depth, so depth runs over a long range
            // either side of the target: nearer points still hide farther ones in shaded views.
            2 if parallel => {
                let range = 2.0 * FLAT_DEPTH_RANGE * self.distance;
                let mut row = view[2].map(|v| -v / range);
                row[3] += 0.5 - self.distance / range;
                row
            }
            3 if parallel => [0.0, 0.0, 0.0, 1.0],
            0 => view[0].map(|v| v * fx),
            1 => view[1].map(|v| v * fy),
            2 => [
                view[2][0] * a,
                view[2][1] * a,
                view[2][2] * a,
                view[2][3] * a + b,
            ],
            _ => view[2].map(|v| -v),
        };
        let mut out = [[0.0_f32; 4]; 4];
        for (r_index, r_values) in (0..4).map(|i| (i, row(i))) {
            for (c_index, value) in r_values.into_iter().enumerate() {
                out[c_index][r_index] = value as f32;
            }
        }
        out
    }

    /// The view as hidden-line removal needs it (spec `make2d.md`). A parallel view's eye is
    /// moved far back so it is behind everything.
    #[must_use]
    pub fn look(&self) -> curva_kernel::hidden::View {
        let (forward, right, up) = self.axes();
        let eye = if self.parallel {
            let back = self.distance.max(1.0) * 1e4;
            std::array::from_fn(|i| self.target[i] - forward[i] * back)
        } else {
            self.eye()
        };
        curva_kernel::hidden::View {
            eye,
            forward,
            right,
            up,
            parallel: self.parallel,
        }
    }

    /// Where a model point lands on screen, or `None` behind the camera.
    #[must_use]
    pub fn project(&self, p: Point, size: [f64; 2]) -> Option<[f64; 2]> {
        self.projector(size).project(p)
    }

    /// The camera's working values, ready for projecting many points quickly.
    #[must_use]
    pub(crate) fn projector(&self, size: [f64; 2]) -> Projector {
        let (forward, right, up) = self.axes();
        Projector {
            eye: self.eye(),
            forward,
            right,
            up,
            focal: self.focal(),
            size,
            parallel_depth: self.parallel.then_some(self.distance),
        }
    }

    /// The point on the ground plane (Z = 0) under a screen position.
    #[must_use]
    pub fn ground_point(&self, screen: [f64; 2], size: [f64; 2]) -> Option<Point> {
        self.plane_point(screen, size, &CPlane::WORLD_XY)
    }

    /// The line of sight through a screen position: where it starts and its direction. In a flat
    /// view every line of sight runs the same way, from a point beside the view's centre.
    #[must_use]
    pub fn ray(&self, screen: [f64; 2], size: [f64; 2]) -> (Point, Point) {
        let (f, r, u) = self.axes();
        let x = (screen[0] / size[0] * 2.0 - 1.0) * (size[0] / size[1]) / self.focal();
        let y = (1.0 - screen[1] / size[1] * 2.0) / self.focal();
        let eye = self.eye();
        if self.parallel {
            let sideways = |axis: usize| eye[axis] + (r[axis] * x + u[axis] * y) * self.distance;
            ([sideways(0), sideways(1), sideways(2)], f)
        } else {
            let along = |axis: usize| f[axis] + r[axis] * x + u[axis] * y;
            (eye, [along(0), along(1), along(2)])
        }
    }

    /// The point on the line through `origin` along `direction` nearest the line of sight through
    /// a screen position, as Rhino finds a height from the mouse. `None` when looking straight
    /// along the line.
    #[must_use]
    pub fn line_point(
        &self,
        screen: [f64; 2],
        size: [f64; 2],
        origin: Point,
        direction: Point,
    ) -> Option<Point> {
        let (start, sight) = self.ray(screen, size);
        let apart = sub(start, origin);
        let (a, b, c) = (
            dot(sight, sight),
            dot(sight, direction),
            dot(direction, direction),
        );
        let (d, e) = (dot(sight, apart), dot(direction, apart));
        let denominator = a * c - b * b;
        if denominator.abs() < 1e-12 * a * c {
            return None;
        }
        let t = (a * e - b * d) / denominator;
        Some(std::array::from_fn(|i| origin[i] + direction[i] * t))
    }

    /// The point on a plane under a screen position, if the view can reach it.
    #[must_use]
    pub fn plane_point(&self, screen: [f64; 2], size: [f64; 2], plane: &CPlane) -> Option<Point> {
        let (start, direction) = self.ray(screen, size);
        let facing = dot(direction, plane.normal);
        if facing.abs() < 1e-9 {
            return None;
        }
        let t = dot(sub(plane.origin, start), plane.normal) / facing;
        // A flat view has no eye, so the plane counts wherever it lies along the view.
        (self.parallel || t > 0.0).then(|| {
            [
                start[0] + direction[0] * t,
                start[1] + direction[1] * t,
                start[2] + direction[2] * t,
            ]
        })
    }

    /// Turns the view for a mouse drag in pixels.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx * ORBIT_SPEED;
        // A two-point view stays level; it only turns.
        if self.two_point {
            return;
        }
        self.pitch = (self.pitch + dy * ORBIT_SPEED).clamp(-MOST_PITCH, MOST_PITCH);
    }

    /// Slides the view for a mouse drag in pixels, so the model follows the cursor.
    pub fn pan(&mut self, dx: f64, dy: f64, height: f64) {
        let (_, r, u) = self.axes();
        let scale = 2.0 * self.distance / self.focal() / height;
        for axis in 0..3 {
            self.target[axis] += (-r[axis] * dx + u[axis] * dy) * scale;
        }
    }

    /// Looks straight down on `plane`, flat, its X to the right and its Y up, as Rhino's Plan.
    pub fn plan(&mut self, plane: &CPlane) {
        self.fixed_axes = Some([plane.normal.map(|v| -v), plane.x, plane.y]);
        self.distance = self.distance.max(PARALLEL_DISTANCE);
        self.parallel = true;
    }

    /// Moves closer (factor below 1) or further away.
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(0.01, 1e8);
    }

    /// Moves closer (factor below 1) or further away, keeping `about` where it is on screen, as
    /// Ctrl + right-drag does in Rhino.
    pub fn zoom_about(&mut self, factor: f64, about: Point) {
        let before = self.distance;
        self.zoom(factor);
        let applied = self.distance / before;
        let target = self.target;
        self.target =
            std::array::from_fn(|axis| about[axis] + (target[axis] - about[axis]) * applied);
    }

    /// Aims the view so a box of points fits.
    pub fn frame(&mut self, low: Point, high: Point) {
        for axis in 0..3 {
            self.target[axis] = (low[axis] + high[axis]) / 2.0;
        }
        let diagonal = dot(sub(high, low), sub(high, low)).sqrt().max(1.0);
        self.distance = diagonal * self.focal() * 0.75;
    }

    /// Aims a flat view at `centre` so exactly `half_height` model units show above and below it,
    /// as a sheet of paper is framed (spec `layouts.md` §1).
    pub fn show_exactly(&mut self, centre: Point, half_height: f64) {
        self.target = centre;
        self.distance = (half_height * self.focal()).clamp(0.01, 1e8);
    }

    /// This camera saved under a name, as seen from the standard view `kind`.
    #[must_use]
    pub fn saved(&self, name: String, kind: StandardView) -> SavedView {
        SavedView {
            name,
            kind: kind.key().to_owned(),
            target: self.target,
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            axes: self.fixed_axes,
            parallel: self.parallel,
            // Kept by the display stream once the camera has a lens (#382).
            lens: None,
        }
    }

    /// The camera a saved view holds.
    #[must_use]
    pub fn from_saved(view: &SavedView) -> Self {
        Self {
            target: view.target,
            yaw: view.yaw,
            pitch: view.pitch,
            distance: view.distance,
            fixed_axes: view.axes,
            parallel: view.parallel,
            // Saved views do not keep the lens yet; they open with Rhino's standard.
            lens_length: LENS_LENGTH,
            two_point: view.kind == StandardView::TwoPoint.key(),
        }
    }
}

/// A view as an engine's camera takes it: where the eye is, where it looks, and its lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lens {
    pub(crate) eye: Point,
    /// The direction looked along, of length 1.
    pub(crate) forward: Point,
    /// Up on the screen, of length 1.
    pub(crate) up: Point,
    /// Whether the view is flat, with no perspective.
    pub(crate) parallel: bool,
    /// The angle from the bottom to the top of the view, in radians, for a perspective view.
    pub(crate) fov: f64,
    /// Half the height shown, in millimetres, for a flat view.
    pub(crate) half_height: f64,
    /// The nearest and farthest distances drawn, along `forward` from the eye.
    pub(crate) near: f64,
    pub(crate) far: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The perspective view sees as Rhino's standard 50 mm lens does: its height spans what
    /// 24 mm of film does at 50 mm, about 27°, in Curva's matrix and in the lens Bevy gets alike;
    /// flat views keep their scale.
    #[test]
    fn the_perspective_lens_is_rhinos_50_mm() {
        let camera = Camera::default();
        let rhino = 2.0 * (12.0_f64 / 50.0).atan();
        assert!((camera.lens().fov - rhino).abs() < 1e-12);
        assert!((rhino.to_degrees() - 26.99).abs() < 0.01);
        // A point at the top edge of the view, at the target's distance, lands on the edge.
        let (_, _, up) = camera.axes();
        let half = camera.distance * (rhino / 2.0).tan();
        let edge = std::array::from_fn(|k| camera.target[k] + up[k] * half);
        let at = camera.project(edge, [800.0, 600.0]).unwrap();
        assert!(at[1].abs() < 1e-6, "{at:?}");
        let flat = StandardView::Top.camera();
        assert!((flat.lens().half_height - PARALLEL_DISTANCE * (0.35_f64).tan()).abs() < 1e-9);
    }

    #[test]
    fn the_mouse_finds_heights_along_a_line() {
        let size = [800.0, 600.0];
        for camera in [Camera::default(), StandardView::Front.camera()] {
            let up = [0.0, 0.0, 1.0];
            for height in [-3.0, 0.0, 12.5] {
                let point = [2.0, 1.0, height];
                let screen = camera.project(point, size).unwrap();
                let found = camera
                    .line_point(screen, size, [2.0, 1.0, 0.0], up)
                    .unwrap();
                assert!(
                    curva_kernel::gap(found, point) < 1e-6,
                    "{found:?} for {point:?}"
                );
            }
        }
        // Looking straight down a vertical line gives no height.
        let top = StandardView::Top.camera();
        let screen = top.project([0.0; 3], size).unwrap();
        assert!(
            top.line_point(screen, size, [0.0; 3], [0.0, 0.0, 1.0])
                .is_none()
        );
    }

    #[test]
    fn flat_views_keep_nearer_points_in_front() {
        let camera = StandardView::Top.camera();
        let depth = |p: Point| {
            let m = camera.matrix(1.5);
            let clip: Vec<f32> = (0..4)
                .map(|r| (0..3).map(|c| m[c][r] * p[c] as f32).sum::<f32>() + m[3][r])
                .collect();
            clip[2] / clip[3]
        };
        let (high, low) = (depth([0.0, 0.0, 50.0]), depth([0.0, 0.0, -50.0]));
        assert!(
            high < low,
            "seen from above, higher is nearer: {high} {low}"
        );
        assert!((0.0..=1.0).contains(&high) && (0.0..=1.0).contains(&low));
    }

    #[test]
    fn a_saved_view_gives_back_the_same_camera() {
        for view in [StandardView::Top, StandardView::Perspective] {
            let mut camera = view.camera();
            camera.zoom(0.5);
            camera.target = [3.0, 4.0, 5.0];
            let saved = camera.saved("Corner".to_owned(), view);
            assert_eq!(StandardView::from_key(&saved.kind), Some(view));
            assert_eq!(Camera::from_saved(&saved), camera);
        }
    }

    const SIZE: [f64; 2] = [800.0, 600.0];

    #[test]
    fn the_target_is_in_the_middle_of_the_screen() {
        let at = Camera::default().project([0.0; 3], SIZE).unwrap();
        assert!((at[0] - 400.0).abs() < 1e-6 && (at[1] - 300.0).abs() < 1e-6);
    }

    #[test]
    fn a_ground_point_projects_back_to_where_it_was_clicked() {
        let camera = Camera::default();
        let screen = [523.0, 187.0];
        let ground = camera.ground_point(screen, SIZE).unwrap();
        assert!(ground[2].abs() < 1e-9);
        let back = camera.project(ground, SIZE).unwrap();
        assert!((back[0] - screen[0]).abs() < 1e-6 && (back[1] - screen[1]).abs() < 1e-6);
    }

    #[test]
    fn the_matrix_agrees_with_project() {
        let camera = Camera::default();
        let m = camera.matrix(SIZE[0] / SIZE[1]);
        let p = [12.0_f32, -7.0, 3.0];
        let clip: Vec<f32> = (0..4)
            .map(|row| m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row])
            .collect();
        let expected = camera
            .project([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])], SIZE)
            .unwrap();
        let x = (f64::from(clip[0] / clip[3]) + 1.0) / 2.0 * SIZE[0];
        let y = (1.0 - f64::from(clip[1] / clip[3])) / 2.0 * SIZE[1];
        assert!((x - expected[0]).abs() < 1e-2 && (y - expected[1]).abs() < 1e-2);
    }

    /// The point on screen where a model point lands, for flat-view tests.
    fn on_screen(camera: &Camera, p: Point) -> [f64; 2] {
        camera.project(p, SIZE).unwrap()
    }

    #[test]
    fn top_shows_x_to_the_right_and_y_up() {
        let camera = StandardView::Top.camera();
        let (centre, x, y) = (
            on_screen(&camera, [0.0; 3]),
            on_screen(&camera, [10.0, 0.0, 0.0]),
            on_screen(&camera, [0.0, 10.0, 0.0]),
        );
        assert!(x[0] > centre[0] && (x[1] - centre[1]).abs() < 1e-9);
        assert!(y[1] < centre[1] && (y[0] - centre[0]).abs() < 1e-9);
    }

    #[test]
    fn front_shows_x_to_the_right_and_z_up() {
        let camera = StandardView::Front.camera();
        let (centre, x, z) = (
            on_screen(&camera, [0.0; 3]),
            on_screen(&camera, [10.0, 0.0, 0.0]),
            on_screen(&camera, [0.0, 0.0, 10.0]),
        );
        assert!(x[0] > centre[0] && (x[1] - centre[1]).abs() < 1e-9);
        assert!(z[1] < centre[1] && (z[0] - centre[0]).abs() < 1e-9);
    }

    #[test]
    fn right_shows_y_to_the_right_and_z_up() {
        let camera = StandardView::Right.camera();
        let (centre, y, z) = (
            on_screen(&camera, [0.0; 3]),
            on_screen(&camera, [0.0, 10.0, 0.0]),
            on_screen(&camera, [0.0, 0.0, 10.0]),
        );
        assert!(y[0] > centre[0] && (y[1] - centre[1]).abs() < 1e-9);
        assert!(z[1] < centre[1] && (z[0] - centre[0]).abs() < 1e-9);
    }

    #[test]
    fn a_flat_view_ignores_depth() {
        let camera = StandardView::Top.camera();
        let (near, far) = (
            on_screen(&camera, [7.0, 3.0, 500.0]),
            on_screen(&camera, [7.0, 3.0, -500.0]),
        );
        assert!((near[0] - far[0]).abs() < 1e-9 && (near[1] - far[1]).abs() < 1e-9);
    }

    #[test]
    fn only_the_perspective_view_has_perspective() {
        let camera = StandardView::Perspective.camera();
        assert!(!camera.is_parallel());
        assert!(StandardView::Top.camera().is_parallel());
    }

    #[test]
    fn clicks_land_on_each_views_plane_and_project_back() {
        for view in [
            StandardView::Top,
            StandardView::Front,
            StandardView::Right,
            StandardView::Perspective,
        ] {
            let (camera, plane) = (view.camera(), view.plane());
            let screen = [523.0, 187.0];
            let hit = camera.plane_point(screen, SIZE, &plane).unwrap();
            assert!(
                dot(sub(hit, plane.origin), plane.normal).abs() < 1e-9,
                "{view:?}"
            );
            let back = camera.project(hit, SIZE).unwrap();
            assert!((back[0] - screen[0]).abs() < 1e-6, "{view:?}");
            assert!((back[1] - screen[1]).abs() < 1e-6, "{view:?}");
        }
    }

    #[test]
    fn a_click_in_front_can_leave_the_ground() {
        let camera = StandardView::Front.camera();
        let hit = camera
            .plane_point([400.0, 100.0], SIZE, &CPlane::WORLD_XZ)
            .unwrap();
        assert!(hit[2] > 10.0 && hit[1].abs() < 1e-9);
    }

    #[test]
    fn the_flat_matrix_agrees_with_project() {
        let camera = StandardView::Front.camera();
        let m = camera.matrix(SIZE[0] / SIZE[1]);
        let p = [12.0_f32, -7.0, 3.0];
        let clip: Vec<f32> = (0..4)
            .map(|row| m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row])
            .collect();
        let expected = on_screen(&camera, [12.0, -7.0, 3.0]);
        let x = (f64::from(clip[0] / clip[3]) + 1.0) / 2.0 * SIZE[0];
        let y = (1.0 - f64::from(clip[1] / clip[3])) / 2.0 * SIZE[1];
        assert!((x - expected[0]).abs() < 1e-2 && (y - expected[1]).abs() < 1e-2);
        assert!(
            (0.0..=1.0).contains(&(clip[2] / clip[3])),
            "must not be clipped"
        );
    }

    #[test]
    fn framing_a_flat_view_centres_the_box() {
        let mut camera = StandardView::Top.camera();
        camera.frame([100.0, 40.0, 0.0], [140.0, 80.0, 0.0]);
        let at = on_screen(&camera, [120.0, 60.0, 0.0]);
        assert!((at[0] - 400.0).abs() < 1e-6 && (at[1] - 300.0).abs() < 1e-6);
    }

    #[test]
    fn orbit_cannot_flip_over_the_top() {
        let mut camera = Camera::default();
        camera.orbit(0.0, 1e6);
        assert!(camera.pitch <= MOST_PITCH);
    }

    #[test]
    fn panning_moves_the_model_with_the_cursor() {
        let mut camera = Camera::default();
        let before = camera.project([0.0; 3], SIZE).unwrap();
        camera.pan(40.0, 0.0, SIZE[1]);
        let after = camera.project([0.0; 3], SIZE).unwrap();
        assert!((after[0] - before[0] - 40.0).abs() < 1.0);
    }

    #[test]
    fn zooming_about_a_point_keeps_it_in_place_on_screen() {
        let mut camera = StandardView::Top.camera();
        let size = [800.0, 600.0];
        let about = [30.0, 20.0, 0.0];
        let before = camera.project(about, size).unwrap();
        camera.zoom_about(0.5, about);
        let after = camera.project(about, size).unwrap();
        assert!((after[0] - before[0]).abs() < 1e-6 && (after[1] - before[1]).abs() < 1e-6);
    }
}
