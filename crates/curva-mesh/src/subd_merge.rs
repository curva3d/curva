//! Rhino 8's MergeFaces for SubDs: picked faces that touch along edges become one face round
//! their outer border. Measured (22 November 2026): two neighbouring quads on the side of a 4 m
//! SubD box of 2 × 2 a side become one six-sided face, the edge between them gone and every point
//! kept (23 faces on 26 points); Rhino says "Changed 1 objects.".

use std::collections::{BTreeMap, BTreeSet};

use crate::{Mesh, MeshError, SubD};

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// The picked faces in groups that touch along edges.
fn groups(faces: &[Vec<u32>], picked: &BTreeSet<usize>) -> Vec<Vec<usize>> {
    let mut by_edge: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for &f in picked {
        let ring = &faces[f];
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            by_edge.entry((a.min(b), a.max(b))).or_default().push(f);
        }
    }
    let mut left: BTreeSet<usize> = picked.clone();
    let mut out = Vec::new();
    while let Some(&start) = left.iter().next() {
        left.remove(&start);
        let (mut group, mut todo) = (vec![start], vec![start]);
        while let Some(f) = todo.pop() {
            let ring = &faces[f];
            for k in 0..ring.len() {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                for &g in &by_edge[&(a.min(b), a.max(b))] {
                    if left.remove(&g) {
                        group.push(g);
                        todo.push(g);
                    }
                }
            }
        }
        group.sort_unstable();
        out.push(group);
    }
    out
}

/// A group's outer border as one loop, running the way its faces do; `None` when the border is
/// not one loop (a ring of faces round a hole, say).
fn border(faces: &[Vec<u32>], group: &[usize]) -> Option<Vec<u32>> {
    let mut uses: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for &f in group {
        let ring = &faces[f];
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    for &f in group {
        let ring = &faces[f];
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            if uses[&(a.min(b), a.max(b))] == 1 && next.insert(a, b).is_some() {
                return None;
            }
        }
    }
    let &start = next.keys().next()?;
    let mut ring = vec![start];
    let mut here = next[&start];
    while here != start {
        ring.push(here);
        here = *next.get(&here)?;
        if ring.len() > next.len() {
            return None;
        }
    }
    (ring.len() == next.len()).then_some(ring)
}

impl SubD {
    /// The faces numbered `faces` merged, as Rhino's MergeFaces: each group of them that touch
    /// along edges becomes one face round the group's outer border, keeping the first face's
    /// colour; a face alone, or a group whose border is not one loop, stays as it is. Points no
    /// face uses any more go.
    ///
    /// # Errors
    /// When the merged cage is not a valid mesh.
    pub fn with_faces_merged(&self, faces: &[usize]) -> Result<Self, MeshError> {
        let cage = self.cage();
        let rings: Vec<Vec<u32>> = cage.faces().map(<[u32]>::to_vec).collect();
        let picked: BTreeSet<usize> = faces.iter().copied().filter(|&f| f < rings.len()).collect();
        let mut merged: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        let mut gone: BTreeSet<usize> = BTreeSet::new();
        for group in groups(&rings, &picked) {
            if group.len() < 2 {
                continue;
            }
            if let Some(ring) = border(&rings, &group) {
                merged.insert(group[0], ring);
                gone.extend(group[1..].iter().copied());
            }
        }
        let mut new_rings = Vec::new();
        let mut old_faces = Vec::new();
        for (f, ring) in rings.iter().enumerate() {
            if gone.contains(&f) {
                continue;
            }
            new_rings.push(merged.get(&f).unwrap_or(ring).clone());
            old_faces.push(Some(f));
        }
        let mut used = vec![false; cage.vertex_count()];
        for ring in &new_rings {
            for &v in ring {
                used[v as usize] = true;
            }
        }
        let mut number = vec![None; cage.vertex_count()];
        let mut points = Vec::new();
        for (v, p) in cage.vertices().iter().enumerate() {
            if used[v] {
                number[v] = Some(index(points.len()));
                points.push(*p);
            }
        }
        let new_rings: Vec<Vec<u32>> = new_rings
            .iter()
            .map(|ring| ring.iter().filter_map(|&v| number[v as usize]).collect())
            .collect();
        Ok(self.carried_to(Mesh::new(points, &new_rings)?, &number, &old_faces))
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

    /// The face whose middle is at `p`.
    fn face_at(subd: &SubD, p: [f64; 3]) -> usize {
        let at = subd.cage().vertices();
        subd.cage()
            .faces()
            .position(|f| {
                let n = f.len() as f64;
                (0..3).all(|i| {
                    let mid = f.iter().map(|&v| at[v as usize][i]).sum::<f64>() / n;
                    (mid - p[i]).abs() < 1e-9
                })
            })
            .unwrap()
    }

    #[test]
    fn two_neighbouring_quads_become_one_six_sided_face_as_in_rhino() {
        let subd = rhino_box();
        let picked = [
            face_at(&subd, [201.0, 0.0, 1.0]),
            face_at(&subd, [203.0, 0.0, 1.0]),
        ];
        let merged = subd.with_faces_merged(&picked).unwrap();
        assert_eq!(merged.cage().face_count(), 23);
        assert_eq!(merged.cage().vertex_count(), 26);
        let mut sides: Vec<usize> = merged.cage().faces().map(<[u32]>::len).collect();
        sides.sort_unstable();
        assert_eq!(sides.iter().filter(|&&n| n == 6).count(), 1);
        assert_eq!(sides.iter().filter(|&&n| n == 4).count(), 22);
        assert!(merged.cage().is_closed());
    }

    #[test]
    fn four_round_a_point_merge_and_the_point_goes() {
        let subd = rhino_box();
        let picked: Vec<usize> = [
            [201.0, 0.0, 1.0],
            [203.0, 0.0, 1.0],
            [201.0, 0.0, 3.0],
            [203.0, 0.0, 3.0],
        ]
        .map(|p| face_at(&subd, p))
        .to_vec();
        let merged = subd.with_faces_merged(&picked).unwrap();
        assert_eq!(merged.cage().face_count(), 21);
        assert_eq!(
            merged.cage().vertex_count(),
            25,
            "the side's middle point goes"
        );
    }

    #[test]
    fn faces_apart_stay_as_they_are() {
        let subd = rhino_box();
        let picked = [
            face_at(&subd, [201.0, 0.0, 1.0]),
            face_at(&subd, [203.0, 0.0, 3.0]),
        ];
        let same = subd.with_faces_merged(&picked).unwrap();
        assert_eq!(same.cage().face_count(), 24);
    }
}
