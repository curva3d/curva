//! Rhino 8's InsertPoint for SubDs: points put on cage edges, one after another, and each two in
//! a row that share a face joined by a new edge across it. Measured through the screen
//! (22–23 November 2026): on a 4 m SubD box of 2 × 2 a side, one point at the middle of a bottom
//! edge splits it, the two faces beside it five-sided (24 faces on 27 points); a second point on
//! the opposite edge of the bottom face beside it splits that face into two quads.

use std::collections::BTreeSet;

use crate::{Mesh, MeshError, Point, SubD};

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Splits edge `a`–`b` at new point `v` in every face that has it.
fn split_edge(rings: &mut [Vec<u32>], a: u32, b: u32, v: u32) {
    for ring in rings.iter_mut() {
        let n = ring.len();
        if let Some(k) = (0..n).find(|&k| {
            let (p, q) = (ring[k], ring[(k + 1) % n]);
            (p, q) == (a, b) || (p, q) == (b, a)
        }) {
            ring.insert(k + 1, v);
        }
    }
}

/// Splits the face holding both `u` and `v`, not side by side, into two along a new edge.
fn split_face(rings: &mut Vec<Vec<u32>>, from: &mut Vec<Option<usize>>, u: u32, v: u32) {
    let found = rings.iter().enumerate().find_map(|(f, ring)| {
        let n = ring.len();
        let i = ring.iter().position(|&p| p == u)?;
        let j = ring.iter().position(|&p| p == v)?;
        let apart = (i + 1) % n != j && (j + 1) % n != i;
        apart.then_some((f, i, j))
    });
    let Some((f, i, j)) = found else { return };
    let ring = rings[f].clone();
    let n = ring.len();
    let walk = |s: usize, e: usize| -> Vec<u32> {
        let mut out = vec![ring[s]];
        let mut k = s;
        while k != e {
            k = (k + 1) % n;
            out.push(ring[k]);
        }
        out
    };
    rings[f] = walk(i, j);
    rings.push(walk(j, i));
    from.push(from[f]);
}

impl SubD {
    /// Points put on cage edges, as Rhino's InsertPoint: each `(edge, t)` places a point `t` of
    /// the way along the edge from its first point, splitting the edge in every face beside it,
    /// and joins it to the point before by a new edge across the face they share, when they
    /// share one and are not already side by side. A creased edge stays creased in both halves.
    ///
    /// # Errors
    /// When the new cage is not a valid mesh.
    pub fn with_points_inserted(&self, points: &[([u32; 2], f64)]) -> Result<Self, MeshError> {
        let cage = self.cage();
        let mut vertices: Vec<Point> = cage.vertices().to_vec();
        let mut rings: Vec<Vec<u32>> = cage.faces().map(<[u32]>::to_vec).collect();
        let mut from: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        let mut creased: BTreeSet<[u32; 2]> = BTreeSet::new();
        let mut previous: Option<u32> = None;
        for &([a, b], t) in points {
            let t = t.clamp(0.0, 1.0);
            let (pa, pb) = (vertices[a as usize], vertices[b as usize]);
            let v = index(vertices.len());
            vertices.push(std::array::from_fn(|i| pa[i] + t * (pb[i] - pa[i])));
            split_edge(&mut rings, a, b, v);
            if self.sharpness(a, b).is_crease() || creased.contains(&[a.min(b), a.max(b)]) {
                creased.insert([a.min(v), a.max(v)]);
                creased.insert([v.min(b), v.max(b)]);
            }
            if let Some(u) = previous {
                split_face(&mut rings, &mut from, u, v);
            }
            previous = Some(v);
        }
        let number: Vec<Option<u32>> = (0..cage.vertex_count()).map(|v| Some(index(v))).collect();
        let out = self.carried_to(Mesh::new(vertices, &rings)?, &number, &from);
        let halves: Vec<[u32; 2]> = creased.into_iter().collect();
        Ok(if halves.is_empty() {
            out
        } else {
            out.with_creases(&halves)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Frame, SubD};

    fn rhino_box() -> SubD {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])
    }

    fn at(subd: &SubD, p: [f64; 3]) -> u32 {
        let k = subd.cage().vertices().iter().position(|&q| q == p).unwrap();
        u32::try_from(k).unwrap()
    }

    fn sides(subd: &SubD, n: usize) -> usize {
        subd.cage().faces().filter(|f| f.len() == n).count()
    }

    #[test]
    fn one_point_splits_its_edge_as_in_rhino() {
        let subd = rhino_box();
        let edge = [at(&subd, [200.0, 0.0, 0.0]), at(&subd, [202.0, 0.0, 0.0])];
        let out = subd.with_points_inserted(&[(edge, 0.5)]).unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (24, 27)
        );
        assert_eq!(sides(&out, 5), 2);
        assert_eq!(out.cage().vertices()[26], [201.0, 0.0, 0.0]);
        assert!(out.cage().is_closed());
    }

    #[test]
    fn two_points_across_a_face_split_it_as_in_rhino() {
        // Rhino 8: after a point at (201, 0, 0), points at (203, 0, 0) and (203, 2, 0) give 25
        // faces (21 quads, 4 five-sided) on 29 points, the bottom quad split at x = 203.
        let subd = rhino_box();
        let p = |x: f64, y: f64| at(&subd, [x, y, 0.0]);
        let first = subd
            .with_points_inserted(&[([p(200.0, 0.0), p(202.0, 0.0)], 0.5)])
            .unwrap();
        let q = |x: f64, y: f64| at(&first, [x, y, 0.0]);
        let out = first
            .with_points_inserted(&[
                ([q(202.0, 0.0), q(204.0, 0.0)], 0.5),
                ([q(202.0, 2.0), q(204.0, 2.0)], 0.5),
            ])
            .unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (25, 29)
        );
        assert_eq!(sides(&out, 4), 21);
        assert_eq!(sides(&out, 5), 4);
        assert!(out.cage().is_closed());
    }
}
