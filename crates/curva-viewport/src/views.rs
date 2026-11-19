//! The standard views and the perspective lens (#356, for the Display panel's view picker): the
//! six sides of the box as Rhino has them, the perspective and two-point perspective views, and
//! the lens in millimetres or as an angle of view.

#[cfg(test)]
use crate::camera::LENS_LENGTH;
use crate::camera::PARALLEL_DISTANCE;
use crate::{CPlane, Camera};
use curva_kernel::Point;

/// The shortest and longest lenses offered, in millimetres: Rhino's limits are wider, but past
/// these the view is a fisheye or a telescope.
const LENS_RANGE: (f64, f64) = (5.0, 2000.0);

/// The views every model can be looked at from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    /// From above, on the ground plane.
    Top,
    /// From the front, on the XZ plane.
    Front,
    /// From the right, on the YZ plane.
    Right,
    /// An orbiting perspective view, on the ground plane.
    Perspective,
    /// From below, looking up at the ground plane, X to the right as in Rhino.
    Bottom,
    /// From the left, looking along +X.
    Left,
    /// From the back, looking along -Y.
    Back,
    /// A perspective view that stays level, so verticals stay upright and parallel on screen,
    /// as Rhino's `TwoPointPerspective`; on the ground plane.
    TwoPoint,
}

impl StandardView {
    /// Every view, in the order they are listed.
    pub const ALL: [Self; 8] = [
        Self::Top,
        Self::Front,
        Self::Right,
        Self::Perspective,
        Self::Bottom,
        Self::Left,
        Self::Back,
        Self::TwoPoint,
    ];

    /// A camera for this view.
    #[must_use]
    pub fn camera(self) -> Camera {
        let flat = |[forward, right, up]: [Point; 3]| Camera {
            fixed_axes: Some([forward, right, up]),
            parallel: true,
            distance: PARALLEL_DISTANCE,
            ..Camera::default()
        };
        match self.flat_axes() {
            Some(axes) => flat(axes),
            None if self == Self::TwoPoint => Camera {
                pitch: 0.0,
                two_point: true,
                ..Camera::default()
            },
            None => Camera::default(),
        }
    }

    /// Forward, right and up for a flat view; `None` for the perspective views.
    fn flat_axes(self) -> Option<[Point; 3]> {
        let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let minus = |p: Point| p.map(|v: f64| -v);
        Some(match self {
            Self::Top => [minus(z), x, y],
            Self::Bottom => [z, x, minus(y)],
            Self::Front => [y, x, z],
            Self::Back => [minus(y), minus(x), z],
            Self::Right => [minus(x), y, z],
            Self::Left => [x, minus(y), z],
            Self::Perspective | Self::TwoPoint => return None,
        })
    }

    /// The plane clicks in this view land on: for a flat view the plane facing it, its X to the
    /// right and its Y up, as Rhino's construction planes for its standard views.
    #[must_use]
    pub fn plane(self) -> CPlane {
        match self.flat_axes() {
            Some([forward, right, up]) => CPlane {
                origin: [0.0; 3],
                x: right,
                y: up,
                normal: forward.map(|v| -v),
            },
            None => CPlane::WORLD_XY,
        }
    }

    /// The word a saved view stores for this view.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Front => "Front",
            Self::Right => "Right",
            Self::Perspective => "Perspective",
            Self::Bottom => "Bottom",
            Self::Left => "Left",
            Self::Back => "Back",
            Self::TwoPoint => "TwoPoint",
        }
    }

    /// The view a saved view's word stands for.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.key() == key)
    }
}

impl Camera {
    /// The perspective lens's length in millimetres, on 35 mm film as Rhino gives it: 50 mm to
    /// start with. Flat views do not use it.
    #[must_use]
    pub fn lens_length(&self) -> f64 {
        self.lens_length
    }

    /// Sets the lens's length in millimetres, kept between 5 and 2000 mm. A longer lens sees less
    /// of the model from the same place.
    pub fn set_lens_length(&mut self, millimetres: f64) {
        if millimetres.is_finite() {
            self.lens_length = millimetres.clamp(LENS_RANGE.0, LENS_RANGE.1);
        }
    }

    /// The angle of view, in degrees, as Rhino shows it beside the lens: across the film's 36 mm
    /// width, so 39.6° for the 50 mm lens.
    #[must_use]
    pub fn angle_of_view(&self) -> f64 {
        (2.0 * (18.0 / self.lens_length).atan()).to_degrees()
    }

    /// Sets the angle of view in degrees, as Rhino's: the lens that gives it.
    pub fn set_angle_of_view(&mut self, degrees: f64) {
        let half = (degrees / 2.0).to_radians();
        if half > 0.0 && half < std::f64::consts::FRAC_PI_2 {
            self.set_lens_length(18.0 / half.tan());
        }
    }

    /// Whether the view is a two-point perspective, always level.
    #[must_use]
    pub fn is_two_point(&self) -> bool {
        self.two_point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-12)
    }

    /// Each flat view looks the way Rhino's does, with up and right square to it and making a
    /// right-handed screen; its plane faces the camera; its word comes back as the same view.
    #[test]
    fn each_view_looks_as_rhinos() {
        let size = [800.0, 600.0];
        let expected: [(StandardView, Point, Point); 6] = [
            (StandardView::Top, [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            (StandardView::Bottom, [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
            (StandardView::Front, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            (StandardView::Back, [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            (StandardView::Right, [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            (StandardView::Left, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ];
        for (view, forward, up) in expected {
            let camera = view.camera();
            assert!(close(camera.forward(), forward), "{view:?}");
            assert!(camera.is_parallel());
            let plane = view.plane();
            assert!(close(plane.y, up), "{view:?}");
            assert!(close(plane.normal, forward.map(|v| -v)), "{view:?}");
            // Its plane's X runs to the right on screen and its Y up.
            let o = camera.project([0.0; 3], size).unwrap();
            let x = camera.project(plane.x, size).unwrap();
            let y = camera.project(plane.y, size).unwrap();
            assert!(x[0] > o[0] && (x[1] - o[1]).abs() < 1e-9, "{view:?}");
            assert!(y[1] < o[1] && (y[0] - o[0]).abs() < 1e-9, "{view:?}");
            assert_eq!(StandardView::from_key(view.key()), Some(view));
        }
        // Bottom shows X to the right, as the issue asks.
        let bottom = StandardView::Bottom.camera();
        let o = bottom.project([0.0; 3], size).unwrap();
        assert!(bottom.project([1.0, 0.0, 0.0], size).unwrap()[0] > o[0]);
        for view in [StandardView::Perspective, StandardView::TwoPoint] {
            assert_eq!(StandardView::from_key(view.key()), Some(view));
            assert_eq!(view.plane(), CPlane::WORLD_XY);
            assert!(!view.camera().is_parallel());
        }
    }

    /// The lens reads and sets in millimetres or degrees, linked as Rhino links them: 50 mm is
    /// 39.6°; it changes what the view sees and stays when the view is turned.
    #[test]
    fn the_lens_reads_in_millimetres_and_degrees() {
        let mut camera = Camera::default();
        assert_eq!(camera.lens_length(), LENS_LENGTH);
        assert!((camera.angle_of_view() - 39.6).abs() < 0.05);
        camera.set_angle_of_view(39.6);
        assert!((camera.lens_length() - 50.0).abs() < 0.1);
        camera.set_lens_length(50.0);
        // A 100 mm lens: half the size on film of a point at the edge, so it shows twice as big.
        let size = [800.0, 600.0];
        let point = [camera.target[0], camera.target[1], camera.target[2] + 10.0];
        let wide = camera.project(point, size).unwrap()[1];
        camera.set_lens_length(100.0);
        let long = camera.project(point, size).unwrap()[1];
        assert!(((300.0 - long) / (300.0 - wide) - 2.0).abs() < 1e-9);
        camera.orbit(40.0, 25.0);
        assert_eq!(camera.lens_length(), 100.0);
        // A saved view keeps it; one saved before the lens was kept opens at 50 mm.
        let saved = camera.saved("Long".into(), StandardView::Perspective);
        assert_eq!(Camera::from_saved(&saved).lens_length(), 100.0);
        let older = curva_document::SavedView {
            lens: None,
            ..saved
        };
        assert_eq!(Camera::from_saved(&older).lens_length(), LENS_LENGTH);
        // Kept within what makes sense, and nonsense ignored.
        camera.set_lens_length(1.0);
        assert_eq!(camera.lens_length(), 5.0);
        camera.set_angle_of_view(200.0);
        camera.set_lens_length(f64::NAN);
        assert_eq!(camera.lens_length(), 5.0);
    }

    /// A two-point view keeps verticals upright and parallel on screen, however it is turned:
    /// both ends of a vertical edge land at the same left-right place.
    #[test]
    fn a_two_point_view_keeps_verticals_parallel() {
        let size = [800.0, 600.0];
        let mut camera = StandardView::TwoPoint.camera();
        assert!(camera.is_two_point());
        for (dx, dy) in [(0.0, 0.0), (60.0, 80.0), (-150.0, -40.0)] {
            camera.orbit(dx, dy);
            for foot in [[30.0, 10.0, 0.0], [-20.0, 40.0, 0.0]] {
                let head = [foot[0], foot[1], 25.0];
                let (a, b) = (
                    camera.project(foot, size).unwrap(),
                    camera.project(head, size).unwrap(),
                );
                assert!((a[0] - b[0]).abs() < 1e-9, "{a:?} {b:?}");
            }
        }
        // An ordinary perspective view, looking down, leans them.
        let perspective = Camera::default();
        let a = perspective.project([30.0, 10.0, 0.0], size).unwrap();
        let b = perspective.project([30.0, 10.0, 25.0], size).unwrap();
        assert!((a[0] - b[0]).abs() > 1.0);
    }
}
