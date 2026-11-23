//! Rhino 8's InsertEdge with `Type=Loop`: edges inserted beside a run of cage edges. Measured
//! through the screen (22 November 2026, `docs/commands/rhino-reference.md`): a click picks one
//! edge, and `Mode=Full` runs it on through points where four edges meet to the next point where
//! they do not; the new run sits beside it at the amount given along each crossing edge, the faces
//! it crosses split in two, and the faces beyond its ends get its end points (on a 4 m SubD box of
//! 2 × 2 a side, a corner edge and 0.5 give 3 new points, 26 faces, two of them five-sided).

use std::collections::{BTreeMap, BTreeSet};

use crate::{Mesh, MeshError, Point, SubD};

/// How far from the run the new edges go: a distance along each crossing edge, or a share of
/// its length (Rhino's `OffsetMode=Absolute` and `Proportional`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InsertAmount {
    /// A distance in model units, kept a little short of the crossing edge's far end.
    Absolute(f64),
    /// A share of each crossing edge, from 0 to 1.
    Proportional(f64),
}

/// Which side of the run gets the new edges: the faces that run along it the way it goes, the
/// faces that run against it, or both (Rhino's `BothSides`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertSide {
    /// The faces whose corners run the way the run does.
    Along,
    /// The faces whose corners run against it.
    Against,
    /// Both.
    Both,
}

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Each face not in `gone` given the new point of any of its edges that `new_point` splits, between
/// the edge's two ends.
fn give_new_points(
    rings: &mut [Vec<u32>],
    gone: &BTreeSet<usize>,
    new_point: &BTreeMap<(u32, u32), u32>,
) {
    for (f, ring) in rings.iter_mut().enumerate() {
        if gone.contains(&f) {
            continue;
        }
        let mut with = Vec::with_capacity(ring.len() + 2);
        for k in 0..ring.len() {
            let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
            with.push(p);
            if let Some(&c) = new_point.get(&(p, q)).or_else(|| new_point.get(&(q, p))) {
                with.push(c);
            }
        }
        *ring = with;
    }
}

impl SubD {
    /// The run Rhino's InsertEdge `Mode=Full` makes of one edge: on from each end through points
    /// where exactly four edges meet, across to the edge opposite the one it came along, until a
    /// point where they do not, or the border; a run that comes back to its start is a loop and
    /// repeats its first point at the end.
    #[must_use]
    pub fn edge_run(&self, [a, b]: [u32; 2]) -> Vec<u32> {
        let uses = self.edge_uses();
        let mut around: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for &(p, q) in uses.keys() {
            around.entry(p).or_default().insert(q);
            around.entry(q).or_default().insert(p);
        }
        let faces: Vec<Vec<u32>> = self.cage().faces().map(<[u32]>::to_vec).collect();
        // The edge across `here` from `from`: the one of four sharing no face with it.
        let across = |from: u32, here: u32| -> Option<u32> {
            let next = around.get(&here)?;
            let inner = uses.get(&key(here, from)).is_some_and(|f| f.len() == 2);
            if next.len() != 4 || !inner {
                return None;
            }
            let beside: BTreeSet<u32> = uses[&key(here, from)]
                .iter()
                .flat_map(|&f| faces[f].iter().copied())
                .collect();
            let mut left = next.iter().filter(|&&n| n != from && !beside.contains(&n));
            match (left.next(), left.next()) {
                (Some(&n), None) if uses.get(&key(here, n)).is_some_and(|f| f.len() == 2) => {
                    Some(n)
                }
                _ => None,
            }
        };
        let mut forward = vec![a, b];
        while let Some(n) = across(forward[forward.len() - 2], forward[forward.len() - 1]) {
            forward.push(n);
            if n == a || forward.len() > around.len() + 1 {
                return forward;
            }
        }
        let mut backward = vec![b, a];
        while let Some(n) = across(backward[backward.len() - 2], backward[backward.len() - 1]) {
            if forward.contains(&n) {
                break;
            }
            backward.push(n);
        }
        backward.reverse();
        backward.pop();
        backward.pop();
        backward.extend(forward);
        backward
    }

    /// New edges beside the run [`SubD::edge_run`] makes of `edge`, as Rhino's InsertEdge: on
    /// `side`, a point on each crossing edge at `amount`, each face the run borders there split
    /// into two, and the faces beyond the run's ends given its end points. The run's faces on
    /// that side must be quads.
    ///
    /// # Errors
    /// When a face beside the run is not a quad, or the new cage is not a valid mesh.
    pub fn with_edges_inserted(
        &self,
        edge: [u32; 2],
        amount: InsertAmount,
        side: InsertSide,
    ) -> Result<Self, MeshError> {
        self.with_edges_inserted_on(&self.edge_run(edge), amount, side)
    }

    /// The part of `first`'s run ([`SubD::edge_run`]) from `first` to `last`, both included, as
    /// Rhino's InsertEdge `Mode=Range` takes the edges between two picked ones (measured: the
    /// two middle edges of a box side give just those two, where `Full` runs round the box).
    /// `None` when `last` is not on that run.
    #[must_use]
    pub fn edge_range(&self, first: [u32; 2], last: [u32; 2]) -> Option<Vec<u32>> {
        let run = self.edge_run(first);
        let at = |[p, q]: [u32; 2]| {
            run.windows(2)
                .position(|w| (w[0], w[1]) == (p, q) || (w[0], w[1]) == (q, p))
        };
        let (i, j) = (at(first)?, at(last)?);
        Some(run[i.min(j)..=i.max(j) + 1].to_vec())
    }

    /// New edges beside the given run of points (one [`SubD::edge_run`] or
    /// [`SubD::edge_range`] made), as [`SubD::with_edges_inserted`] does.
    ///
    /// # Errors
    /// When a face beside the run is not a quad, or the new cage is not a valid mesh.
    pub fn with_edges_inserted_on(
        &self,
        run: &[u32],
        amount: InsertAmount,
        side: InsertSide,
    ) -> Result<Self, MeshError> {
        let mut subd = self.clone();
        if matches!(side, InsertSide::Along | InsertSide::Both) {
            subd = subd.inserted_beside(run, amount)?;
        }
        if matches!(side, InsertSide::Against | InsertSide::Both) {
            let back: Vec<u32> = run.iter().rev().copied().collect();
            subd = subd.inserted_beside(&back, amount)?;
        }
        Ok(subd)
    }

    /// New edges beside `run` in the faces whose corners run along it.
    fn inserted_beside(&self, run: &[u32], amount: InsertAmount) -> Result<Self, MeshError> {
        let cage = self.cage();
        let mut rings: Vec<Vec<u32>> = cage.faces().map(<[u32]>::to_vec).collect();
        let mut points = cage.vertices().to_vec();
        // Each run edge's face on this side, and its two crossing edges' far ends.
        let mut split: Vec<(usize, [u32; 4])> = Vec::new();
        for w in run.windows(2) {
            let (a, b) = (w[0], w[1]);
            let found = rings.iter().enumerate().find_map(|(f, ring)| {
                let n = ring.len();
                let k = (0..n).find(|&k| ring[k] == a && ring[(k + 1) % n] == b)?;
                Some((f, ring.clone(), k))
            });
            let Some((f, ring, k)) = found else { continue };
            if ring.len() != 4 {
                return Err(MeshError::FaceTooSmall { face: f });
            }
            // The quad a, b, then b's far point, then a's far point.
            split.push((f, [a, b, ring[(k + 2) % 4], ring[(k + 3) % 4]]));
        }
        // One new point per crossing edge, shared by the faces on either side of it.
        let mut new_point: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut place = |near: u32, far: u32, points: &mut Vec<Point>| -> u32 {
            *new_point.entry((near, far)).or_insert_with(|| {
                let (p, q) = (points[near as usize], points[far as usize]);
                let length = (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f64>().sqrt();
                let t = match amount {
                    InsertAmount::Absolute(d) if length > 0.0 => (d / length).clamp(0.0, 0.95),
                    InsertAmount::Absolute(_) => 0.5,
                    InsertAmount::Proportional(s) => s.clamp(0.0, 0.95),
                };
                points.push(std::array::from_fn(|i| p[i] + t * (q[i] - p[i])));
                index(points.len() - 1)
            })
        };
        let mut halves: Vec<Vec<u32>> = Vec::new();
        let mut gone: BTreeSet<usize> = BTreeSet::new();
        for &(f, [a, b, bb, aa]) in &split {
            let (ca, cb) = (place(a, aa, &mut points), place(b, bb, &mut points));
            gone.insert(f);
            halves.push(vec![a, b, cb, ca]);
            halves.push(vec![ca, cb, bb, aa]);
        }
        // The other faces on a split crossing edge take its new point.
        give_new_points(&mut rings, &gone, &new_point);
        let mut old_faces: Vec<Option<usize>> = Vec::new();
        let mut faces: Vec<Vec<u32>> = Vec::new();
        for (f, ring) in rings.into_iter().enumerate() {
            if !gone.contains(&f) {
                faces.push(ring);
                old_faces.push(Some(f));
            }
        }
        let old_count = cage.vertex_count();
        old_faces.extend(std::iter::repeat_n(None, halves.len()));
        faces.extend(halves);
        let number: Vec<Option<u32>> = (0..old_count).map(|v| Some(index(v))).collect();
        let mut out = self.carried_to(Mesh::new(points, &faces)?, &number, &old_faces);
        // A creased crossing edge stays creased in both its halves.
        let creased: Vec<[u32; 2]> = new_point
            .iter()
            .filter(|&(&(p, q), _)| self.sharpness(p, q).is_crease())
            .flat_map(|(&(p, q), &c)| [[p, c], [c, q]])
            .collect();
        if !creased.is_empty() {
            out = out.with_creases(&creased);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    fn at(subd: &SubD, p: Point) -> u32 {
        index(subd.cage().vertices().iter().position(|&q| q == p).unwrap())
    }

    /// Rhino 8's box: 4 m, 2 × 2 faces a side, at x 200 to 204.
    fn rhino_box() -> SubD {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])
    }

    #[test]
    fn a_corner_edge_runs_to_the_box_corners() {
        let subd = rhino_box();
        let (a, b) = (at(&subd, [204.0, 0.0, 0.0]), at(&subd, [204.0, 0.0, 2.0]));
        let run = subd.edge_run([a, b]);
        let points: Vec<Point> = run
            .iter()
            .map(|&v| subd.cage().vertices()[v as usize])
            .collect();
        assert_eq!(
            points,
            vec![[204.0, 0.0, 0.0], [204.0, 0.0, 2.0], [204.0, 0.0, 4.0]]
        );
    }

    #[test]
    fn inserting_beside_a_corner_edge_matches_rhino() {
        // Rhino 8: the corner edge (204, 0, 0) to (204, 0, 4) and 0.5 give a run at x = 203.5 on
        // the front: 3 new points, 26 faces (24 quads, 2 five-sided), 29 points.
        let subd = rhino_box();
        let (a, b) = (at(&subd, [204.0, 0.0, 0.0]), at(&subd, [204.0, 0.0, 2.0]));
        let mut front = None;
        for side in [InsertSide::Along, InsertSide::Against] {
            let out = subd
                .with_edges_inserted([a, b], InsertAmount::Absolute(0.5), side)
                .unwrap();
            assert_eq!(out.cage().vertex_count(), 29);
            assert_eq!(out.cage().face_count(), 26);
            let mut sides: Vec<usize> = out.cage().faces().map(<[u32]>::len).collect();
            sides.sort_unstable();
            assert_eq!(sides.iter().filter(|&&n| n == 5).count(), 2);
            let new: Vec<Point> = out.cage().vertices()[26..].to_vec();
            if new
                .iter()
                .all(|p| (p[0] - 203.5).abs() < 1e-12 && p[1] == 0.0)
            {
                front = Some(side);
            }
        }
        assert!(
            front.is_some(),
            "one side puts the run at x = 203.5 on the front"
        );
    }

    #[test]
    fn a_range_takes_only_the_edges_between_as_rhino_does() {
        // Rhino 8, Mode=Range: the two middle edges of the front, x = 202, give a run beside
        // just them at x = 202.5: 3 new points, 26 faces on 29 points. Full runs round the box.
        let subd = rhino_box();
        let p = |z: f64| at(&subd, [202.0, 0.0, z]);
        let (low, high) = ([p(0.0), p(2.0)], [p(2.0), p(4.0)]);
        assert_eq!(
            subd.edge_run(low).len(),
            9,
            "Full runs round the box and back"
        );
        let range = subd.edge_range(low, high).unwrap();
        assert_eq!(range.len(), 3);
        let mut placed = Vec::new();
        for side in [InsertSide::Along, InsertSide::Against] {
            let out = subd
                .with_edges_inserted_on(&range, InsertAmount::Absolute(0.5), side)
                .unwrap();
            assert_eq!(
                (out.cage().face_count(), out.cage().vertex_count()),
                (26, 29)
            );
            placed.push(out.cage().vertices()[26][0]);
        }
        placed.sort_by(f64::total_cmp);
        assert_eq!(placed, vec![201.5, 202.5], "one side each way");
    }

    #[test]
    fn both_sides_and_proportional() {
        let subd = rhino_box();
        let (a, b) = (at(&subd, [204.0, 0.0, 0.0]), at(&subd, [204.0, 0.0, 2.0]));
        let both = subd
            .with_edges_inserted([a, b], InsertAmount::Proportional(0.25), InsertSide::Both)
            .unwrap();
        assert_eq!(both.cage().vertex_count(), 32);
        assert_eq!(both.cage().face_count(), 28);
        assert!(both.cage().vertices().contains(&[203.5, 0.0, 2.0]));
        assert!(both.cage().vertices().contains(&[204.0, 0.5, 2.0]));
    }
}
