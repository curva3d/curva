//! Rhino 8's Bevel for SubD edges. Measured (23 November 2026, RhinoCommon pick, `-Bevel 0.5`):
//! with its starting options (Segments=3, OffsetMode=Absolute, Straightness=0), the middle edge
//! of a 4 m SubD box's front, (202, 0, 0) to (202, 0, 2), is replaced by a strip of 3 quads; each
//! end point gives way to a row of 4 points from 0.5 along one side edge to 0.5 along the other
//! (x = 201.5, 201.8333, 202.1667 and 202.5), and a five-sided cap closes each end: the row and
//! the far point of the edge that carried on past it. 24 faces become 29 (27 quads, 2 five-sided)
//! on 32 points.

use crate::{Mesh, MeshError, Point, SubD};

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    std::array::from_fn(|i| a[i] + t * (b[i] - a[i]))
}

/// How far along each side edge a bevel's rows start: a distance, or a share of the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BevelAmount {
    /// A distance in model units.
    Absolute(f64),
    /// A share of each side edge, 0 to 1.
    Proportional(f64),
}

/// Where, in face `f`'s ring, the corner `v` sits.
fn corner(ring: &[u32], v: u32) -> Option<usize> {
    ring.iter().position(|&p| p == v)
}

/// The face other than `not` that has the edge `a`–`b`.
fn face_with(rings: &[Vec<u32>], a: u32, b: u32, not: usize) -> Option<usize> {
    rings.iter().enumerate().position(|(f, ring)| {
        let n = ring.len();
        f != not
            && (0..n).any(|k| {
                let (p, q) = (ring[k], ring[(k + 1) % n]);
                (p, q) == (a, b) || (p, q) == (b, a)
            })
    })
}

/// A bevel's working cage: its faces and points as they change.
struct Work {
    rings: Vec<Vec<u32>>,
    points: Vec<Point>,
}

/// How a bevel's rows are made.
#[derive(Clone, Copy)]
struct Shape {
    amount: BevelAmount,
    segments: usize,
    straightness: f64,
}

/// The point `amount` along the side edge from `v` to `s`.
fn side_point(points: &[Point], v: u32, s: u32, amount: BevelAmount) -> Point {
    let (pv, ps) = (points[v as usize], points[s as usize]);
    let length = (0..3).map(|i| (ps[i] - pv[i]).powi(2)).sum::<f64>().sqrt();
    let t = match amount {
        BevelAmount::Absolute(d) if length > 0.0 => (d / length).clamp(0.0, 0.95),
        BevelAmount::Absolute(_) => 0.5,
        BevelAmount::Proportional(t) => t.clamp(0.0, 0.95),
    };
    lerp(pv, ps, t)
}

impl Work {
    /// One end `v` of the bevelled edge (`first` for the edge's first point): its row of new
    /// points from the side by face `f1` to the side by `f2`, the four faces round it moved onto
    /// the row's ends, and its cap. `None` when `v` is not an inner point where four edges meet.
    fn end(
        &mut self,
        v: u32,
        first: bool,
        [f1, f2]: [usize; 2],
        uses: &std::collections::BTreeMap<(u32, u32), Vec<usize>>,
        shape: Shape,
    ) -> Option<(Vec<u32>, Vec<u32>)> {
        let (ring1, ring2) = (&self.rings[f1], &self.rings[f2]);
        let (n1, n2) = (ring1.len(), ring2.len());
        let (k1, k2) = (corner(ring1, v)?, corner(ring2, v)?);
        // The side point by f1 comes before the edge's first point in f1, after its second; by
        // f2 the other way round.
        let (s1, s2) = if first {
            (ring1[(k1 + n1 - 1) % n1], ring2[(k2 + 1) % n2])
        } else {
            (ring1[(k1 + 1) % n1], ring2[(k2 + n2 - 1) % n2])
        };
        let f3 = face_with(&self.rings, v, s1, f1)?;
        let f4 = face_with(&self.rings, v, s2, f2)?;
        let far = self.rings[f3].iter().copied().find(|&p| {
            p != v
                && p != s1
                && self.rings[f4].contains(&p)
                && uses.contains_key(&(p.min(v), p.max(v)))
        })?;
        let (p1, p2) = (
            side_point(&self.points, v, s1, shape.amount),
            side_point(&self.points, v, s2, shape.amount),
        );
        let bow = lerp(
            self.points[v as usize],
            lerp(p1, p2, 0.5),
            shape.straightness.clamp(0.0, 1.0),
        );
        let n = shape.segments;
        let row: Vec<u32> = (0..=n)
            .map(|k| {
                #[allow(clippy::cast_precision_loss)]
                let t = k as f64 / n as f64;
                let q: Point = std::array::from_fn(|i| {
                    (1.0 - t).powi(2) * p1[i] + 2.0 * t * (1.0 - t) * bow[i] + t * t * p2[i]
                });
                self.points.push(q);
                index(self.points.len() - 1)
            })
            .collect();
        for (f, r) in [(f1, row[0]), (f3, row[0]), (f2, row[n]), (f4, row[n])] {
            if let Some(k) = corner(&self.rings[f], v) {
                self.rings[f][k] = r;
            }
        }
        let mut cap: Vec<u32> = if first {
            row.iter().rev().copied().collect()
        } else {
            row.clone()
        };
        cap.push(far);
        Some((row, cap))
    }
}

impl SubD {
    /// The edge `[a, b]` bevelled, as Rhino's Bevel: a strip of `segments` quads in its place,
    /// each end point given way to a row from `amount` along one side edge to `amount` along the
    /// other, bowed towards where the point was (`straightness` 0) or straight (1), and a cap
    /// closing each end round the row and the far point of the edge that carried on past it.
    /// Both ends must be inner points where four edges meet.
    ///
    /// # Errors
    /// When an end is not such a point, or the bevelled cage is not a valid mesh.
    pub fn bevelled(
        &self,
        [a, b]: [u32; 2],
        amount: BevelAmount,
        segments: u32,
        straightness: f64,
    ) -> Result<Self, MeshError> {
        let cage = self.cage();
        let bad = || MeshError::UnusableEdge { edge: [a, b] };
        let mut work = Work {
            rings: cage.faces().map(<[u32]>::to_vec).collect(),
            points: cage.vertices().to_vec(),
        };
        let along = |ring: &[u32]| {
            let n = ring.len();
            (0..n).any(|k| (ring[k], ring[(k + 1) % n]) == (a, b))
        };
        let f1 = work.rings.iter().position(|r| along(r)).ok_or_else(bad)?;
        let f2 = face_with(&work.rings, a, b, f1).ok_or_else(bad)?;
        let uses = self.edge_uses();
        let valence = |v: u32| uses.keys().filter(|&&(p, q)| p == v || q == v).count();
        if valence(a) != 4 || valence(b) != 4 {
            return Err(bad());
        }
        let shape = Shape {
            amount,
            segments: segments.max(1) as usize,
            straightness,
        };
        let (ra, cap_a) = work.end(a, true, [f1, f2], &uses, shape).ok_or_else(bad)?;
        let (rb, cap_b) = work.end(b, false, [f1, f2], &uses, shape).ok_or_else(bad)?;
        let mut added: Vec<Vec<u32>> = (0..shape.segments)
            .map(|k| vec![rb[k], ra[k], ra[k + 1], rb[k + 1]])
            .collect();
        added.extend([cap_a, cap_b]);
        self.carried_to_without(work, added, [a, b])
    }

    /// The working cage, with `added` faces after its own, as a SubD without the points `gone`.
    fn carried_to_without(
        &self,
        work: Work,
        added: Vec<Vec<u32>>,
        gone: [u32; 2],
    ) -> Result<Self, MeshError> {
        let Work { mut rings, points } = work;
        let mut old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        old_faces.extend(std::iter::repeat_n(None, added.len()));
        rings.extend(added);
        let (lo, hi) = (gone[0].min(gone[1]), gone[0].max(gone[1]));
        let renumber = |p: u32| p - u32::from(p > lo) - u32::from(p > hi);
        let number: Vec<Option<u32>> = (0..self.cage().vertex_count())
            .map(|v| {
                let v = index(v);
                (!gone.contains(&v)).then(|| renumber(v))
            })
            .collect();
        let kept: Vec<Point> = points
            .iter()
            .enumerate()
            .filter(|&(v, _)| !gone.contains(&index(v)))
            .map(|(_, p)| *p)
            .collect();
        let rings: Vec<Vec<u32>> = rings
            .iter()
            .map(|ring| ring.iter().map(|&p| renumber(p)).collect())
            .collect();
        Ok(self.carried_to(Mesh::new(kept, &rings)?, &number, &old_faces))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    #[test]
    fn a_front_edge_bevels_as_rhino_bevels_it() {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let at = |p: Point| index(subd.cage().vertices().iter().position(|&q| q == p).unwrap());
        let edge = [at([202.0, 0.0, 0.0]), at([202.0, 0.0, 2.0])];
        let out = subd
            .bevelled(edge, BevelAmount::Absolute(0.5), 3, 0.0)
            .unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (29, 32)
        );
        assert_eq!(out.cage().faces().filter(|f| f.len() == 5).count(), 2);
        assert_eq!(out.cage().faces().filter(|f| f.len() == 4).count(), 27);
        assert!(out.cage().is_closed());
        let mut xs: Vec<f64> = out
            .cage()
            .vertices()
            .iter()
            .filter(|p| p[1] == 0.0 && (p[2] == 0.0 || p[2] == 2.0))
            .map(|p| (p[0] * 1e4).round() / 1e4)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        assert_eq!(
            xs,
            vec![200.0, 201.5, 201.8333, 202.1667, 202.5, 204.0],
            "Rhino's rows, the end points gone"
        );
    }
}
