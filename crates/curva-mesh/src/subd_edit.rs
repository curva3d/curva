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
    /// Triangles across the hole between its own points, with no new point.
    Triangulated,
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
        self.filled_holes(&self.border_loops(), style)
    }

    /// The holes these border edges run round closed, as Rhino's Fill on the picked edges; other
    /// holes stay open.
    ///
    /// # Errors
    /// When the filled cage is not a valid mesh.
    pub fn filled_at(&self, edges: &[[u32; 2]], style: FillStyle) -> Result<Self, MeshError> {
        let picked: BTreeSet<(u32, u32)> =
            edges.iter().map(|&[a, b]| (a.min(b), a.max(b))).collect();
        let holes: Vec<Vec<u32>> = self
            .border_loops()
            .into_iter()
            .filter(|hole| {
                (0..hole.len()).any(|k| {
                    let (a, b) = (hole[k], hole[(k + 1) % hole.len()]);
                    picked.contains(&(a.min(b), a.max(b)))
                })
            })
            .collect();
        self.filled_holes(&holes, style)
    }

    fn filled_holes(&self, holes: &[Vec<u32>], style: FillStyle) -> Result<Self, MeshError> {
        let cage = self.cage();
        let mut points = cage.vertices().to_vec();
        let mut rings = faces_of(cage);
        let mut old_faces: Vec<Option<usize>> = (0..rings.len()).map(Some).collect();
        for hole in holes {
            let around: Vec<Point> = hole.iter().map(|&v| points[v as usize]).collect();
            let (faces, middle) = crate::fill::patch(&around, style)?;
            let centre = index(points.len());
            if let Some(p) = middle {
                points.push(p);
            }
            rings.extend(faces.into_iter().map(|face| {
                face.into_iter()
                    .map(|k| hole.get(k as usize).copied().unwrap_or(centre))
                    .collect::<Vec<u32>>()
            }));
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
    /// quads (at least 1). The rows bow out along the borders as Rhino's do, less as
    /// `straightness` goes from 0 to 1, where they lie on straight lines (see `subd_bridge`).
    ///
    /// # Errors
    /// When the chains differ in length, or the bridged cage is not a valid mesh.
    pub fn bridged(
        &self,
        first: &[u32],
        second: &[u32],
        segments: u32,
        straightness: f64,
    ) -> Result<Self, MeshError> {
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
        for row in crate::subd_bridge::bridge_rows(cage, first, second, rows, straightness) {
            let line = row
                .into_iter()
                .map(|p| {
                    points.push(p);
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
#[path = "subd_edit_tests.rs"]
mod tests;
