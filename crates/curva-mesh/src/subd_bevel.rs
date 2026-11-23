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

/// The two side points of a run's point `v`: by the faces the run's edge `[p, q]` (holding `v`)
/// runs along, and by the faces it runs against.
fn sides(rings: &[Vec<u32>], v: u32, [p, q]: [u32; 2]) -> Option<[u32; 2]> {
    let holds = |ring: &[u32], x: u32, y: u32| {
        let n = ring.len();
        (0..n).any(|k| (ring[k], ring[(k + 1) % n]) == (x, y))
    };
    let along = rings.iter().find(|r| holds(r, p, q))?;
    let against = rings.iter().find(|r| holds(r, q, p))?;
    let (na, nb) = (along.len(), against.len());
    let (ka, kb) = (corner(along, v)?, corner(against, v)?);
    Some(if v == p {
        [along[(ka + na - 1) % na], against[(kb + 1) % nb]]
    } else {
        [along[(ka + 1) % na], against[(kb + nb - 1) % nb]]
    })
}

/// The points joined to `v` by an edge.
fn neighbours(uses: &std::collections::BTreeMap<(u32, u32), Vec<usize>>, v: u32) -> Vec<u32> {
    uses.keys()
        .filter_map(|&(p, q)| match (p == v, q == v) {
            (true, _) => Some(q),
            (_, true) => Some(p),
            _ => None,
        })
        .collect()
}

impl Work {
    /// The row of new points for `v`, from `amount` along the edge to `s1` to `amount` along the
    /// edge to `s2`, bowed towards `v`.
    fn row(&mut self, v: u32, [s1, s2]: [u32; 2], shape: Shape) -> Vec<u32> {
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
        (0..=n)
            .map(|k| {
                #[allow(clippy::cast_precision_loss)]
                let t = k as f64 / n as f64;
                let q: Point = std::array::from_fn(|i| {
                    (1.0 - t).powi(2) * p1[i] + 2.0 * t * (1.0 - t) * bow[i] + t * t * p2[i]
                });
                self.points.push(q);
                index(self.points.len() - 1)
            })
            .collect()
    }

    /// Every face corner at `v` moved onto its row: to the row's start in faces beside `s1`,
    /// to its end in faces beside `s2`.
    fn move_corners(&mut self, v: u32, [s1, s2]: [u32; 2], row: &[u32]) {
        let (first, last) = (row[0], row[row.len() - 1]);
        for ring in &mut self.rings {
            let n = ring.len();
            if let Some(k) = corner(ring, v) {
                let (before, after) = (ring[(k + n - 1) % n], ring[(k + 1) % n]);
                if before == s1 || after == s1 {
                    ring[k] = first;
                } else if before == s2 || after == s2 {
                    ring[k] = last;
                }
            }
        }
    }
}

impl SubD {
    /// The edge `[a, b]` bevelled, as Rhino's Bevel ([`SubD::bevelled_run`] of one edge).
    ///
    /// # Errors
    /// As [`SubD::bevelled_run`].
    pub fn bevelled(
        &self,
        edge: [u32; 2],
        amount: BevelAmount,
        segments: u32,
        straightness: f64,
    ) -> Result<Self, MeshError> {
        self.bevelled_run(&edge, amount, segments, straightness)
    }

    /// A run of edges bevelled, as Rhino's Bevel: `run` lists its points in order (a loop
    /// repeats its first at the end). Each point gives way to a row from `amount` along one side
    /// edge to `amount` along the other, bowed towards where the point was (`straightness` 0)
    /// or straight (1); rows next to each other are joined by strips of `segments` quads; an
    /// open run's ends are capped round their row and the far point of the edge that carried on
    /// past them. Measured: two edges in a row give 32 faces on 35 points; a loop round a box
    /// gives 48 quads on 50 points, with no caps. Every point must be an inner one where four
    /// edges meet.
    ///
    /// # Errors
    /// When a point is not such a point, or the bevelled cage is not a valid mesh.
    pub fn bevelled_run(
        &self,
        run: &[u32],
        amount: BevelAmount,
        segments: u32,
        straightness: f64,
    ) -> Result<Self, MeshError> {
        let cage = self.cage();
        let closed = run.len() > 3 && run.first() == run.last();
        let points: Vec<u32> = if closed {
            run[..run.len() - 1].to_vec()
        } else {
            run.to_vec()
        };
        let bad = || MeshError::UnusableEdge {
            edge: [run[0], run.get(1).copied().unwrap_or(run[0])],
        };
        let uses = self.edge_uses();
        if points.len() < 2 || points.iter().any(|&v| neighbours(&uses, v).len() != 4) {
            return Err(bad());
        }
        let mut work = Work {
            rings: cage.faces().map(<[u32]>::to_vec).collect(),
            points: cage.vertices().to_vec(),
        };
        let shape = Shape {
            amount,
            segments: segments.max(1) as usize,
            straightness,
        };
        let m = points.len();
        let edge_at = |i: usize| -> [u32; 2] {
            if i + 1 < m || closed {
                [points[i], points[(i + 1) % m]]
            } else {
                [points[i - 1], points[i]]
            }
        };
        let side_pairs: Vec<[u32; 2]> = (0..m)
            .map(|i| sides(&work.rings, points[i], edge_at(i)).ok_or_else(bad))
            .collect::<Result<_, _>>()?;
        // The far point past an open end: its fourth neighbour, found before any corner moves.
        let far = |i: usize, next: usize| {
            let [s1, s2] = side_pairs[i];
            neighbours(&uses, points[i])
                .into_iter()
                .find(|&w| w != points[next] && w != s1 && w != s2)
                .ok_or_else(bad)
        };
        let ends = if closed {
            None
        } else {
            Some((far(0, 1)?, far(m - 1, m - 2)?))
        };
        let rows: Vec<Vec<u32>> = (0..m)
            .map(|i| work.row(points[i], side_pairs[i], shape))
            .collect();
        for i in 0..m {
            work.move_corners(points[i], side_pairs[i], &rows[i]);
        }
        let joins = if closed { m } else { m - 1 };
        let mut added: Vec<Vec<u32>> = Vec::new();
        for i in 0..joins {
            let (ra, rb) = (&rows[i], &rows[(i + 1) % m]);
            added.extend((0..shape.segments).map(|k| vec![rb[k], ra[k], ra[k + 1], rb[k + 1]]));
        }
        if let Some((first, last)) = ends {
            let mut cap: Vec<u32> = rows[0].iter().rev().copied().collect();
            cap.push(first);
            added.push(cap);
            let mut cap = rows[m - 1].clone();
            cap.push(last);
            added.push(cap);
        }
        self.carried_to_without(work, added, &points)
    }

    /// The working cage, with `added` faces after its own, as a SubD without the points `gone`.
    fn carried_to_without(
        &self,
        work: Work,
        added: Vec<Vec<u32>>,
        gone: &[u32],
    ) -> Result<Self, MeshError> {
        let Work { mut rings, points } = work;
        let mut old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        old_faces.extend(std::iter::repeat_n(None, added.len()));
        rings.extend(added);
        let mut number: Vec<Option<u32>> = vec![None; points.len()];
        let mut kept: Vec<Point> = Vec::new();
        for (v, p) in points.iter().enumerate() {
            if !gone.contains(&index(v)) {
                number[v] = Some(index(kept.len()));
                kept.push(*p);
            }
        }
        let rings: Vec<Vec<u32>> = rings
            .iter()
            .map(|ring| ring.iter().filter_map(|&p| number[p as usize]).collect())
            .collect();
        number.truncate(self.cage().vertex_count());
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

    fn front_box() -> SubD {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])
    }

    fn at(subd: &SubD, p: Point) -> u32 {
        index(subd.cage().vertices().iter().position(|&q| q == p).unwrap())
    }

    #[test]
    fn two_edges_in_a_row_bevel_as_rhino_bevels_them() {
        let subd = front_box();
        let run: Vec<u32> = [[202.0, 0.0, 0.0], [202.0, 0.0, 2.0], [202.0, 0.0, 4.0]]
            .map(|p| at(&subd, p))
            .to_vec();
        let out = subd
            .bevelled_run(&run, BevelAmount::Absolute(0.5), 3, 0.0)
            .unwrap();
        let cage = out.cage();
        assert_eq!((cage.face_count(), cage.vertex_count()), (32, 35));
        assert_eq!(cage.faces().filter(|f| f.len() == 5).count(), 2);
        assert!(cage.is_closed());
        for z in [0.0, 2.0, 4.0] {
            let row = cage
                .vertices()
                .iter()
                .filter(|p| p[1] == 0.0 && (p[2] - z).abs() < 1e-9 && p[0] > 201.0 && p[0] < 203.0)
                .count();
            assert_eq!(row, 4, "Rhino's row of 4 at z = {z}");
        }
    }

    #[test]
    fn a_loop_round_the_box_bevels_without_caps() {
        let subd = front_box();
        let mut run: Vec<u32> = [
            [202.0, 0.0, 0.0],
            [202.0, 0.0, 2.0],
            [202.0, 0.0, 4.0],
            [202.0, 2.0, 4.0],
            [202.0, 4.0, 4.0],
            [202.0, 4.0, 2.0],
            [202.0, 4.0, 0.0],
            [202.0, 2.0, 0.0],
        ]
        .map(|p| at(&subd, p))
        .to_vec();
        run.push(run[0]);
        let out = subd
            .bevelled_run(&run, BevelAmount::Absolute(0.5), 3, 0.0)
            .unwrap();
        let cage = out.cage();
        assert_eq!((cage.face_count(), cage.vertex_count()), (48, 50));
        assert!(cage.faces().all(|f| f.len() == 4));
        assert!(cage.is_closed());
    }
}
