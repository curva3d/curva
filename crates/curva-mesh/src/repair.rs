//! MeshRepair's main fixes: unify normals, fill holes, remove degenerate and duplicate faces and
//! unused points, weld, and rebuild normals (spec `docs/specs/mesh-and-subd.md` §1).
//!
//! manifold-rust only takes closed meshes of triangles, and these fixes are mostly for meshes that
//! are not, so they are Curva's own, each marked as a custom operation
//! (`docs/mesh/custom-operations.md`). Deciding which way a closed piece should point is
//! manifold-rust's winding repair. Each fix returns the new mesh and how many things it changed;
//! normals, texture coordinates and colours follow the faces they belong to.

use crate::check::{EdgeUse, Topology, distance};
use crate::mesh::{NewCorner, index};
use crate::triangulate::{ear_clip, newell_normal};
use crate::{Attribute, Mesh, Point};
use std::collections::{BTreeMap, HashMap, HashSet};

impl Mesh {
    /// Faces turned so each piece points one way, as Rhino's UnifyMeshNormals: the way most of the
    /// piece's faces already point, then outwards for a closed piece, unless it is a hollow inside
    /// another and points in. Also says how many faces turned.
    ///
    /// Rhino keeps a closed piece pointing in when all its faces agree; Curva turns it out, as a
    /// solid that points in has a negative volume and fails in booleans.
    #[must_use]
    pub fn unified(&self) -> (Self, usize) {
        let (turn, count) = Topology::of(self).orientation(self);
        if count == 0 {
            return (self.clone(), 0);
        }
        let faces: Vec<Vec<NewCorner>> = (0..self.face_count())
            .map(|f| {
                let range = self.face_range(f);
                let order: Vec<usize> = if turn[f] {
                    range.rev().collect()
                } else {
                    range.collect()
                };
                self.corners_of(&order, turn[f])
            })
            .collect();
        (self.rebuilt(&self.all_vertices(), &faces), count)
    }

    /// The faces with no area taken out (see [`crate::MeshCheck::degenerate_faces`]), and how many.
    // CUSTOM MESH OPERATION: cull-degenerate-faces
    #[must_use]
    pub fn without_degenerate_faces(&self) -> (Self, usize) {
        let topology = Topology::of(self);
        self.keeping_faces(|f| !topology.is_degenerate(self, f))
    }

    /// Faces on the same places as an earlier face taken out, and how many.
    // CUSTOM MESH OPERATION: cull-duplicate-faces
    #[must_use]
    pub fn without_duplicate_faces(&self) -> (Self, usize) {
        let topology = Topology::of(self);
        let mut seen = HashSet::new();
        let keep: Vec<bool> = (0..self.face_count())
            .map(|f| {
                let mut key: Vec<u32> = topology.face(self, f).collect();
                key.sort_unstable();
                seen.insert(key)
            })
            .collect();
        self.keeping_faces(|f| keep[f])
    }

    /// Points no face uses taken out, and how many.
    // CUSTOM MESH OPERATION: cull-unused-vertices
    #[must_use]
    pub fn without_unused_vertices(&self) -> (Self, usize) {
        let mut used = vec![false; self.vertex_count()];
        for &v in self.corners() {
            used[v as usize] = true;
        }
        let kept: Vec<u32> = (0..self.vertex_count())
            .filter(|&v| used[v])
            .map(index)
            .collect();
        let removed = self.vertex_count() - kept.len();
        if removed == 0 {
            return (self.clone(), 0);
        }
        let mut new = vec![0; self.vertex_count()];
        for (i, &v) in kept.iter().enumerate() {
            new[v as usize] = index(i);
        }
        let faces = self.faces_mapped(|v| new[v as usize]);
        (self.rebuilt(&kept, &faces), removed)
    }

    /// Points closer than `distance` merged into one, as Rhino's Weld, and how many points went.
    /// Each merged point stays where its lowest-numbered point was. Faces that lose corners keep
    /// the rest; faces left with fewer than three, or folded onto a point twice, go. A distance of
    /// zero merges only points at exactly the same place.
    ///
    /// Normals, texture coordinates and colours stay as they were on each corner, so a hard edge
    /// or a texture seam survives the weld; [`Mesh::with_rebuilt_normals`] smooths the normals.
    // CUSTOM MESH OPERATION: weld-vertices
    #[must_use]
    pub fn welded(&self, distance_apart: f64) -> (Self, usize) {
        let to = self.merged_points(distance_apart);
        let kept: Vec<u32> = (0..self.vertex_count())
            .filter(|&v| to[v] as usize == v)
            .map(index)
            .collect();
        let removed = self.vertex_count() - kept.len();
        if removed == 0 {
            return (self.clone(), 0);
        }
        let mut new = vec![0; self.vertex_count()];
        for (i, &v) in kept.iter().enumerate() {
            new[v as usize] = index(i);
        }
        let mut faces = Vec::with_capacity(self.face_count());
        for f in 0..self.face_count() {
            let range = self.face_range(f);
            let mut face: Vec<NewCorner> = Vec::with_capacity(range.len());
            for c in range {
                let vertex = new[to[self.corners()[c] as usize] as usize];
                if face.last().is_none_or(|last| last.vertex != vertex) {
                    face.push(NewCorner {
                        vertex,
                        from: c,
                        flip: false,
                    });
                }
            }
            while face.len() > 1 && face[0].vertex == face[face.len() - 1].vertex {
                face.pop();
            }
            let folded =
                (1..face.len()).any(|i| face[..i].iter().any(|c| c.vertex == face[i].vertex));
            if face.len() >= 3 && !folded {
                faces.push(face);
            }
        }
        (self.rebuilt(&kept, &faces), removed)
    }

    /// For each point, the point it merges into: the lowest-numbered one in its cluster.
    fn merged_points(&self, distance_apart: f64) -> Vec<u32> {
        if distance_apart <= 0.0 || !distance_apart.is_finite() {
            return self.places();
        }
        let mut parent: Vec<usize> = (0..self.vertex_count()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        // Points go into cubes of the weld distance; only neighbouring cubes can hold a match.
        #[allow(clippy::cast_possible_truncation)]
        let cell = |p: &Point| p.map(|c| (c / distance_apart).floor() as i64);
        let mut cells: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
        for (i, p) in self.vertices().iter().enumerate() {
            cells.entry(cell(p)).or_default().push(i);
        }
        for (i, p) in self.vertices().iter().enumerate() {
            let [x, y, z] = cell(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(others) = cells.get(&[x + dx, y + dy, z + dz]) else {
                            continue;
                        };
                        for &j in others {
                            if j > i && distance(*p, self.vertices()[j]) <= distance_apart {
                                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                                parent[a.max(b)] = a.min(b);
                            }
                        }
                    }
                }
            }
        }
        (0..self.vertex_count())
            .map(|i| index(root(&mut parent, i)))
            .collect()
    }

    /// Normals worked out again from the faces, as Rhino's RebuildMeshNormals: each the average of
    /// the faces around it, weighted by their area.
    ///
    /// Where a point had several normals (a hard edge), each keeps its own side of the edge, so
    /// creases stay sharp; a mesh without normals gets one per point.
    // CUSTOM MESH OPERATION: rebuild-normals
    #[must_use]
    pub fn with_rebuilt_normals(&self) -> Self {
        let old = self.normals();
        let mut group: HashMap<(u32, Option<[u64; 3]>), usize> = HashMap::new();
        let mut corner_group = vec![0; self.corners().len()];
        let mut sums: Vec<Point> = Vec::new();
        for f in 0..self.face_count() {
            let points: Vec<Point> = self
                .face(f)
                .iter()
                .map(|&v| self.vertices()[v as usize])
                .collect();
            let n = newell_normal(&points);
            for c in self.face_range(f) {
                let v = self.corners()[c];
                let key = (v, old.at(c, v as usize).map(|n| n.map(f64::to_bits)));
                let next = sums.len();
                let g = *group.entry(key).or_insert(next);
                if g == next {
                    sums.push([0.0; 3]);
                }
                corner_group[c] = g;
                for i in 0..3 {
                    sums[g][i] += n[i];
                }
            }
        }
        let unit = |s: Point| {
            let l = distance(s, [0.0; 3]);
            if l > 0.0 { s.map(|c| c / l) } else { [0.0; 3] }
        };
        let groups_per_vertex = {
            let mut count = vec![0; self.vertex_count()];
            for &(v, _) in group.keys() {
                count[v as usize] += 1;
            }
            count
        };
        let normals = if groups_per_vertex.iter().all(|&n| n <= 1) {
            let mut per_vertex = vec![[0.0; 3]; self.vertex_count()];
            for (&(v, _), &g) in &group {
                per_vertex[v as usize] = unit(sums[g]);
            }
            Attribute::PerVertex(per_vertex)
        } else {
            Attribute::PerCorner(corner_group.iter().map(|&g| unit(sums[g])).collect())
        };
        self.clone()
            .with_normals(normals)
            .unwrap_or_else(|_| self.clone())
    }

    /// Each hole closed with triangles, as Rhino's FillMeshHoles, and how many holes. A hole is a
    /// loop of open edges; one that touches itself at a point is filled as two holes.
    // CUSTOM MESH OPERATION: fill-holes
    #[must_use]
    pub fn with_holes_filled(&self) -> (Self, usize) {
        let topology = Topology::of(self);
        let mut naked: Vec<EdgeUse> = topology
            .edges
            .values()
            .filter(|uses| uses.len() == 1)
            .map(|uses| uses[0])
            .collect();
        naked.sort_unstable_by_key(|u| (u.face, u.corner));
        // A hole runs along each open edge the other way from its face.
        let mut leaving: BTreeMap<u32, Vec<(usize, HoleCorner)>> = BTreeMap::new();
        let mut ends = Vec::with_capacity(naked.len());
        for (i, u) in naked.iter().enumerate() {
            let range = self.face_range(u.face);
            let n = range.len();
            let a = self.corners()[range.start + u.corner];
            let at_b = range.start + (u.corner + 1) % n;
            let b = self.corners()[at_b];
            let start = HoleCorner {
                place: topology.place[b as usize],
                vertex: b,
                from: at_b,
            };
            leaving.entry(start.place).or_default().push((i, start));
            ends.push(topology.place[a as usize]);
        }
        let mut used = vec![false; naked.len()];
        let mut loops: Vec<Vec<HoleCorner>> = Vec::new();
        for first in 0..naked.len() {
            if used[first] {
                continue;
            }
            let Some(start) = leaving
                .values()
                .flatten()
                .find(|(i, _)| *i == first)
                .map(|e| e.1)
            else {
                continue;
            };
            used[first] = true;
            let mut ring = vec![start];
            let mut here = ends[first];
            let mut closed = true;
            while here != start.place {
                let next = leaving
                    .get(&here)
                    .and_then(|out| out.iter().find(|(i, _)| !used[*i]).copied());
                let Some((i, corner)) = next else {
                    closed = false;
                    break;
                };
                used[i] = true;
                ring.push(corner);
                here = ends[i];
            }
            if closed {
                loops.extend(simple_loops(ring));
            }
        }

        let mut faces: Vec<Vec<NewCorner>> = (0..self.face_count())
            .map(|f| self.corners_of(&self.face_range(f).collect::<Vec<_>>(), false))
            .collect();
        let mut filled = 0;
        for ring in loops.iter().filter(|r| r.len() >= 3) {
            let points: Vec<Point> = ring
                .iter()
                .map(|c| self.vertices()[c.vertex as usize])
                .collect();
            for triangle in ear_clip(&points) {
                faces.push(
                    triangle
                        .iter()
                        .map(|&k| NewCorner {
                            vertex: ring[k].vertex,
                            from: ring[k].from,
                            flip: false,
                        })
                        .collect(),
                );
            }
            filled += 1;
        }
        if filled == 0 {
            return (self.clone(), 0);
        }
        (self.rebuilt(&self.all_vertices(), &faces), filled)
    }

    pub(crate) fn all_vertices(&self) -> Vec<u32> {
        (0..self.vertex_count()).map(index).collect()
    }

    pub(crate) fn corners_of(&self, corners: &[usize], flip: bool) -> Vec<NewCorner> {
        corners
            .iter()
            .map(|&c| NewCorner {
                vertex: self.corners()[c],
                from: c,
                flip,
            })
            .collect()
    }

    fn faces_mapped(&self, vertex: impl Fn(u32) -> u32) -> Vec<Vec<NewCorner>> {
        (0..self.face_count())
            .map(|f| {
                self.face_range(f)
                    .map(|c| NewCorner {
                        vertex: vertex(self.corners()[c]),
                        from: c,
                        flip: false,
                    })
                    .collect()
            })
            .collect()
    }

    pub(crate) fn keeping_faces(&self, keep: impl Fn(usize) -> bool) -> (Self, usize) {
        let faces: Vec<Vec<NewCorner>> = (0..self.face_count())
            .filter(|&f| keep(f))
            .map(|f| self.corners_of(&self.face_range(f).collect::<Vec<_>>(), false))
            .collect();
        let removed = self.face_count() - faces.len();
        if removed == 0 {
            return (self.clone(), 0);
        }
        (self.rebuilt(&self.all_vertices(), &faces), removed)
    }
}

/// A corner of a hole's filling: its place, the vertex there, and the corner beside the hole whose
/// normal, texture coordinate and colour it copies.
#[derive(Clone, Copy, Debug)]
struct HoleCorner {
    place: u32,
    vertex: u32,
    from: usize,
}

/// A loop of corners split where it passes a place twice, into loops that do not.
fn simple_loops(ring: Vec<HoleCorner>) -> Vec<Vec<HoleCorner>> {
    let mut out = Vec::new();
    let mut stack: Vec<HoleCorner> = Vec::with_capacity(ring.len());
    for corner in ring {
        if let Some(i) = stack.iter().position(|c| c.place == corner.place) {
            out.push(stack.split_off(i));
        }
        stack.push(corner);
    }
    out.push(stack);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::tests::{cube_turning, cube_with, strip};
    use crate::mesh::tests::cube;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    /// The cube as six separate squares, 24 points, each square's points carrying its normal.
    fn loose_cube() -> Mesh {
        let cube = cube();
        let mut points = Vec::new();
        let mut normals = Vec::new();
        let mut faces = Vec::new();
        for face in cube.faces() {
            let corners: Vec<Point> = face.iter().map(|&v| cube.vertices()[v as usize]).collect();
            let n = newell_normal(&corners).map(|c| c / 2.0);
            let start = index(points.len());
            points.extend(corners);
            normals.extend([n; 4]);
            faces.push([start, start + 1, start + 2, start + 3]);
        }
        Mesh::new(points, &faces)
            .unwrap()
            .with_normals(Attribute::PerVertex(normals))
            .unwrap()
    }

    #[test]
    fn unifying_turns_faces_pointing_in_and_leaves_a_good_cube() {
        let (fixed, turned) = cube_turning(&[0, 1, 2, 3]).unified();
        assert_eq!(turned, 4);
        assert!(close(fixed.volume().unwrap(), 1.0));
        assert!(fixed.check(1e-4).is_good());
        assert_eq!(cube().unified(), (cube(), 0));
    }

    #[test]
    fn unifying_turns_an_inside_out_cube_the_right_way_out() {
        let (fixed, turned) = cube_turning(&[0, 1, 2, 3, 4, 5]).unified();
        assert_eq!(turned, 6);
        assert!(close(fixed.volume().unwrap(), 1.0));
    }

    #[test]
    fn unifying_keeps_a_hollow_pointing_in() {
        // A 3-unit cube with the unit cube inside it pointing in: a solid with a hollow. Turned
        // wholly inside out, unifying puts it back; as it is, unifying leaves it alone.
        let outer = cube().mapped(&curva_kernel::Similarity::scaling([0.0; 3], 3.0));
        let inner = cube().translated([1.0, 1.0, 1.0]);
        let mut points = outer.vertices().to_vec();
        points.extend_from_slice(inner.vertices());
        let mut faces: Vec<Vec<u32>> = outer.faces().map(<[u32]>::to_vec).collect();
        faces.extend(
            inner
                .faces()
                .map(|f| f.iter().rev().map(|v| v + 8).collect::<Vec<_>>()),
        );
        let hollow = Mesh::new(points, &faces).unwrap();
        assert!(close(hollow.volume().unwrap(), 26.0));
        assert_eq!(hollow.unified(), (hollow.clone(), 0));
        let (fixed, turned) = hollow.flipped().unified();
        assert_eq!(turned, 12);
        assert_eq!(fixed, hollow);
    }

    #[test]
    fn unifying_an_open_mesh_keeps_the_way_most_faces_point() {
        let (fixed, turned) = strip(&[1, 2]).unified();
        assert_eq!((fixed, turned), (strip(&[0, 1, 2]), 1));
    }

    #[test]
    fn turned_faces_turn_their_normals_too() {
        let normals = vec![[0.0, 0.0, 1.0]; 8];
        let mesh = strip(&[1])
            .with_normals(Attribute::PerVertex(normals))
            .unwrap();
        let (fixed, _) = mesh.unified();
        let Attribute::PerCorner(n) = fixed.normals() else {
            panic!("a point's two faces now disagree, so normals go per corner");
        };
        assert_eq!(&n[4..8], &[[0.0, 0.0, -1.0]; 4]);
        assert_eq!(&n[0..4], &[[0.0, 0.0, 1.0]; 4]);
    }

    #[test]
    fn degenerate_duplicate_faces_and_unused_points_come_out() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let flat = Mesh::new(points, &[[0, 1, 2], [0, 1, 3]]).unwrap();
        let (fixed, removed) = flat.without_degenerate_faces();
        assert_eq!((fixed.face_count(), removed), (1, 1));
        assert_eq!(fixed.face(0), &[0, 1, 3]);

        let (fixed, removed) = cube_with(&[[4, 0, 1, 5]], false).without_duplicate_faces();
        assert_eq!((fixed, removed), (cube(), 1));

        let (fixed, removed) = cube_with(&[], true).without_unused_vertices();
        assert_eq!((fixed, removed), (cube(), 1));
    }

    #[test]
    fn welding_joins_loose_faces_into_a_closed_cube_and_keeps_hard_edges() {
        let loose = loose_cube();
        // Its squares already meet by place, as Rhino sees them: closed before the weld too.
        assert_eq!(loose.check(1e-4).naked_edges, 0);
        assert!(loose.is_closed());
        let (welded, removed) = loose.welded(0.0);
        assert_eq!((welded.vertex_count(), removed), (8, 16));
        assert!(welded.is_closed());
        assert!(close(welded.volume().unwrap(), 1.0));
        // Each corner keeps its square's normal, so the cube still shades with sharp edges.
        let Attribute::PerCorner(n) = welded.normals() else {
            panic!("three normals meet at each point")
        };
        assert_eq!(n[0], [0.0, 0.0, -1.0]);
        assert_eq!(n[4], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn welding_merges_points_within_the_distance_only() {
        let mut points = cube().vertices().to_vec();
        points.push([1.0 + 1e-4, 1.0, 1.0]);
        let faces: Vec<Vec<u32>> = cube()
            .faces()
            .map(|f| {
                f.iter()
                    .map(|&v| if v == 6 && f[0] == 4 { 8 } else { v })
                    .collect()
            })
            .collect();
        let mesh = Mesh::new(points, &faces).unwrap();
        assert_eq!(mesh.welded(1e-5).1, 0);
        let (welded, removed) = mesh.welded(1e-3);
        assert_eq!(removed, 1);
        assert!(welded.is_closed());
        assert_eq!(
            welded.vertices()[6],
            [1.0, 1.0, 1.0],
            "the lower-numbered point stays"
        );
    }

    #[test]
    fn welding_drops_faces_that_collapse() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0 + 1e-6, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let mesh = Mesh::new(points, &[[0, 1, 3], [1, 2, 3]]).unwrap();
        let (welded, removed) = mesh.welded(1e-3);
        assert_eq!((removed, welded.face_count()), (1, 1));
    }

    #[test]
    fn rebuilt_normals_are_smooth_on_a_welded_mesh_and_sharp_on_hard_edges() {
        let smooth = cube().with_rebuilt_normals();
        let Attribute::PerVertex(n) = smooth.normals() else {
            panic!("one normal per point")
        };
        let third = 1.0 / 3.0_f64.sqrt();
        assert!(n[6].iter().all(|&c| close(c, third)));

        let sharp = loose_cube().welded(0.0).0.with_rebuilt_normals();
        assert_eq!(sharp.normals(), loose_cube().welded(0.0).0.normals());
    }

    #[test]
    fn filling_closes_an_open_box_pointing_out() {
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(1).map(<[u32]>::to_vec).collect();
        let open = Mesh::new(cube.vertices().to_vec(), &faces).unwrap();
        let (filled, holes) = open.with_holes_filled();
        assert_eq!((holes, filled.face_count()), (1, 7));
        assert!(close(filled.volume().unwrap(), 1.0));
        assert!(filled.check(1e-4).is_good());
    }

    #[test]
    fn filling_a_tube_closes_both_ends() {
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(2).map(<[u32]>::to_vec).collect();
        let tube = Mesh::new(cube.vertices().to_vec(), &faces).unwrap();
        let (filled, holes) = tube.with_holes_filled();
        assert_eq!(holes, 2);
        assert!(close(filled.volume().unwrap(), 1.0));
    }

    #[test]
    fn a_filled_hole_takes_the_texture_of_the_corners_beside_it() {
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(1).map(<[u32]>::to_vec).collect();
        let texture: Vec<[f64; 2]> = (0..8).map(|i| [f64::from(i), 0.0]).collect();
        let open = Mesh::new(cube.vertices().to_vec(), &faces)
            .unwrap()
            .with_texture(Attribute::PerVertex(texture.clone()))
            .unwrap();
        let (filled, _) = open.with_holes_filled();
        assert_eq!(filled.texture(), &Attribute::PerVertex(texture));
    }
}
