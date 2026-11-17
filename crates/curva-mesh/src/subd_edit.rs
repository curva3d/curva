//! What Rhino 8's SubD editing commands do to the cage (spec `docs/specs/mesh-and-subd.md` §1,
//! Stage C): RemoveCrease, DeleteSubDFaces, Fill, Stitch, InsertEdge across a ring, Bridge, and
//! UnweldEdge and WeldEdge.
//! (Crease is [`SubD::with_creases`] and [`SubD::with_corners`].) Each keeps the tags, sharpness
//! and colours it can ([`SubD::carried_to`]); a new open border is creased, as always.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Mesh, MeshError, Point, Sharpness, SubD, VertexTag};

/// How Fill closes a hole, as Rhino's options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillStyle {
    /// Quads wherever it can (Rhino's `Automatic`): a fan of quads round a new middle point, with
    /// one triangle when the hole has an odd number of sides.
    #[default]
    Automatic,
    /// One face of as many sides as the hole.
    SingleFace,
    /// Triangles round a new middle point.
    TriangleFan,
}

/// Where Stitch puts the points it joins, as Rhino's options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StitchAt {
    /// Halfway between them.
    #[default]
    Average,
    /// At the first point of each pair.
    First,
    /// At the second point of each pair.
    Second,
}

fn faces_of(mesh: &Mesh) -> Vec<Vec<u32>> {
    mesh.faces().map(<[u32]>::to_vec).collect()
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn midpoint(points: &[Point]) -> Point {
    let n = points.len().max(1) as f64;
    let sum = points
        .iter()
        .fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
    sum.map(|c| c / n)
}

impl SubD {
    /// The same SubD with these edges smooth again, as Rhino's RemoveCrease. An open border stays
    /// creased: a SubD's border always is.
    #[must_use]
    pub fn without_creases(self, edges: &[[u32; 2]]) -> Self {
        let uses = self.edge_uses();
        edges.iter().fold(self, |subd, &[a, b]| {
            let inner = uses
                .get(&(a.min(b), a.max(b)))
                .is_some_and(|f| f.len() == 2);
            if inner {
                subd.with_sharpness([a, b], Sharpness::SMOOTH)
            } else {
                subd
            }
        })
    }

    /// The faces numbered `faces` taken away, as Rhino's DeleteSubDFaces; points no face uses any
    /// more go too. The new border is creased.
    ///
    /// # Errors
    /// When the cage left is not a valid mesh.
    pub fn without_faces(&self, faces: &[usize]) -> Result<Self, MeshError> {
        let cage = self.cage();
        let kept: Vec<usize> = (0..cage.face_count())
            .filter(|f| !faces.contains(f))
            .collect();
        let mut used = vec![false; cage.vertex_count()];
        for &f in &kept {
            for &v in cage.face(f) {
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
        let rings: Vec<Vec<u32>> = kept
            .iter()
            .map(|&f| {
                cage.face(f)
                    .iter()
                    .filter_map(|&v| number[v as usize])
                    .collect()
            })
            .collect();
        let new_cage = Mesh::new(points, &rings)?;
        let old_faces: Vec<Option<usize>> = kept.into_iter().map(Some).collect();
        Ok(self.carried_to(new_cage, &number, &old_faces))
    }

    /// Every hole closed, as Rhino's Fill on each border loop, in the given style. The filled
    /// border's edges become smooth (they are no longer a border).
    ///
    /// # Errors
    /// When the filled cage is not a valid mesh.
    pub fn filled(&self, style: FillStyle) -> Result<Self, MeshError> {
        let cage = self.cage();
        let mut points = cage.vertices().to_vec();
        let mut rings = faces_of(cage);
        let mut old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        for hole in self.border_loops() {
            let n = hole.len();
            match style {
                FillStyle::SingleFace => rings.push(hole.clone()),
                FillStyle::TriangleFan | FillStyle::Automatic => {
                    let around: Vec<Point> = hole.iter().map(|&v| points[v as usize]).collect();
                    let centre = index(points.len());
                    points.push(midpoint(&around));
                    let step = if style == FillStyle::Automatic { 2 } else { 1 };
                    let mut k = 0;
                    while k < n {
                        let take = if k + step < n || n % step == 0 {
                            step
                        } else {
                            1
                        };
                        let mut face = vec![centre];
                        face.extend((0..=take).map(|s| hole[(k + s) % n]));
                        rings.push(face);
                        k += take;
                    }
                }
            }
            old_faces.resize(rings.len(), None);
        }
        let number: Vec<Option<u32>> = (0..cage.vertex_count()).map(|v| Some(index(v))).collect();
        Ok(self.carried_to(Mesh::new(points, &rings)?, &number, &old_faces))
    }

    /// The open border's loops, each running the way a face across the hole must, so it faces the
    /// same way as the rest.
    fn border_loops(&self) -> Vec<Vec<u32>> {
        let cage = self.cage();
        // Each border edge, from the point it ends at to the one it starts at (the hole's way).
        let mut next: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
        let uses = self.edge_uses();
        for face in cage.faces() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                if uses
                    .get(&(a.min(b), a.max(b)))
                    .is_some_and(|f| f.len() == 1)
                {
                    next.insert(b, a);
                }
            }
        }
        let mut loops = Vec::new();
        while let Some((&start, _)) = next.iter().next() {
            let mut ring = vec![start];
            let mut here = start;
            while let Some(to) = next.remove(&here) {
                if to == start {
                    break;
                }
                ring.push(to);
                here = to;
            }
            if ring.len() >= 3 {
                loops.push(ring);
            }
        }
        loops
    }

    /// Pairs of points joined into one, as Rhino's Stitch: each pair's second point goes into its
    /// first, at the place `at` says. Faces left with a point twice lose the repeat, and faces left
    /// with fewer than three corners go.
    ///
    /// # Errors
    /// When the stitched cage is not a valid mesh.
    pub fn stitched(&self, pairs: &[[u32; 2]], at: StitchAt) -> Result<Self, MeshError> {
        let cage = self.cage();
        let mut points = cage.vertices().to_vec();
        let mut into: Vec<u32> = (0..cage.vertex_count()).map(index).collect();
        for &[a, b] in pairs {
            let (pa, pb) = (points[a as usize], points[b as usize]);
            points[a as usize] = match at {
                StitchAt::Average => midpoint(&[pa, pb]),
                StitchAt::First => pa,
                StitchAt::Second => pb,
            };
            into[b as usize] = a;
        }
        // Follow chains of joins to their end.
        let root = |mut v: u32| {
            while into[v as usize] != v {
                v = into[v as usize];
            }
            v
        };
        let mut number = vec![None; cage.vertex_count()];
        let mut kept = Vec::new();
        for v in 0..cage.vertex_count() {
            if root(index(v)) == index(v) {
                number[v] = Some(index(kept.len()));
                kept.push(points[v]);
            }
        }
        let number: Vec<Option<u32>> = (0..cage.vertex_count())
            .map(|v| number[root(index(v)) as usize])
            .collect();
        let mut rings = Vec::new();
        let mut old_faces = Vec::new();
        for (f, face) in cage.faces().enumerate() {
            let mut ring: Vec<u32> = Vec::with_capacity(face.len());
            for &v in face {
                if let Some(n) = number[v as usize]
                    && !ring.contains(&n)
                {
                    ring.push(n);
                }
            }
            if ring.len() >= 3 {
                rings.push(ring);
                old_faces.push(Some(f));
            }
        }
        Ok(self.carried_to(Mesh::new(kept, &rings)?, &number, &old_faces))
    }

    /// A new edge loop across the ring of quads through edge `[a, b]`, as Rhino's InsertEdge with
    /// `Type=Ring`: each quad of the ring is cut in two, `offset` of the way (0 to 1, Rhino's
    /// `Proportional`) along its crossing edges from the side of `a`. The ring runs from quad to
    /// quad across opposite edges until it closes, or stops at a border or a face that is not a
    /// quad.
    ///
    /// # Errors
    /// When the edge is not in the cage, or the cut cage is not a valid mesh.
    pub fn with_ring_cut(&self, [a, b]: [u32; 2], offset: f64) -> Result<Self, MeshError> {
        let cage = self.cage();
        let t = offset.clamp(0.0, 1.0);
        let uses = self.edge_uses();
        let Some(start) = uses.get(&(a.min(b), a.max(b))) else {
            return Err(MeshError::FaceTooSmall { face: 0 });
        };
        // The crossing edges, each oriented so its "from" side follows `a`'s side, and the quads
        // between them.
        let mut crossings: Vec<[u32; 2]> = vec![[a, b]];
        let mut quads: Vec<usize> = Vec::new();
        // From each face beside the first edge: on a closed ring the second walk finds its first
        // quad taken already and stops; on an open one it adds the other side.
        for &first in start {
            let mut face = first;
            let mut edge = [a, b];
            loop {
                let ring = cage.face(face);
                if ring.len() != 4 || quads.contains(&face) {
                    break;
                }
                quads.push(face);
                // The opposite edge, oriented to keep `a`'s side.
                let k = (0..4).find(|&k| {
                    let (p, q) = (ring[k], ring[(k + 1) % 4]);
                    (p == edge[0] && q == edge[1]) || (p == edge[1] && q == edge[0])
                });
                let Some(k) = k else { break };
                let forward = ring[k] == edge[0];
                let (c, d) = (ring[(k + 2) % 4], ring[(k + 3) % 4]);
                let opposite = if forward { [d, c] } else { [c, d] };
                if opposite == crossings[0] || opposite == [crossings[0][1], crossings[0][0]] {
                    break;
                }
                crossings.push(opposite);
                let others =
                    uses.get(&(opposite[0].min(opposite[1]), opposite[0].max(opposite[1])));
                match others.and_then(|fs| fs.iter().copied().find(|&g| g != face)) {
                    Some(g) => {
                        face = g;
                        edge = opposite;
                    }
                    None => break,
                }
            }
        }
        let mut points = cage.vertices().to_vec();
        let mut cut_point = std::collections::HashMap::new();
        for &[p, q] in &crossings {
            let (pp, pq) = (points[p as usize], points[q as usize]);
            let new = index(points.len());
            points.push(std::array::from_fn(|i| pp[i] + t * (pq[i] - pp[i])));
            cut_point.insert((p, q), new);
            cut_point.insert((q, p), new);
        }
        let mut rings = Vec::new();
        let mut old_faces = Vec::new();
        for (f, face) in cage.faces().enumerate() {
            if quads.contains(&f) {
                // Split the quad along the two cut points on its crossing edges.
                let cuts: Vec<usize> = (0..4)
                    .filter(|&k| cut_point.contains_key(&(face[k], face[(k + 1) % 4])))
                    .collect();
                if let [k0, k1] = cuts[..] {
                    let c0 = cut_point[&(face[k0], face[(k0 + 1) % 4])];
                    let c1 = cut_point[&(face[k1], face[(k1 + 1) % 4])];
                    let one = vec![face[k0], c0, c1, face[(k1 + 1) % 4]];
                    let two = vec![c0, face[(k0 + 1) % 4], face[k1], c1];
                    rings.push(one);
                    rings.push(two);
                    old_faces.extend([Some(f), Some(f)]);
                    continue;
                }
            }
            // Faces beside the ring that share a cut edge get the cut point too.
            let mut ring = Vec::with_capacity(face.len() + 2);
            for k in 0..face.len() {
                let (p, q) = (face[k], face[(k + 1) % face.len()]);
                ring.push(p);
                if let Some(&c) = cut_point.get(&(p, q)) {
                    ring.push(c);
                }
            }
            rings.push(ring);
            old_faces.push(Some(f));
        }
        let number: Vec<Option<u32>> = (0..cage.vertex_count()).map(|v| Some(index(v))).collect();
        Ok(self.carried_to(Mesh::new(points, &rings)?, &number, &old_faces))
    }

    /// Two border chains joined by quads, as Rhino's Bridge: `first` and `second` are runs of
    /// points along open borders, of the same length, joined point to point in `segments` rows of
    /// quads (at least 1).
    ///
    /// # Errors
    /// When the chains differ in length, or the bridged cage is not a valid mesh.
    pub fn bridged(&self, first: &[u32], second: &[u32], segments: u32) -> Result<Self, MeshError> {
        if first.len() != second.len() || first.len() < 2 {
            return Err(MeshError::FaceTooSmall { face: 0 });
        }
        let cage = self.cage();
        let mut points = cage.vertices().to_vec();
        let mut rings = faces_of(cage);
        let mut old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        let rows = segments.max(1);
        // Rows of points from the first chain to the second, the inner ones new.
        let mut lines: Vec<Vec<u32>> = vec![first.to_vec()];
        for r in 1..rows {
            let t = f64::from(r) / f64::from(rows);
            let line = first
                .iter()
                .zip(second)
                .map(|(&p, &q)| {
                    let (pp, pq) = (points[p as usize], points[q as usize]);
                    points.push(std::array::from_fn(|i| pp[i] + t * (pq[i] - pp[i])));
                    index(points.len() - 1)
                })
                .collect();
            lines.push(line);
        }
        lines.push(second.to_vec());
        // Whether the face beside the first chain runs along it from its first point.
        let flip = cage
            .faces()
            .any(|f| (0..f.len()).any(|k| f[k] == first[0] && f[(k + 1) % f.len()] == first[1]));
        for pair in lines.windows(2) {
            for k in 0..first.len() - 1 {
                let quad = [pair[0][k], pair[0][k + 1], pair[1][k + 1], pair[1][k]];
                // A new face runs along a shared edge the other way from its neighbour.
                rings.push(if flip {
                    quad.iter().rev().copied().collect()
                } else {
                    quad.to_vec()
                });
            }
        }
        old_faces.resize(rings.len(), None);
        let number: Vec<Option<u32>> = (0..cage.vertex_count()).map(|v| Some(index(v))).collect();
        Ok(self.carried_to(Mesh::new(points, &rings)?, &number, &old_faces))
    }

    /// The SubD cut open along these inner edges, as Rhino's UnweldEdge: each becomes two creased
    /// border edges at the same place, one for each face. A point on the cut is copied once for
    /// each piece of its ring of faces the cut makes, so a cut ending inside the surface keeps its
    /// end points whole. Copies of a corner are corners too.
    ///
    /// # Errors
    /// When an edge is not an inner edge of the cage (`UnusableEdge`), or the cut cage is not a
    /// valid mesh.
    pub fn unwelded(&self, edges: &[[u32; 2]]) -> Result<Self, MeshError> {
        let cage = self.cage();
        let uses = self.edge_uses();
        let mut cut = BTreeSet::new();
        for &[a, b] in edges {
            let key = (a.min(b), a.max(b));
            if !uses.get(&key).is_some_and(|f| f.len() == 2) {
                return Err(MeshError::UnusableEdge { edge: [a, b] });
            }
            cut.insert(key);
        }
        let mut faces_at: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (f, face) in cage.faces().enumerate() {
            for &v in face {
                faces_at.entry(v).or_default().push(f);
            }
        }
        // The points either side of `v` in face `f`.
        let beside = |f: usize, v: u32| {
            let face = cage.face(f);
            let k = face.iter().position(|&w| w == v).unwrap_or(0);
            let n = face.len();
            [face[(k + n - 1) % n], face[(k + 1) % n]]
        };
        let mut points = cage.vertices().to_vec();
        let mut rings = faces_of(cage);
        let mut copies: Vec<(u32, u32)> = Vec::new();
        let touched: BTreeSet<u32> = cut.iter().flat_map(|&(a, b)| [a, b]).collect();
        for v in touched {
            let around = &faces_at[&v];
            // Faces round `v` joined across edges that are not cut, as pieces.
            let mut piece: Vec<usize> = (0..around.len()).collect();
            let find = |piece: &[usize], mut i: usize| {
                while piece[i] != i {
                    i = piece[i];
                }
                i
            };
            for i in 0..around.len() {
                for j in i + 1..around.len() {
                    let shared = beside(around[i], v)
                        .into_iter()
                        .find(|w| beside(around[j], v).contains(w));
                    if let Some(w) = shared
                        && !cut.contains(&(v.min(w), v.max(w)))
                    {
                        let (ri, rj) = (find(&piece, i), find(&piece, j));
                        piece[ri.max(rj)] = ri.min(rj);
                    }
                }
            }
            // The piece with the lowest face keeps `v`; each other gets a copy.
            let mut copy_of_piece: BTreeMap<usize, u32> = BTreeMap::new();
            for (i, &f) in around.iter().enumerate() {
                let root = find(&piece, i);
                if root == 0 {
                    continue;
                }
                let new = *copy_of_piece.entry(root).or_insert_with(|| {
                    points.push(points[v as usize]);
                    copies.push((v, index(points.len() - 1)));
                    index(points.len() - 1)
                });
                for corner in &mut rings[f] {
                    if *corner == v {
                        *corner = new;
                    }
                }
            }
        }
        let number: Vec<Option<u32>> = (0..cage.vertex_count()).map(|v| Some(index(v))).collect();
        let old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        let mut subd = self.carried_to(Mesh::new(points, &rings)?, &number, &old_faces);
        // Sharp inner edges beside a copied point: found through a face that holds them.
        for (&(a, b), fs) in &uses {
            let sharpness = self.sharpness(a, b);
            if fs.len() != 2 || cut.contains(&(a, b)) || sharpness == Sharpness::SMOOTH {
                continue;
            }
            let old = cage.face(fs[0]);
            let at = |v: u32| old.iter().position(|&w| w == v).map(|k| rings[fs[0]][k]);
            if let (Some(na), Some(nb)) = (at(a), at(b)) {
                subd = subd.with_sharpness([na, nb], sharpness);
            }
        }
        let corners: Vec<u32> = copies
            .iter()
            .filter(|&&(v, _)| self.vertex_tag(v) == VertexTag::Corner)
            .map(|&(_, new)| new)
            .collect();
        Ok(subd.with_corners(&corners))
    }

    /// Border edges joined to the border edges at exactly the same place, as Rhino's WeldEdge
    /// (the way back from [`SubD::unwelded`]): the joined edges are smooth. Rhino welds nothing
    /// that is not at exactly the same place, however close.
    ///
    /// # Errors
    /// When an edge is not a border edge or has no other border edge at its place
    /// (`UnusableEdge`), or the welded cage is not a valid mesh.
    pub fn welded(&self, edges: &[[u32; 2]]) -> Result<Self, MeshError> {
        let p = self.cage().vertices();
        let uses = self.edge_uses();
        let border: Vec<(u32, u32)> = uses
            .iter()
            .filter(|(_, f)| f.len() == 1)
            .map(|(&k, _)| k)
            .collect();
        let same = |x: u32, y: u32| p[x as usize] == p[y as usize];
        let mut pairs = Vec::new();
        for &[a, b] in edges {
            let key = (a.min(b), a.max(b));
            let partner = border.iter().find(|&&(c, d)| {
                (c, d) != key && ((same(c, a) && same(d, b)) || (same(c, b) && same(d, a)))
            });
            let (Some(&(c, d)), true) = (partner, border.contains(&key)) else {
                return Err(MeshError::UnusableEdge { edge: [a, b] });
            };
            let (c, d) = if same(c, a) { (c, d) } else { (d, c) };
            for (x, y) in [(a, c), (b, d)] {
                if x != y {
                    pairs.push([x.min(y), x.max(y)]);
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        self.stitched(&pairs, StitchAt::First)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    fn open_box() -> SubD {
        SubD::new(cube()).without_faces(&[1]).unwrap()
    }

    #[test]
    fn remove_crease_smooths_inner_edges_but_not_the_border() {
        let creased = SubD::new(cube()).with_creases(&[[0, 1], [1, 2]]);
        let smooth = creased.without_creases(&[[1, 0]]);
        assert!(!smooth.sharpness(0, 1).is_crease());
        assert!(smooth.sharpness(1, 2).is_crease());
        let open = open_box();
        let border: Vec<[u32; 2]> = open.creases().collect();
        assert_eq!(open.clone().without_creases(&border).creases().count(), 4);
        let cornered = SubD::new(cube()).with_corners(&[3]).without_corners(&[3]);
        assert_eq!(cornered.vertex_tag(3), VertexTag::Smooth);
    }

    #[test]
    fn deleting_a_face_opens_a_creased_border_and_keeps_other_creases() {
        let subd = SubD::new(cube()).with_creases(&[[0, 3]]);
        let open = subd.without_faces(&[1]).unwrap();
        assert_eq!(open.cage().face_count(), 5);
        assert_eq!(open.cage().naked_edges().len(), 4);
        assert_eq!(
            open.creases().count(),
            5,
            "the four border edges and the kept crease"
        );
        let two = subd.without_faces(&[0, 1]).unwrap();
        assert_eq!(two.cage().vertex_count(), 8);
        let lone = SubD::new(cube()).without_faces(&[0, 1, 2, 3, 4]).unwrap();
        assert_eq!(
            (lone.cage().vertex_count(), lone.cage().face_count()),
            (4, 1)
        );
    }

    #[test]
    fn fill_closes_the_hole_in_each_style() {
        let open = open_box();
        let single = open.filled(FillStyle::SingleFace).unwrap();
        assert!(single.cage().is_closed());
        assert_eq!(single.cage().face_count(), 6);
        assert_eq!(
            single.creases().count(),
            0,
            "the old border is smooth once filled"
        );
        assert!((single.cage().volume().unwrap() - 1.0).abs() < 1e-12);
        let fan = open.filled(FillStyle::TriangleFan).unwrap();
        assert_eq!(fan.cage().face_count(), 5 + 4);
        let quads = open.filled(FillStyle::Automatic).unwrap();
        assert_eq!(
            quads.cage().face_count(),
            5 + 2,
            "two quads for a square hole"
        );
        assert!(quads.cage().faces().skip(5).all(|f| f.len() == 4));
        assert!(quads.cage().is_closed());
    }

    #[test]
    fn stitch_joins_points_where_asked() {
        // Two separate squares side by side, a gap between them.
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.2, 0.0, 0.0],
            [2.2, 0.0, 0.0],
            [2.2, 1.0, 0.0],
            [1.2, 1.0, 0.0],
        ];
        let two = SubD::new(Mesh::new(points, &[[0, 1, 2, 3], [4, 5, 6, 7]]).unwrap());
        let joined = two.stitched(&[[1, 4], [2, 7]], StitchAt::Average).unwrap();
        assert_eq!(joined.cage().vertex_count(), 6);
        assert_eq!(joined.cage().vertices()[1], [1.1, 0.0, 0.0]);
        assert_eq!(joined.creases().count(), 6, "the shared edge is inside now");
        let first = two.stitched(&[[1, 4], [2, 7]], StitchAt::First).unwrap();
        assert_eq!(first.cage().vertices()[1], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_ring_cut_splits_the_quads_round_a_cube() {
        let subd = SubD::new(cube());
        // Edge 0–1 runs along x; the ring round x passes the bottom, back, top and front faces.
        let cut = subd.with_ring_cut([0, 1], 0.25).unwrap();
        assert_eq!(cut.cage().face_count(), 10);
        assert_eq!(cut.cage().vertex_count(), 12);
        assert!(cut.cage().is_closed());
        assert!((cut.cage().volume().unwrap() - 1.0).abs() < 1e-12);
        assert!(
            cut.cage().vertices()[8..]
                .iter()
                .all(|p| (p[0] - 0.25).abs() < 1e-12)
        );
    }

    #[test]
    fn a_ring_cut_through_the_middle_of_an_open_strip_cuts_both_ways() {
        // Three squares in a row, cut across the middle one's shared edges.
        let points: Vec<Point> = (0..4)
            .flat_map(|i| [[0.0, f64::from(i), 0.0], [1.0, f64::from(i), 0.0]])
            .collect();
        let faces = [[0, 1, 3, 2], [2, 3, 5, 4], [4, 5, 7, 6]];
        let strip = SubD::new(Mesh::new(points, &faces).unwrap());
        let cut = strip.with_ring_cut([2, 3], 0.5).unwrap();
        assert_eq!(
            cut.cage().face_count(),
            6,
            "every square of the strip is cut"
        );
        assert_eq!(cut.cage().vertex_count(), 12);
    }

    #[test]
    fn bridge_joins_two_borders_with_rows_of_quads() {
        // Two open squares facing each other, a unit apart.
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let two = SubD::new(Mesh::new(points, &[[0, 3, 2, 1], [4, 5, 6, 7]]).unwrap());
        let bridged = two.bridged(&[0, 1], &[4, 5], 2).unwrap();
        assert_eq!(bridged.cage().face_count(), 4);
        assert_eq!(bridged.cage().vertex_count(), 10);
        // The new faces join the borders, so those edges are no longer creased.
        assert!(!bridged.sharpness(0, 1).is_crease());
        assert!(bridged.cage().check(1e-9).misdirected_faces == Some(0));
    }

    /// Rhino's test: a 2 × 2 × 2 box from a mesh, and the two edges at x = 1 on its y = 0 side.
    fn box_and_cut() -> (SubD, Vec<[u32; 2]>) {
        let mesh = Mesh::mesh_box(&crate::Frame::world(), [2.0; 3], [2; 3]);
        let subd = SubD::new(mesh);
        let p = subd.cage().vertices();
        let on_cut = |v: u32| p[v as usize][0] == 1.0 && p[v as usize][1] == 0.0;
        let edges = subd
            .edge_uses()
            .keys()
            .filter(|&&(a, b)| on_cut(a) && on_cut(b))
            .map(|&(a, b)| [a, b])
            .collect();
        (subd, edges)
    }

    #[test]
    fn unweld_cuts_a_box_open_as_rhino_does() {
        let (subd, edges) = box_and_cut();
        assert_eq!(edges.len(), 2);
        assert_eq!(subd.cage().vertex_count(), 26);
        let cut = subd.unwelded(&edges).unwrap();
        // Rhino 8's UnweldEdge on the same box: 27 points, 50 edges, 4 creased, not closed. Only
        // the middle point is copied; the cut's ends stay whole inside the surface.
        assert_eq!(cut.cage().vertex_count(), 27);
        assert_eq!(cut.edge_uses().len(), 50);
        assert_eq!(cut.creases().count(), 4);
        let tags = (0..27).filter(|&v| cut.vertex_tag(v) == VertexTag::Crease);
        assert_eq!(tags.count(), 4);
        assert_eq!(cut.cage().vertices()[26], [1.0, 0.0, 1.0]);
    }

    #[test]
    fn weld_joins_the_cut_back_as_rhino_does() {
        let (subd, edges) = box_and_cut();
        let cut = subd.unwelded(&edges).unwrap();
        let border: Vec<[u32; 2]> = cut.creases().collect();
        let welded = cut.welded(&border).unwrap();
        // The same cage, smooth again.
        assert_eq!(welded.cage().vertices(), subd.cage().vertices());
        assert!(welded.cage().faces().eq(subd.cage().faces()));
        assert_eq!(welded.creases().count(), 0);
        // One side of the cut is enough: the edges that do not use the copy, point 26.
        let one_side: Vec<[u32; 2]> = border
            .iter()
            .copied()
            .filter(|e| !e.contains(&26))
            .collect();
        assert_eq!(one_side.len(), 2);
        assert_eq!(cut.welded(&one_side).unwrap().creases().count(), 0);
    }

    #[test]
    fn weld_and_unweld_refuse_edges_they_cannot_use() {
        let (subd, edges) = box_and_cut();
        assert_eq!(
            subd.welded(&edges),
            Err(MeshError::UnusableEdge { edge: edges[0] }),
            "an inner edge is not a border"
        );
        let open = open_box();
        let border: Vec<[u32; 2]> = open.creases().collect();
        assert!(open.welded(&border).is_err(), "no other edge at its place");
        assert!(open.unwelded(&border).is_err(), "a border is open already");
        // Rhino welds nothing that is merely close.
        let cut = subd.unwelded(&edges).unwrap();
        let mut points = cut.cage().vertices().to_vec();
        points[26][0] += 0.0006;
        let moved = cut.clone().with_cage_points(points);
        let border: Vec<[u32; 2]> = moved.creases().collect();
        assert!(moved.welded(&border).is_err());
    }

    #[test]
    fn a_cut_right_across_a_grid_splits_it_and_keeps_its_creases() {
        // A 2 × 2 grid of squares, cut along its middle line from border to border.
        let points: Vec<Point> = (0..9)
            .map(|i| [f64::from(i % 3), f64::from(i / 3), 0.0])
            .collect();
        let faces = [[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]];
        let grid = SubD::new(Mesh::new(points, &faces).unwrap()).with_creases(&[[3, 4]]);
        let cut = grid.unwelded(&[[1, 4], [4, 7]]).unwrap();
        assert_eq!(cut.cage().vertex_count(), 12, "the whole line is copied");
        assert_eq!(
            cut.creases().count(),
            8 + 4 + 1,
            "border, both sides of the cut, 3-4"
        );
        // The crease from 3 to 4 stays on the left piece, which keeps point 4.
        assert!(cut.sharpness(3, 4).is_crease());
        let p = cut.cage().vertices();
        let seam: Vec<[u32; 2]> = cut
            .creases()
            .filter(|e| e.iter().all(|&v| p[v as usize][0] == 1.0))
            .collect();
        assert_eq!(seam.len(), 4);
        let welded = cut.welded(&seam).unwrap();
        assert_eq!(welded.cage().vertex_count(), 9);
        assert_eq!(welded.creases().count(), 8 + 1);
    }
}
