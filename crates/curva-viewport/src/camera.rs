//! The viewport camera: a perspective view orbiting a target, with Z up as in Rhino.

use curva_kernel::Point;

const FIELD_OF_VIEW: f64 = 0.7;
/// Keeps the camera from flipping over the top.
const MOST_PITCH: f64 = 1.55;
/// Radians turned per pixel dragged.
const ORBIT_SPEED: f64 = 0.008;

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
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: [0.0; 3],
            yaw: -1.0,
            pitch: 0.6,
            distance: 150.0,
        }
    }
}

/// A camera's values worked out once, as snapping projects many points on every frame.
pub(crate) struct Projector {
    eye: Point,
    forward: Point,
    right: Point,
    up: Point,
    focal: f64,
    size: [f64; 2],
}

impl Projector {
    /// Where a model point lands on screen, or `None` behind the camera.
    pub(crate) fn project(&self, p: Point) -> Option<[f64; 2]> {
        let offset = sub(p, self.eye);
        let depth = dot(offset, self.forward);
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
    fn eye(&self) -> Point {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        [
            self.target[0] + self.distance * cp * self.yaw.cos(),
            self.target[1] + self.distance * cp * self.yaw.sin(),
            self.target[2] + self.distance * sp,
        ]
    }

    /// Forward, right and up directions.
    fn axes(&self) -> (Point, Point, Point) {
        let forward = unit(sub(self.target, self.eye()));
        let right = unit(cross(forward, [0.0, 0.0, 1.0]));
        (forward, right, cross(right, forward))
    }

    fn focal(&self) -> f64 {
        1.0 / (FIELD_OF_VIEW / 2.0).tan()
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
        let row = |i: usize| match i {
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
        }
    }

    /// The point on the ground plane (Z = 0) under a screen position.
    #[must_use]
    pub fn ground_point(&self, screen: [f64; 2], size: [f64; 2]) -> Option<Point> {
        let (f, r, u) = self.axes();
        let x = (screen[0] / size[0] * 2.0 - 1.0) * (size[0] / size[1]) / self.focal();
        let y = (1.0 - screen[1] / size[1] * 2.0) / self.focal();
        let direction = [
            f[0] + r[0] * x + u[0] * y,
            f[1] + r[1] * x + u[1] * y,
            f[2] + r[2] * x + u[2] * y,
        ];
        if direction[2].abs() < 1e-9 {
            return None;
        }
        let eye = self.eye();
        let t = -eye[2] / direction[2];
        (t > 0.0).then(|| [eye[0] + direction[0] * t, eye[1] + direction[1] * t, 0.0])
    }

    /// Turns the view for a mouse drag in pixels.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx * ORBIT_SPEED;
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

    /// Moves closer (factor below 1) or further away.
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(0.01, 1e8);
    }

    /// Aims the view so a box of points fits.
    pub fn frame(&mut self, low: Point, high: Point) {
        for axis in 0..3 {
            self.target[axis] = (low[axis] + high[axis]) / 2.0;
        }
        let diagonal = dot(sub(high, low), sub(high, low)).sqrt().max(1.0);
        self.distance = diagonal * self.focal() * 0.75;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
