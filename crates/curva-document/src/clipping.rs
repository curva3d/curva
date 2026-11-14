//! Clipping planes (spec `layouts.md` §3): a rectangle in the model that cuts away everything on
//! its back side in the views it is enabled in, with closed solids it cuts filled by its section
//! style. The rectangle is an ordinary curve, so it moves, turns and is picked like any other;
//! this holds what it does.

use crate::Document;
use curva_kernel::Point;
use serde::{Deserialize, Serialize};

/// A view a clipping plane can cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipView {
    /// One of the model's viewports, by its place (0 to 3).
    Viewport(usize),
    /// A detail on a layout, by its frame.
    Detail(u64),
}

/// How the cut faces of closed solids are filled.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SectionFill {
    /// Left open: the cut shows the inside.
    None,
    /// Filled with a colour, red, green and blue from 0 to 1.
    Solid([f32; 3]),
}

impl Default for SectionFill {
    fn default() -> Self {
        Self::Solid([0.55, 0.56, 0.6])
    }
}

/// A clipping plane's record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClippingPlane {
    /// The rectangle that is the plane.
    pub frame: u64,
    /// Whether it keeps the other side.
    pub flipped: bool,
    /// The views it cuts.
    pub views: Vec<ClipView>,
    /// How it fills the cut faces of closed solids.
    pub section: SectionFill,
}

/// A clipping plane where it lies now: a point on it, and the direction of the side it keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneCut {
    /// The rectangle that is the plane.
    pub frame: u64,
    /// A point on the plane.
    pub origin: Point,
    /// The side kept, of length 1.
    pub keep: Point,
    /// How it fills the cut faces of closed solids.
    pub section: SectionFill,
}

impl PlaneCut {
    /// The plane as the graphics card cuts with it: `[nx, ny, nz, d]`, cutting away where
    /// `n·p + d > 0`.
    #[must_use]
    pub fn for_drawing(&self) -> [f32; 4] {
        let n = self.keep.map(|v| -v);
        let d = (0..3).map(|i| self.keep[i] * self.origin[i]).sum::<f64>();
        #[allow(clippy::cast_possible_truncation)]
        [n[0] as f32, n[1] as f32, n[2] as f32, d as f32]
    }

    /// Whether a point is cut away.
    #[must_use]
    pub fn cuts(&self, p: Point) -> bool {
        (0..3)
            .map(|i| self.keep[i] * (p[i] - self.origin[i]))
            .sum::<f64>()
            < 0.0
    }
}

/// The plane of a closed polygon, by Newell's method: its middle and its unit normal.
fn plane_of(points: &[Point]) -> Option<(Point, Point)> {
    if points.len() < 3 {
        return None;
    }
    let mut normal = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length < 1e-12 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let count = points.len() as f64;
    let middle = std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / count);
    Some((middle, normal.map(|v| v / length)))
}

impl Document {
    /// Adds a clipping plane's record for the rectangle `frame`.
    pub fn add_clipping_plane(&mut self, plane: ClippingPlane) {
        self.state.clipping_planes.push(plane);
    }

    /// The clipping plane made by the rectangle `frame`.
    #[must_use]
    pub fn clipping_plane_of(&self, frame: u64) -> Option<&ClippingPlane> {
        self.state.clipping_planes.iter().find(|c| c.frame == frame)
    }

    /// Changes the clipping plane made by the rectangle `frame`.
    pub fn change_clipping_plane(&mut self, frame: u64, change: impl FnOnce(&mut ClippingPlane)) {
        if let Some(plane) = self
            .state
            .clipping_planes
            .iter_mut()
            .find(|c| c.frame == frame)
        {
            change(plane);
        }
    }

    /// The clipping planes cutting `view`, where they lie now. A plane whose rectangle is
    /// gone, hidden or not flat cuts nothing.
    #[must_use]
    pub fn planes_cutting(&self, view: ClipView) -> Vec<PlaneCut> {
        self.state
            .clipping_planes
            .iter()
            .filter(|c| c.views.contains(&view))
            .filter_map(|c| {
                let frame = self.state.objects.iter().find(|o| o.id == c.frame)?;
                if frame.hidden {
                    return None;
                }
                let mut points = frame.curve()?.sample().ok()?;
                // A closed curve's samples end where they start.
                if points.len() > 3 && points.first() == points.last() {
                    points.pop();
                }
                let (origin, normal) = plane_of(&points)?;
                let keep = if c.flipped {
                    normal.map(|v| -v)
                } else {
                    normal
                };
                Some(PlaneCut {
                    frame: c.frame,
                    origin,
                    keep,
                    section: c.section,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    fn square(z: f64) -> Curve {
        Curve::polyline(
            vec![
                [0.0, 0.0, z],
                [10.0, 0.0, z],
                [10.0, 10.0, z],
                [0.0, 10.0, z],
            ],
            true,
        )
        .unwrap()
    }

    #[test]
    fn a_plane_cuts_away_its_back_side_in_the_views_it_is_on_in() {
        let mut doc = Document::default();
        let frame = doc.add(square(5.0));
        doc.add_clipping_plane(ClippingPlane {
            frame,
            flipped: false,
            views: vec![ClipView::Viewport(1)],
            section: SectionFill::default(),
        });
        assert!(doc.planes_cutting(ClipView::Viewport(0)).is_empty());
        let [cut] = doc.planes_cutting(ClipView::Viewport(1))[..] else {
            panic!("one plane");
        };
        // Drawn counter-clockwise from above, it keeps what is above it.
        assert!((cut.keep[2] - 1.0).abs() < 1e-9);
        assert!(cut.cuts([5.0, 5.0, 0.0]));
        assert!(!cut.cuts([5.0, 5.0, 9.0]));
        let [nx, ny, nz, d] = cut.for_drawing();
        let cut_away = |p: [f32; 3]| nx * p[0] + ny * p[1] + nz * p[2] + d > 0.0;
        assert!(cut_away([5.0, 5.0, 0.0]) && !cut_away([5.0, 5.0, 9.0]));
        doc.change_clipping_plane(frame, |c| c.flipped = true);
        assert!(!doc.planes_cutting(ClipView::Viewport(1))[0].cuts([5.0, 5.0, 0.0]));
    }

    #[test]
    fn a_hidden_or_deleted_plane_cuts_nothing() {
        let mut doc = Document::default();
        let frame = doc.add(square(0.0));
        doc.add_clipping_plane(ClippingPlane {
            frame,
            flipped: false,
            views: vec![ClipView::Detail(7)],
            section: SectionFill::None,
        });
        assert_eq!(doc.planes_cutting(ClipView::Detail(7)).len(), 1);
        doc.hide(&[frame]);
        assert!(doc.planes_cutting(ClipView::Detail(7)).is_empty());
        doc.show_all();
        doc.remove(&[frame]);
        assert!(doc.planes_cutting(ClipView::Detail(7)).is_empty());
    }
}
