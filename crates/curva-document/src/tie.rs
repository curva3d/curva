//! Ties: where on an object a dimension's point was snapped, so the dimension follows the object
//! when it moves or changes (spec `annotation.md` §3, decision 0035).

use crate::annotation::Annotation;
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

/// Whether two points are the same, allowing for rounding.
fn same(a: Point, b: Point) -> bool {
    let size = a.iter().chain(&b).fold(1.0_f64, |m, v| m.max(v.abs()));
    curva_kernel::gap(a, b) <= 1e-9 * size
}

/// The points of an annotation that can be tied, in order: a linear dimension's two measured
/// points, an ordinate dimension's measured point; none for other annotation.
#[must_use]
pub fn tied_points(annotation: &Annotation) -> Vec<Point> {
    match annotation {
        Annotation::Dimension(d) => vec![d.first, d.second],
        Annotation::Ordinate(d) => vec![d.point],
        _ => Vec::new(),
    }
}

/// The annotation with its tied points, in `tied_points` order, moved to `points`.
fn with_tied_points(annotation: &Annotation, points: &[Point]) -> Annotation {
    let mut moved = annotation.clone();
    match (&mut moved, points) {
        (Annotation::Dimension(d), [first, second]) => {
            d.first = *first;
            d.second = *second;
        }
        (Annotation::Ordinate(d), [point]) => d.point = *point,
        _ => {}
    }
    moved
}

/// What becomes of one tied point: the tie as it now is, if it holds, and where the point goes.
fn follow(tie: Tie, point: Point, now: Option<Point>) -> (Option<Tie>, Point) {
    match now {
        // The object or the place on it is gone: the point stays, untied.
        None => (None, point),
        // The point is at the place (both moved together, or neither): the tie holds.
        Some(now) if same(point, now) => (Some(Tie { at: now, ..tie }), point),
        // The object changed and the point did not: the point follows.
        Some(now) if same(point, tie.at) => (Some(Tie { at: now, ..tie }), now),
        // The point was moved by itself: the tie is dropped, as in Rhino.
        Some(_) => (None, point),
    }
}

impl Document {
    /// Where a tie's place is now, if its object is still there with that part and place.
    #[must_use]
    pub fn tie_point(&self, tie: &Tie) -> Option<Point> {
        let object = self.state.objects.iter().find(|o| o.id == tie.object)?;
        place_of(&object.shape, tie.part, tie.place)
    }

    /// Notes the tie that came with the point being clicked, for a command to keep, or clears
    /// it once the click is handled.
    pub fn set_pick_tie(&mut self, tie: Option<Tie>) {
        self.pick_tie = tie;
    }

    /// The tie that came with the clicked point `at`, if there is one and the point was not moved
    /// after it snapped (by a typed distance, ortho or a guide).
    #[must_use]
    pub fn pick_tie_at(&self, at: Point) -> Option<Tie> {
        self.pick_tie
            .filter(|tie| same(tie.at, at))
            .map(|tie| Tie { at, ..tie })
    }

    /// Ties object `id`'s points, in `tied_points` order.
    pub fn set_ties(&mut self, id: u64, ties: Vec<Option<Tie>>) {
        let ties = if ties.iter().all(Option::is_none) {
            Vec::new()
        } else {
            ties
        };
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == id) {
            std::sync::Arc::make_mut(object).ties = ties;
        }
    }

    /// Where object `id`'s points are tied, in `tied_points` order; empty when none is.
    #[must_use]
    pub fn ties_of(&self, id: u64) -> Vec<Option<Tie>> {
        self.state
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.ties.clone())
            .unwrap_or_default()
    }

    /// Moves every tied dimension whose objects changed. A dimension whose tied points all
    /// moved the same way moves whole, its line and text with them; otherwise only its points
    /// move.
    pub(crate) fn refresh_ties(&mut self) {
        for index in 0..self.state.objects.len() {
            let object = &self.state.objects[index];
            if object.ties.iter().all(Option::is_none) {
                continue;
            }
            let Shape::Annotation(annotation) = &object.shape else {
                continue;
            };
            let points = tied_points(annotation);
            let mut ties = object.ties.clone();
            ties.resize(points.len(), None);
            let mut moved = points.clone();
            for (slot, tie) in ties.iter_mut().enumerate() {
                if let Some(held) = *tie {
                    (*tie, moved[slot]) = follow(held, points[slot], self.tie_point(&held));
                }
            }
            let shifts: Vec<Point> = points
                .iter()
                .zip(&moved)
                .map(|(p, m)| std::array::from_fn(|i| m[i] - p[i]))
                .collect();
            let whole = matches!(annotation, Annotation::Dimension(_))
                && shifts.iter().all(|s| same(*s, shifts[0]))
                && !same(shifts[0], [0.0; 3]);
            let changed = if whole {
                annotation.translated(shifts[0])
            } else {
                with_tied_points(annotation, &moved)
            };
            if ties.iter().all(Option::is_none) {
                ties.clear();
            }
            let shape_changed = changed != *annotation;
            if !shape_changed && ties == object.ties {
                continue;
            }
            let object = std::sync::Arc::make_mut(&mut self.state.objects[index]);
            if shape_changed {
                object.shape = Shape::Annotation(changed);
            }
            object.ties = ties;
        }
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

    /// A line from (0, 0) to (100, 0), and a horizontal dimension of it, tied to both its ends
    /// unless `ends` says otherwise.
    fn dimensioned(ends: [bool; 2]) -> (Document, u64, u64) {
        use crate::annotation::{Dimension, DimensionKind};
        let mut doc = Document::default();
        let line = doc.add(Curve::Line {
            start: [0.0, 0.0, 0.0],
            end: [100.0, 0.0, 0.0],
        });
        let dimension = doc.add_annotation(Annotation::Dimension(Dimension {
            kind: DimensionKind::Horizontal,
            first: [0.0, 0.0, 0.0],
            second: [100.0, 0.0, 0.0],
            line: [50.0, 20.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: doc.current_annotation_style(),
            text: None,
        }));
        let tie = |i: usize, at: Point| {
            ends[i].then_some(Tie {
                object: line,
                part: None,
                place: Place::Corner(i),
                at,
            })
        };
        doc.set_ties(
            dimension,
            vec![tie(0, [0.0, 0.0, 0.0]), tie(1, [100.0, 0.0, 0.0])],
        );
        (doc, line, dimension)
    }

    fn dimension_of(doc: &Document, id: u64) -> crate::annotation::Dimension {
        match &doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .shape
        {
            Shape::Annotation(Annotation::Dimension(d)) => d.clone(),
            _ => panic!("not a dimension"),
        }
    }

    #[test]
    fn a_dimension_moves_whole_with_its_moved_line() {
        let (mut doc, line, dimension) = dimensioned([true, true]);
        doc.translate(&[line], [10.0, 5.0, 0.0]);
        doc.refresh_history();
        let d = dimension_of(&doc, dimension);
        assert_eq!(d.first, [10.0, 5.0, 0.0]);
        assert_eq!(d.second, [110.0, 5.0, 0.0]);
        // Its line moves with it.
        assert_eq!(d.line, [60.0, 25.0, 0.0]);
        assert_eq!(doc.ties_of(dimension).len(), 2);
    }

    #[test]
    fn a_dimension_follows_its_line_stretched_and_scaled() {
        let (mut doc, line, dimension) = dimensioned([true, true]);
        doc.replace(
            line,
            Curve::Line {
                start: [0.0, 0.0, 0.0],
                end: [160.0, 0.0, 0.0],
            },
        );
        doc.refresh_history();
        let d = dimension_of(&doc, dimension);
        assert_eq!((d.first, d.second), ([0.0, 0.0, 0.0], [160.0, 0.0, 0.0]));
        // Only one end moved, so the dimension's line stays.
        assert_eq!(d.line, [50.0, 20.0, 0.0]);
        doc.transform(&[line], &curva_kernel::Similarity::scaling([0.0; 3], 0.5));
        doc.refresh_history();
        assert!((dimension_of(&doc, dimension).value() - 80.0).abs() < 1e-9);
    }

    #[test]
    fn an_untied_point_stays_where_it_was() {
        let (mut doc, line, dimension) = dimensioned([false, true]);
        doc.translate(&[line], [0.0, 10.0, 0.0]);
        doc.refresh_history();
        let d = dimension_of(&doc, dimension);
        assert_eq!(d.first, [0.0, 0.0, 0.0]);
        assert_eq!(d.second, [100.0, 10.0, 0.0]);
    }

    #[test]
    fn a_dimension_moved_with_its_line_stays_tied_and_one_moved_alone_lets_go() {
        let (mut doc, line, dimension) = dimensioned([true, true]);
        doc.translate(&[line, dimension], [0.0, 30.0, 0.0]);
        doc.refresh_history();
        assert_eq!(doc.ties_of(dimension).iter().flatten().count(), 2);
        doc.translate(&[dimension], [0.0, 30.0, 0.0]);
        doc.refresh_history();
        assert!(doc.ties_of(dimension).is_empty());
        // Untied, it no longer follows the line.
        doc.translate(&[line], [5.0, 0.0, 0.0]);
        doc.refresh_history();
        assert_eq!(dimension_of(&doc, dimension).first, [0.0, 60.0, 0.0]);
    }

    #[test]
    fn a_dimension_stays_when_its_line_is_deleted() {
        let (mut doc, line, dimension) = dimensioned([true, true]);
        doc.remove(&[line]);
        doc.refresh_history();
        assert!(doc.ties_of(dimension).is_empty());
        assert_eq!(dimension_of(&doc, dimension).second, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn a_point_tied_to_a_solid_edge_follows_the_solid() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 3.0, 2.0]).unwrap());
        let shape = doc.state().objects[0].shape.clone();
        let start = place_of(&shape, Some(0), Place::Start).unwrap();
        let end = place_of(&shape, Some(0), Place::End).unwrap();
        let dimension = doc.add_annotation(Annotation::Dimension(crate::annotation::Dimension {
            kind: crate::annotation::DimensionKind::Aligned,
            first: start,
            second: end,
            line: [-1.0, -1.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: doc.current_annotation_style(),
            text: None,
        }));
        let tie = |place, at| Tie {
            object: solid,
            part: Some(0),
            place,
            at,
        };
        doc.set_ties(
            dimension,
            vec![Some(tie(Place::Start, start)), Some(tie(Place::End, end))],
        );
        doc.translate(&[solid], [0.0, 0.0, 7.0]);
        doc.refresh_history();
        let d = dimension_of(&doc, dimension);
        assert!(curva_kernel::gap(d.first, [start[0], start[1], start[2] + 7.0]) < 1e-9);
        assert!(curva_kernel::gap(d.second, [end[0], end[1], end[2] + 7.0]) < 1e-9);
    }

    #[test]
    fn ties_are_saved_and_read_back() {
        let (mut doc, _, dimension) = dimensioned([true, false]);
        let folder = std::env::temp_dir().join(format!("curva-ties-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("ties.curva");
        doc.save(&file).unwrap();
        let back = Document::open(&file).unwrap();
        assert_eq!(back.ties_of(dimension), doc.ties_of(dimension));
        assert!(back.ties_of(dimension)[0].is_some());
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn a_picked_tie_counts_only_for_the_point_it_came_with() {
        let mut doc = Document::default();
        let tie = Tie {
            object: 1,
            part: None,
            place: Place::End,
            at: [1.0, 2.0, 0.0],
        };
        doc.set_pick_tie(Some(tie));
        assert_eq!(doc.pick_tie_at([1.0, 2.0, 0.0]), Some(tie));
        // A point moved after it snapped, by ortho or a typed distance, is not tied.
        assert_eq!(doc.pick_tie_at([1.0, 3.0, 0.0]), None);
    }
}
