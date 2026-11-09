//! Curva's 3D viewport: what to draw, how to pick it, and drawing it on the graphics card.

mod camera;
mod gpu;

use curva_document::State;
use curva_kernel::Point;

pub use camera::Camera;
pub use gpu::{Drawing, LineRenderer, save_picture};

const GRID_HALF_SIZE: i32 = 100;
const GRID_STEP: i32 = 10;
const GRID_COLOUR: [f32; 3] = [0.30, 0.31, 0.34];
const CURVE_COLOUR: [f32; 3] = [0.92, 0.92, 0.95];
const SELECTED_COLOUR: [f32; 3] = [1.0, 0.85, 0.2];
const PREVIEW_COLOUR: [f32; 3] = [0.45, 0.75, 1.0];
const X_AXIS_COLOUR: [f32; 3] = [0.85, 0.3, 0.3];
const Y_AXIS_COLOUR: [f32; 3] = [0.35, 0.8, 0.4];

/// One end of a drawn line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in model space.
    pub position: [f32; 3],
    /// Colour, red, green and blue from 0 to 1.
    pub colour: [f32; 3],
}

/// The drawable outline of each object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    /// Object id and points along it.
    pub objects: Vec<(u64, Vec<Point>)>,
}

impl Outlines {
    /// Samples every object. An object that cannot be sampled is left out, not fatal to drawing.
    #[must_use]
    pub fn of(state: &State) -> Self {
        Self {
            objects: state
                .objects
                .iter()
                .filter_map(|object| Some((object.id, object.curve.sample().ok()?)))
                .collect(),
        }
    }

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
        for (id, points) in &self.objects {
            let projected: Vec<Option<[f64; 2]>> =
                points.iter().map(|p| camera.project(*p, size)).collect();
            for pair in projected.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    let distance = to_segment(screen, *a, *b);
                    if distance <= reach && best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, *id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }
}

fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length < 1e-12 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn segment(out: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], colour: [f32; 3]) {
    out.push(Vertex {
        position: a,
        colour,
    });
    out.push(Vertex {
        position: b,
        colour,
    });
}

fn narrow(p: Point) -> [f32; 3] {
    p.map(|value| value as f32)
}

fn polyline(out: &mut Vec<Vertex>, points: &[Point], colour: [f32; 3]) {
    for pair in points.windows(2) {
        segment(out, narrow(pair[0]), narrow(pair[1]), colour);
    }
}

/// The grid, axes and objects as line segments.
#[must_use]
pub fn scene_lines(outlines: &Outlines, selected: &[u64]) -> Vec<Vertex> {
    let mut out = Vec::new();
    let reach = GRID_HALF_SIZE as f32;
    for step in (-GRID_HALF_SIZE..=GRID_HALF_SIZE).step_by(GRID_STEP as usize) {
        let at = step as f32;
        if step != 0 {
            segment(&mut out, [at, -reach, 0.0], [at, reach, 0.0], GRID_COLOUR);
            segment(&mut out, [-reach, at, 0.0], [reach, at, 0.0], GRID_COLOUR);
        }
    }
    segment(
        &mut out,
        [-reach, 0.0, 0.0],
        [reach, 0.0, 0.0],
        X_AXIS_COLOUR,
    );
    segment(
        &mut out,
        [0.0, -reach, 0.0],
        [0.0, reach, 0.0],
        Y_AXIS_COLOUR,
    );
    for (id, points) in &outlines.objects {
        let colour = if selected.contains(id) {
            SELECTED_COLOUR
        } else {
            CURVE_COLOUR
        };
        polyline(&mut out, points, colour);
    }
    out
}

/// The live preview through a command's points, ending at the cursor.
#[must_use]
pub fn preview_lines(points: &[Point], cursor: Option<Point>) -> Vec<Vertex> {
    let mut all = points.to_vec();
    if !all.is_empty() {
        all.extend(cursor);
    }
    let mut out = Vec::new();
    polyline(&mut out, &all, PREVIEW_COLOUR);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::Document;
    use curva_kernel::Curve;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn scene() -> (Outlines, u64, u64) {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::circle([0.0, 40.0, 0.0], 5.0).unwrap());
        (Outlines::of(doc.state()), a, b)
    }

    #[test]
    fn clicking_on_a_curve_picks_it() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let on_line = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, on_line, SIZE, 6.0), Some(line));
    }

    #[test]
    fn clicking_on_nothing_picks_nothing() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let empty = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, empty, SIZE, 6.0), None);
    }

    #[test]
    fn selected_objects_change_colour() {
        let (outlines, line, _) = scene();
        let plain = scene_lines(&outlines, &[]);
        let marked = scene_lines(&outlines, &[line]);
        assert_eq!(plain.len(), marked.len());
        assert!(marked.iter().any(|v| v.colour == SELECTED_COLOUR));
        assert!(!plain.iter().any(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn bounds_surround_everything() {
        let (outlines, _, _) = scene();
        let (low, high) = outlines.bounds().unwrap();
        assert!(low[0] <= -20.0 && high[0] >= 20.0 && high[1] >= 45.0 - 1e-6);
    }

    #[test]
    fn preview_runs_through_its_points_to_the_cursor() {
        let lines = preview_lines(&[[0.0; 3], [1.0, 0.0, 0.0]], Some([2.0, 0.0, 0.0]));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3].position, [2.0, 0.0, 0.0]);
        assert!(preview_lines(&[], Some([2.0, 0.0, 0.0])).is_empty());
    }
}
