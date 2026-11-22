//! Guidelines from held points, as Rhino's SmartTrack and ArchiCAD's guide lines send them out,
//! and the snaps they give: where they cross, where they cross an object, half way between held
//! points, and along one away from any crossing (#512, #547).

use super::*;

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
        for segment in self.segments_near(&view, (cursor, SNAP_REACH), &[]) {
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
        let nearest = |list: Vec<(SnapKind, Point)>| {
            list.into_iter()
                .filter_map(|(kind, point)| Some((gap(point)?, kind, point)))
                .filter(|(distance, ..)| *distance <= SNAP_REACH)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, kind, point)| SnapHit { kind, point })
        };
        // Where lines cross comes first; otherwise the pointer runs along the nearest guideline,
        // as Rhino's SmartTrack does (#512: it only ever snapped where lines crossed).
        nearest(candidates).or_else(|| {
            let on_lines = lines
                .iter()
                .filter_map(|(p, d)| camera.line_point(cursor, size, *p, *d))
                .map(|point| (SnapKind::Track, point))
                .collect();
            nearest(on_lines)
        })
    }
}
