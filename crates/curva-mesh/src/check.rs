//! `Check` for meshes: what is wrong with a mesh, counted as Rhino 8's `Check` counts it.
//!
//! Like Rhino, edges are found between vertex *places*: two vertices at exactly the same place (an
//! unwelded edge) still join their faces, so an unwelded crease is not reported as an open edge.

use crate::triangulate::newell_normal;
use crate::{Mesh, Point};
use std::collections::{HashMap, HashSet};

/// What `Check` found in a mesh. Every count is zero for a good mesh.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeshCheck {
    /// Faces with no area: two corners at one place, or all corners on a line. As Rhino, a face
    /// counts when its height is under about 1.5 × 10⁻⁸ of its longest edge, at any size.
    pub degenerate_faces: usize,
    /// Edges no longer than the length asked for, but not zero long (those make degenerate faces).
    pub short_edges: usize,
    /// Edges used by three or more faces.
    pub non_manifold_edges: usize,
    /// Edges used by only one face: the mesh's open borders.
    pub naked_edges: usize,
    /// Faces on exactly the same places as an earlier face, whichever way round.
    pub duplicate_faces: usize,
    /// Faces that point the other way from the rest of their piece: as many as
    /// [`Mesh::unified`] would turn round. `None` when the mesh has non-manifold edges, where Rhino
    /// skips this count too. On an open piece Curva counts the fewer faces, where Rhino sometimes
    /// counts the others.
    pub misdirected_faces: Option<usize>,
    /// Whether faces cross through each other (faces on top of each other count). `None` when the
    /// mesh is open or has edges that do not pair up, which manifold-rust cannot test yet (Rhino
    /// counts crossing pairs on any mesh).
    pub crosses_itself: Option<bool>,
    /// How many separate pieces its faces form; 1 for a mesh in one piece.
    pub pieces: usize,
    /// Points no face uses.
    pub unused_vertices: usize,
    /// Stored normals that are not one unit long, zero-long ones among them.
    pub bad_normals: usize,
}

impl MeshCheck {
    /// Whether nothing is wrong, as Rhino's "This is a good mesh". A mesh may be open, or in
    /// several pieces, and still be good.
    #[must_use]
    pub fn is_good(&self) -> bool {
        self.degenerate_faces == 0
            && self.short_edges == 0
            && self.non_manifold_edges == 0
            && self.duplicate_faces == 0
            && self.misdirected_faces.unwrap_or(0) == 0
            && self.crosses_itself != Some(true)
            && self.unused_vertices == 0
            && self.bad_normals == 0
    }
}

impl Mesh {
    /// What is wrong with this mesh. Edges no longer than `short_edge` are reported as short. A
    /// tenth of the model's absolute tolerance matches Rhino 8: at a tolerance of 0.001 it reports
    /// an edge of 0.0001 as short and one of 0.00015 not, at any size of mesh.
    #[must_use]
    pub fn check(&self, short_edge: f64) -> MeshCheck {
        let topology = Topology::of(self);
        let non_manifold_edges = topology
            .edges
            .values()
            .filter(|uses| uses.len() > 2)
            .count();
        let short_edges = topology
            .edges
            .keys()
            .filter(|&&(a, b)| {
                let length = distance(self.vertices()[a as usize], self.vertices()[b as usize]);
                length > 0.0 && length <= short_edge
            })
            .count();
        let mut seen = HashSet::new();
        let duplicate_faces = (0..self.face_count())
            .filter(|&f| {
                let mut key: Vec<u32> = topology.face(self, f).collect();
                key.sort_unstable();
                !seen.insert(key)
            })
            .count();
        let mut used = vec![false; self.vertex_count()];
        for &v in self.corners() {
            used[v as usize] = true;
        }
        let bad_normals = match self.normals() {
            crate::Attribute::None => 0,
            crate::Attribute::PerVertex(n) | crate::Attribute::PerCorner(n) => n
                .iter()
                .filter(|n| (distance(**n, [0.0; 3]) - 1.0).abs() > 1e-6)
                .count(),
        };
        MeshCheck {
            degenerate_faces: (0..self.face_count())
                .filter(|&f| topology.is_degenerate(self, f))
                .count(),
            short_edges,
            non_manifold_edges,
            naked_edges: topology
                .edges
                .values()
                .filter(|uses| uses.len() == 1)
                .count(),
            duplicate_faces,
            misdirected_faces: (non_manifold_edges == 0).then(|| {
                let (turn, _) = topology.orientation(self);
                turn.iter().filter(|&&t| t).count()
            }),
            crosses_itself: crate::boolean::crosses_itself(self),
            pieces: topology.pieces(self.face_count()).1,
            unused_vertices: used.iter().filter(|&&u| !u).count(),
            bad_normals,
        }
    }
}

/// A use of an edge by a face: which face, at which of its corners the edge starts, and whether it
/// runs from the lower place to the higher.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeUse {
    pub face: usize,
    pub corner: usize,
    pub forward: bool,
}

/// How a mesh's faces meet, by vertex places (see [`Mesh::places`]).
pub(crate) struct Topology {
    /// Each vertex's place.
    pub place: Vec<u32>,
    /// Each edge, lower place first, with the faces that use it.
    pub edges: HashMap<(u32, u32), Vec<EdgeUse>>,
}

impl Topology {
    pub(crate) fn of(mesh: &Mesh) -> Self {
        let place = mesh.places();
        let mut edges: HashMap<(u32, u32), Vec<EdgeUse>> = HashMap::new();
        for f in 0..mesh.face_count() {
            let face = mesh.face(f);
            for (k, &a) in face.iter().enumerate() {
                let (a, b) = (
                    place[a as usize],
                    place[face[(k + 1) % face.len()] as usize],
                );
                if a == b {
                    continue;
                }
                edges
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push(EdgeUse {
                        face: f,
                        corner: k,
                        forward: a < b,
                    });
            }
        }
        Self { place, edges }
    }

    /// Face `f`'s places in order.
    pub(crate) fn face<'a>(&'a self, mesh: &'a Mesh, f: usize) -> impl Iterator<Item = u32> + 'a {
        mesh.face(f).iter().map(|&v| self.place[v as usize])
    }

    /// Whether face `f` has no area (see [`MeshCheck::degenerate_faces`]).
    pub(crate) fn is_degenerate(&self, mesh: &Mesh, f: usize) -> bool {
        let places: Vec<u32> = self.face(mesh, f).collect();
        if (1..places.len()).any(|i| places[..i].contains(&places[i])) {
            return true;
        }
        let points: Vec<Point> = mesh
            .face(f)
            .iter()
            .map(|&v| mesh.vertices()[v as usize])
            .collect();
        let longest = (0..points.len())
            .map(|k| distance(points[k], points[(k + 1) % points.len()]))
            .fold(0.0, f64::max);
        // Twice the area over the longest edge squared is the height over the longest edge for a
        // triangle; Rhino calls a face degenerate below about the square root of the machine
        // precision, found by measuring (1e-8 is degenerate, 3e-8 not, at sizes 0.001 to 1000).
        distance(newell_normal(&points), [0.0; 3]) < f64::EPSILON.sqrt() * longest * longest
    }

    /// Each face's piece, and how many pieces there are. Faces meeting at an edge are one piece.
    pub(crate) fn pieces(&self, faces: usize) -> (Vec<usize>, usize) {
        let mut parent: Vec<usize> = (0..faces).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for uses in self.edges.values() {
            for pair in uses.windows(2) {
                let (a, b) = (
                    root(&mut parent, pair[0].face),
                    root(&mut parent, pair[1].face),
                );
                parent[a.max(b)] = a.min(b);
            }
        }
        let mut number = HashMap::new();
        let piece: Vec<usize> = (0..faces)
            .map(|f| {
                let r = root(&mut parent, f);
                let next = number.len();
                *number.entry(r).or_insert(next)
            })
            .collect();
        (piece, number.len())
    }

    /// Which faces to turn round so each piece points one way, and how many faces that is.
    ///
    /// Faces agree across an edge two faces share when they run along it in opposite directions.
    /// Within each piece the direction most faces already have is kept; a closed piece pointing in
    /// is then turned out by manifold-rust's winding repair, unless it is a hollow inside another.
    // CUSTOM MESH OPERATION: unify-normals
    pub(crate) fn orientation(&self, mesh: &Mesh) -> (Vec<bool>, usize) {
        let faces = mesh.face_count();
        let mut neighbours: Vec<Vec<(usize, bool)>> = vec![Vec::new(); faces];
        for uses in self.edges.values() {
            if let [a, b] = uses[..] {
                // Running the same way along the edge means one of the two must turn.
                let differ = a.forward == b.forward;
                neighbours[a.face].push((b.face, differ));
                neighbours[b.face].push((a.face, differ));
            }
        }
        for n in &mut neighbours {
            n.sort_unstable();
        }
        let mut turn: Vec<Option<bool>> = vec![None; faces];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for seed in 0..faces {
            if turn[seed].is_some() {
                continue;
            }
            turn[seed] = Some(false);
            let mut group = vec![seed];
            let mut next = 0;
            while next < group.len() {
                let f = group[next];
                next += 1;
                let mine = turn[f].unwrap_or(false);
                for &(g, differ) in &neighbours[f] {
                    if turn[g].is_none() {
                        turn[g] = Some(mine != differ);
                        group.push(g);
                    }
                }
            }
            // Keep the direction most faces of the piece have (ties keep the first face's).
            let turned = group.iter().filter(|&&f| turn[f] == Some(true)).count();
            if turned * 2 > group.len() {
                for &f in &group {
                    turn[f] = turn[f].map(|t| !t);
                }
            }
            groups.push(group);
        }
        let mut turn: Vec<bool> = turn.into_iter().map(|t| t.unwrap_or(false)).collect();

        // Closed groups (no edge leading out of them used by just one face) are turned to point
        // out, all together so hollows inside solids point in.
        let mut closed_edge = vec![true; faces];
        for uses in self.edges.values() {
            if uses.len() != 2 {
                for u in uses {
                    closed_edge[u.face] = false;
                }
            }
        }
        let closed: Vec<&Vec<usize>> = groups
            .iter()
            .filter(|g| g.iter().all(|&f| closed_edge[f]))
            .collect();
        if !closed.is_empty() {
            let mut triangles = Vec::new();
            let mut first = Vec::with_capacity(closed.len());
            for group in &closed {
                first.push(triangles.len());
                for &f in group.iter() {
                    triangles.extend(face_triangles(mesh, f, turn[f]));
                }
            }
            let flip = crate::boolean::inside_out(&triangles);
            for (group, &start) in closed.iter().zip(&first) {
                if flip.get(start).copied().unwrap_or(false) {
                    for &f in group.iter() {
                        turn[f] = !turn[f];
                    }
                }
            }
        }
        let count = turn.iter().filter(|&&t| t).count();
        (turn, count)
    }
}

/// Face `f` as triangles of points, turned round when `turn` is set.
fn face_triangles(mesh: &Mesh, f: usize, turn: bool) -> Vec<[Point; 3]> {
    let at = |v: u32| mesh.vertices()[v as usize];
    let face = mesh.face(f);
    // A fan is enough here: only which way the face winds matters to the winding repair.
    (1..face.len() - 1)
        .map(|k| {
            let (a, b, c) = (at(face[0]), at(face[k]), at(face[k + 1]));
            if turn { [a, c, b] } else { [a, b, c] }
        })
        .collect()
}

pub(crate) fn distance(a: Point, b: Point) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh::tests::cube;

    /// The unit cube with the given faces turned round.
    pub(crate) fn cube_turning(turn: &[usize]) -> Mesh {
        let cube = cube();
        let faces: Vec<Vec<u32>> = (0..6)
            .map(|f| {
                let mut face = cube.face(f).to_vec();
                if turn.contains(&f) {
                    face.reverse();
                }
                face
            })
            .collect();
        Mesh::new(cube.vertices().to_vec(), &faces).unwrap()
    }

    /// The cube with extra faces, and optionally an unused point.
    pub(crate) fn cube_with(extra: &[[u32; 4]], unused: bool) -> Mesh {
        let cube = cube();
        let mut points = cube.vertices().to_vec();
        if unused {
            points.push([5.0, 5.0, 5.0]);
        }
        let mut faces: Vec<Vec<u32>> = cube.faces().map(<[u32]>::to_vec).collect();
        faces.extend(extra.iter().map(|f| f.to_vec()));
        Mesh::new(points, &faces).unwrap()
    }

    /// Three squares in a row in the xy plane, pointing up, with the given ones turned down.
    pub(crate) fn strip(turn: &[usize]) -> Mesh {
        let points: Vec<Point> = (0..4)
            .flat_map(|i| [[f64::from(i), 0.0, 0.0], [f64::from(i), 1.0, 0.0]])
            .collect();
        let faces: Vec<Vec<u32>> = (0..3u32)
            .map(|i| {
                let mut f = vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1];
                if turn.contains(&(i as usize)) {
                    f.reverse();
                }
                f
            })
            .collect();
        Mesh::new(points, &faces).unwrap()
    }

    fn triangle(points: [Point; 3]) -> Mesh {
        Mesh::new(points.to_vec(), &[[0, 1, 2]]).unwrap()
    }

    // The expected counts below are what Rhino 8's `Check` (`Mesh.Check` with self-intersection
    // and disjoint-piece checks on) reported for the same meshes, 17 November 2026.

    #[test]
    fn a_cube_is_a_good_mesh() {
        let report = cube().check(1e-4);
        assert_eq!(
            report,
            MeshCheck {
                misdirected_faces: Some(0),
                crosses_itself: Some(false),
                pieces: 1,
                ..MeshCheck::default()
            }
        );
        assert!(report.is_good());
    }

    #[test]
    fn an_unused_point_is_counted() {
        let report = cube_with(&[], true).check(1e-4);
        assert_eq!(report.unused_vertices, 1);
        assert!(!report.is_good());
    }

    #[test]
    fn faces_pointing_in_are_counted_as_rhino_counts_them() {
        assert_eq!(cube_turning(&[1]).check(1e-4).misdirected_faces, Some(1));
        assert_eq!(cube_turning(&[0, 1]).check(1e-4).misdirected_faces, Some(2));
        // Four of six point in: Rhino too counts four, the cube's outside deciding, not the most.
        assert_eq!(
            cube_turning(&[0, 1, 2, 3]).check(1e-4).misdirected_faces,
            Some(4)
        );
        // A cube turned wholly inside out is consistent but points in, so all six are counted.
        // (Rhino counts none here; Curva counts what UnifyMeshNormals would turn.)
        assert_eq!(
            cube_turning(&[0, 1, 2, 3, 4, 5])
                .check(1e-4)
                .misdirected_faces,
            Some(6)
        );
    }

    #[test]
    fn on_an_open_mesh_the_fewer_faces_count_as_misdirected() {
        assert_eq!(strip(&[0]).check(1e-4).misdirected_faces, Some(1));
        assert_eq!(strip(&[1]).check(1e-4).misdirected_faces, Some(1));
        // Rhino counts 2 here, keeping the way the lone face points; Curva keeps the most.
        assert_eq!(strip(&[1, 2]).check(1e-4).misdirected_faces, Some(1));
        assert_eq!(strip(&[0, 1, 2]).check(1e-4).misdirected_faces, Some(0));
    }

    #[test]
    fn an_open_box_has_four_naked_edges() {
        let cube = cube();
        let faces: Vec<Vec<u32>> = cube.faces().skip(1).map(<[u32]>::to_vec).collect();
        let report = Mesh::new(cube.vertices().to_vec(), &faces)
            .unwrap()
            .check(1e-4);
        assert_eq!((report.naked_edges, report.pieces), (4, 1));
        assert!(report.is_good(), "an open mesh can still be a good one");
    }

    #[test]
    fn a_doubled_face_is_a_duplicate_that_crosses_and_is_non_manifold() {
        let report = cube_with(&[[0, 1, 5, 4]], false).check(1e-4);
        assert_eq!(report.duplicate_faces, 1);
        assert_eq!(report.non_manifold_edges, 4);
        assert_eq!(report.misdirected_faces, None, "skipped, as Rhino skips it");
        // Rhino finds one crossing pair here; manifold-rust tests only meshes whose edges pair up.
        assert_eq!(report.crosses_itself, None);
    }

    #[test]
    fn faces_meet_by_place_not_by_point_number() {
        // Point 4 sits on point 0: Rhino sees (0, 1, 3) and (4, 1, 3) as the same face.
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let mesh = Mesh::new(points, &[[0, 1, 2], [0, 1, 3], [4, 1, 3]]).unwrap();
        let report = mesh.check(1e-4);
        assert_eq!(report.degenerate_faces, 1);
        assert_eq!(report.duplicate_faces, 1);
        assert_eq!(report.non_manifold_edges, 1);
        assert_eq!(report.naked_edges, 2);
        assert_eq!(report.unused_vertices, 0);
    }

    #[test]
    fn two_cubes_overlapping_in_one_mesh_cross_themselves() {
        let moved = cube().translated([0.5, 0.5, 0.5]);
        let mut points = cube().vertices().to_vec();
        points.extend_from_slice(moved.vertices());
        let mut faces: Vec<Vec<u32>> = cube().faces().map(<[u32]>::to_vec).collect();
        faces.extend(
            moved
                .faces()
                .map(|f| f.iter().map(|v| v + 8).collect::<Vec<_>>()),
        );
        let report = Mesh::new(points, &faces).unwrap().check(1e-4);
        assert_eq!((report.crosses_itself, report.pieces), (Some(true), 2));
    }

    #[test]
    fn two_cubes_apart_are_two_pieces() {
        let far = cube().translated([3.0, 0.0, 0.0]);
        let mut points = cube().vertices().to_vec();
        points.extend_from_slice(far.vertices());
        let mut faces: Vec<Vec<u32>> = cube().faces().map(<[u32]>::to_vec).collect();
        faces.extend(
            far.faces()
                .map(|f| f.iter().map(|v| v + 8).collect::<Vec<_>>()),
        );
        assert_eq!(Mesh::new(points, &faces).unwrap().check(1e-4).pieces, 2);
    }

    #[test]
    fn thin_faces_are_degenerate_below_rhinos_limit_at_any_size() {
        for size in [0.001, 1.0, 1000.0] {
            let thin = |h: f64| {
                triangle([
                    [0.0, 0.0, 0.0],
                    [size, 0.0, 0.0],
                    [size / 2.0, h * size, 0.0],
                ])
            };
            assert_eq!(thin(1e-8).check(0.0).degenerate_faces, 1, "size {size}");
            assert_eq!(thin(3e-8).check(0.0).degenerate_faces, 0, "size {size}");
            assert_eq!(thin(0.0).check(0.0).degenerate_faces, 1, "size {size}");
        }
    }

    #[test]
    fn short_edges_are_counted_by_length_and_zero_ones_are_degenerate() {
        let short = |e: f64| triangle([[0.0, 0.0, 0.0], [e, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert_eq!(short(1e-4).check(1e-4).short_edges, 1);
        assert_eq!(short(1.5e-4).check(1e-4).short_edges, 0);
        let zero = short(0.0).check(1e-4);
        assert_eq!((zero.short_edges, zero.degenerate_faces), (0, 1));
    }

    #[test]
    fn normals_that_are_not_one_unit_long_are_counted() {
        let normals = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 2.0], [0.0; 3]];
        let mesh = triangle([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
            .with_normals(crate::Attribute::PerVertex(normals))
            .unwrap();
        assert_eq!(mesh.check(1e-4).bad_normals, 2);
    }
}
