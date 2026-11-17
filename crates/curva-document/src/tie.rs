//! Ties: where on an object a dimension's point was snapped, so the dimension follows the object
//! when it moves or changes (spec `annotation.md` §3, decision 0035).

use crate::{Document, Shape};
use curva_kernel::{Curve, Point};
use serde::{Deserialize, Serialize};

/// A place on a single curve that a point can be tied to: the places snapping offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// Where the curve starts.
    Start,
    /// Where it ends.
    End,
    /// Half way along it.
    Mid,
    /// The centre of a circle, an arc or an ellipse.
    Center,
    /// A corner of a line or polyline, by its number among the curve's points (a point object
    /// has one, number 0).
    Corner(usize),
    /// The middle of a line's or polyline's segment, by number: segment `i` runs from corner `i`
    /// to the next.
    SegmentMid(usize),
}

/// Where a point is tied: an object, the part of it (a joined curve's segment, or an edge of a
/// surface or solid), the place on that part, and where that place was when last seen.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tie {
    /// The object's id.
    pub object: u64,
    /// For a joined curve, which segment; for a surface or solid, which edge, numbered as
    /// `Body::edge_curves` lists them; `None` for the whole of a single curve.
    pub part: Option<usize>,
    /// The place on the curve or part.
    pub place: Place,
    /// Where the place was when the tie was made or last followed.
    pub at: Point,
}

/// The middle of the segment from `a` to `b`.
fn middle(a: Point, b: Point) -> Point {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ]
}

/// The corners of a line, a polyline or a point object, and whether it closes back to the
/// first; `None` for other curves.
fn corners(curve: &Curve) -> Option<(Vec<Point>, bool)> {
    match curve {
        Curve::Point { at } => Some((vec![*at], false)),
        Curve::Line { start, end } => Some((vec![*start, *end], false)),
        Curve::Polyline { points, closed } => Some((points.clone(), *closed)),
        _ => None,
    }
}

/// Where `place` is on a single curve, if the curve has it.
#[must_use]
pub fn place_on(curve: &Curve, place: Place) -> Option<Point> {
    match place {
        Place::Start => curve.sample().ok()?.first().copied(),
        Place::End => curve.sample().ok()?.last().copied(),
        Place::Mid => curve.divided(2).ok()?.get(1).copied(),
        Place::Center => match curve {
            Curve::Circle { centre, .. } | Curve::Arc { centre, .. } => Some(*centre),
            _ => curve.as_ellipse().map(|e| e.centre),
        },
        Place::Corner(i) => corners(curve)?.0.get(i).copied(),
        Place::SegmentMid(i) => {
            let (points, closed) = corners(curve)?;
            let segments = if closed {
                points.len()
            } else {
                points.len().saturating_sub(1)
            };
            (i < segments).then(|| middle(points[i], points[(i + 1) % points.len()]))
        }
    }
}

/// Where a tie's place is on a shape now, if the shape still has that part and place.
#[must_use]
pub fn place_of(shape: &Shape, part: Option<usize>, place: Place) -> Option<Point> {
    match (shape, part) {
        (Shape::Curve(curve), None) => place_on(curve, place),
        (Shape::Curve(Curve::Compound { segments }), Some(i)) => place_on(segments.get(i)?, place),
        (Shape::Body(body), Some(i)) => place_on(body.edge_curves().ok()?.get(i)?, place),
        _ => None,
    }
}

impl Document {
    /// Where a tie's place is now, if its object is still there with that part and place.
    #[must_use]
    pub fn tie_point(&self, tie: &Tie) -> Option<Point> {
        let object = self.state.objects.iter().find(|o| o.id == tie.object)?;
        place_of(&object.shape, tie.part, tie.place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_on_lines_polylines_arcs_and_circles() {
        let polyline = Curve::Polyline {
            points: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 2.0, 0.0]],
            closed: true,
        };
        assert_eq!(place_on(&polyline, Place::Corner(1)), Some([4.0, 0.0, 0.0]));
        assert_eq!(
            place_on(&polyline, Place::SegmentMid(0)),
            Some([2.0, 0.0, 0.0])
        );
        // A closed polyline's last segment returns to the first corner.
        assert_eq!(
            place_on(&polyline, Place::SegmentMid(2)),
            Some([2.0, 1.0, 0.0])
        );
        assert_eq!(place_on(&polyline, Place::SegmentMid(3)), None);
        assert_eq!(place_on(&polyline, Place::Corner(3)), None);
        let arc = Curve::Arc {
            centre: [1.0, 1.0, 0.0],
            radius: 2.0,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            sweep: std::f64::consts::PI,
        };
        let near = |a: Option<Point>, b: Point| {
            let a = a.expect("a place");
            assert!(curva_kernel::gap(a, b) < 1e-9, "{a:?} is not {b:?}");
        };
        near(place_on(&arc, Place::Start), [3.0, 1.0, 0.0]);
        near(place_on(&arc, Place::End), [-1.0, 1.0, 0.0]);
        near(place_on(&arc, Place::Mid), [1.0, 3.0, 0.0]);
        near(place_on(&arc, Place::Center), [1.0, 1.0, 0.0]);
        let line = Curve::Line {
            start: [0.0, 0.0, 0.0],
            end: [2.0, 0.0, 0.0],
        };
        assert_eq!(place_on(&line, Place::Center), None);
    }

    #[test]
    fn a_tie_finds_its_place_on_the_object_and_none_once_it_is_gone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::Line {
            start: [0.0, 0.0, 0.0],
            end: [2.0, 0.0, 0.0],
        });
        let tie = Tie {
            object: id,
            part: None,
            place: Place::End,
            at: [2.0, 0.0, 0.0],
        };
        assert_eq!(doc.tie_point(&tie), Some([2.0, 0.0, 0.0]));
        // A single curve has no parts.
        assert_eq!(
            doc.tie_point(&Tie {
                part: Some(0),
                ..tie
            }),
            None
        );
        doc.remove(&[id]);
        assert_eq!(doc.tie_point(&tie), None);
    }
}
