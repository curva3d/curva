//! Moving the corners, edges and faces of a straight-sided surface or solid, as Rhino moves
//! sub-objects picked with Ctrl+Shift: the picked parts' corners move and every face meeting
//! them follows, so a solid stays closed. A face whose corners stay in one plane stays flat; a
//! four-cornered face that no longer does becomes a surface through its four edges, as Rhino
//! bends it.

use crate::{Body, Curve, KernelError, Point};
use solidcore_record::brep::{Curve as RecordCurve, Loop, Surface};

/// How far, as a share of a face's size, a corner may stray from the face's plane and the face
/// still count as flat.
const FLAT: f64 = 1e-9;

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Whether the points all lie in one plane, within [`FLAT`] of their spread.
fn flat(points: &[Point]) -> bool {
    let Some(&first) = points.first() else {
        return true;
    };
    let size = points
        .iter()
        .map(|p| crate::gap(*p, first))
        .fold(0.0, f64::max);
    // Newell's normal, which suits any polygon, convex or not.
    let mut normal = [0.0; 3];
    for k in 0..points.len() {
        let (a, b) = (points[k], points[(k + 1) % points.len()]);
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = dot(normal, normal).sqrt();
    if length <= f64::EPSILON * size * size {
        // No area: the points lie along a line, which is in a plane.
        return true;
    }
    let normal = normal.map(|v| v / length);
    points
        .iter()
        .all(|p| dot(sub(*p, first), normal).abs() <= FLAT.max(1e-12) * size.max(1.0))
}

impl Body {
    /// The body's corners, by number, as its edges name them.
    pub fn corner_points(&self) -> Result<Vec<Point>, KernelError> {
        Ok(self.record()?.vertices.iter().map(|v| v.point).collect())
    }

    /// The corners at the ends of the edges numbered `edges`.
    pub fn corners_of_edges(&self, edges: &[usize]) -> Result<Vec<usize>, KernelError> {
        let record = self.record()?;
        let mut corners = Vec::new();
        for &edge in edges {
            let edge = record
                .edges
                .get(edge)
                .ok_or_else(|| bad("the body has no such edge"))?;
            corners.extend(edge.vertices.iter().map(|v| *v as usize));
        }
        corners.sort_unstable();
        corners.dedup();
        Ok(corners)
    }

    /// The corners round the faces numbered `faces`.
    pub fn corners_of_faces(&self, faces: &[usize]) -> Result<Vec<usize>, KernelError> {
        let record = self.record()?;
        let mut edges = Vec::new();
        for &face in faces {
            let face = record
                .faces
                .get(face)
                .ok_or_else(|| bad("the body has no such face"))?;
            for boundary in &face.loops {
                match boundary {
                    Loop::Edges { edges: run } => edges.extend(run.iter().map(|e| e.edge as usize)),
                    Loop::Vertex { .. } => {}
                }
            }
        }
        self.corners_of_edges(&edges)
    }

    /// Whether every face is flat and every edge straight, so its corners can be moved.
    pub fn is_straight_sided(&self) -> Result<bool, KernelError> {
        let record = self.record()?;
        Ok(record
            .faces
            .iter()
            .all(|face| matches!(face.surface, Surface::Plane { .. }))
            && record.edges.iter().all(|edge| {
                matches!(
                    edge.nurbs.as_ref().unwrap_or(&edge.curve),
                    RecordCurve::Line { .. } | RecordCurve::Polyline { .. }
                )
            }))
    }

    /// The body with the corners numbered `corners` moved by `change` and every face meeting
    /// them following (Rhino's sub-object editing). Only straight-sided bodies can be changed
    /// this way; a face with more than four corners must stay flat.
    pub fn with_corners_moved(
        &self,
        corners: &[usize],
        change: impl Fn(Point) -> Point,
    ) -> Result<Self, KernelError> {
        if corners.is_empty() {
            return Ok(self.clone());
        }
        if !self.is_straight_sided()? {
            return Err(bad(
                "only parts of surfaces and solids with flat faces and straight edges can be moved so far",
            ));
        }
        let record = self.record()?;
        let points: Vec<Point> = record
            .vertices
            .iter()
            .enumerate()
            .map(|(k, v)| {
                if corners.contains(&k) {
                    change(v.point)
                } else {
                    v.point
                }
            })
            .collect();
        let start_of = |used: &solidcore_record::brep::OrientedEdge| -> Result<usize, KernelError> {
            let edge = record
                .edges
                .get(used.edge as usize)
                .ok_or_else(|| bad("a face names a missing edge"))?;
            Ok(edge.vertices[usize::from(!used.forward)] as usize)
        };
        let mut faces = Vec::new();
        for face in &record.faces {
            let mut rings: Vec<Vec<Point>> = Vec::new();
            for boundary in &face.loops {
                if let Loop::Edges { edges } = boundary {
                    let ring = edges
                        .iter()
                        .map(|used| start_of(used).map(|v| points[v]))
                        .collect::<Result<Vec<_>, _>>()?;
                    rings.push(ring);
                }
            }
            let Some(outer) = rings.first() else {
                continue;
            };
            let all: Vec<Point> = rings.iter().flatten().copied().collect();
            if flat(&all) {
                let curves = rings
                    .iter()
                    .map(|ring| Curve::polyline(ring.clone(), true))
                    .collect::<Result<Vec<_>, _>>()?;
                faces.push(Body::planar(&curves)?);
            } else if rings.len() == 1 && (3..=4).contains(&outer.len()) {
                let sides = (0..outer.len())
                    .map(|k| Curve::line(outer[k], outer[(k + 1) % outer.len()]))
                    .collect::<Result<Vec<_>, _>>()?;
                faces.push(Body::from_edges(&sides)?);
            } else {
                return Err(bad(
                    "the move would bend a face with more than four corners or a hole",
                ));
            }
        }
        let made = Body::assembled(&faces)?;
        if self.is_solid() && !made.is_solid() {
            return Err(bad("the move would open the solid"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Body;

    fn up(p: crate::Point) -> crate::Point {
        [p[0], p[1], p[2] + 1.0]
    }

    #[test]
    fn moving_a_box_top_face_up_makes_it_taller_and_keeps_it_closed() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let points = cube.corner_points().unwrap();
        let top: Vec<usize> = (0..points.len()).filter(|k| points[*k][2] > 0.5).collect();
        assert_eq!(top.len(), 4);
        let taller = cube.with_corners_moved(&top, up).unwrap();
        assert!(taller.is_solid());
        assert!((taller.volume().unwrap() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn moving_one_corner_bends_its_faces_and_the_box_stays_closed() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let points = cube.corner_points().unwrap();
        let corner = points
            .iter()
            .position(|p| p.iter().all(|v| *v > 0.5))
            .unwrap();
        let bent = cube.with_corners_moved(&[corner], up).unwrap();
        assert!(bent.is_solid());
        assert!(bent.volume().unwrap() > 1.0);
    }

    #[test]
    fn faces_and_edges_name_their_corners() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        assert_eq!(cube.corners_of_edges(&[0]).unwrap().len(), 2);
        assert_eq!(cube.corners_of_faces(&[0]).unwrap().len(), 4);
        assert_eq!(cube.corner_points().unwrap().len(), 8);
    }

    #[test]
    fn a_curved_body_says_its_parts_cannot_move_yet() {
        let ball = Body::sphere([0.0; 3], 1.0).unwrap();
        assert!(!ball.is_straight_sided().unwrap());
        assert!(ball.with_corners_moved(&[0], up).is_err());
    }
}
