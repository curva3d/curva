//! Rhino 8's `UnweldEdge` for meshes. Measured (23 November 2026, RhinoCommon picks of topology
//! edges on a 4 m mesh box of 2 × 2 × 2 faces, welded, `-UnweldEdge` with its starting options
//! ModifyNormals=Yes and SharpCorners=Yes):
//!
//! - every point of a picked edge gets one more vertex at its place, ends included (one edge:
//!   26 vertices become 28; two in a row, 29); faces stay as they were;
//! - in the middle of a run the faces round the point split along the picked edges, each side
//!   on its own vertex;
//! - at an end, the one face the picked edge leaves the point from (going round the face in its
//!   order) gets the new vertex alone;
//! - with ModifyNormals each of those vertices gets the normal of the faces round the point,
//!   leaving out those across a picked edge from its own faces (folded corner of the box, one
//!   edge: (−0.447, −0.894, 0) and (−0.894, −0.447, 0) at the end where four faces meet); without
//!   it, every copy keeps the old normal.
//!
//! SharpCorners changed nothing in these cases and is not here.

use crate::mesh::Attribute;
use crate::triangulate::newell_normal;
use crate::{Mesh, MeshError, Point};
use std::collections::{BTreeMap, BTreeSet};

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// One face corner at a point being unwelded: its face, where it sits in `corners`, and the
/// places before and after it round the face.
#[derive(Clone, Copy)]
struct Corner {
    face: usize,
    at: usize,
    here: u32,
    before: u32,
    after: u32,
}

/// The corners round one point, grouped into the pieces they part into.
fn pieces(fan: &[Corner], cuts: &BTreeSet<u32>) -> Vec<Vec<usize>> {
    let mut group: Vec<usize> = (0..fan.len()).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    let mut by_edge: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, c) in fan.iter().enumerate() {
        by_edge.entry(c.before).or_default().push(i);
        by_edge.entry(c.after).or_default().push(i);
    }
    for (w, corners) in &by_edge {
        if cuts.contains(w) {
            continue;
        }
        for pair in corners.windows(2) {
            let (a, b) = (root(&mut group, pair[0]), root(&mut group, pair[1]));
            group[a] = b;
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..fan.len() {
        out.entry(root(&mut group, i)).or_default().push(i);
    }
    out.into_values().collect()
}

/// `values` with a copy of vertex `source[k]`'s value added for each new vertex.
fn extended<T: Clone>(values: &Attribute<T>, sources: &[u32]) -> Attribute<T> {
    match values {
        Attribute::PerVertex(v) => {
            let added: Vec<T> = sources.iter().map(|&s| v[s as usize].clone()).collect();
            let mut v = v.clone();
            v.extend(added);
            Attribute::PerVertex(v)
        }
        other => other.clone(),
    }
}

/// The angle a face makes at the point of corner `c`, whose own place is `c.here`.
fn corner_angle(points: &[Point], c: &Corner) -> f64 {
    let p = points[c.here as usize];
    let (a, b) = (points[c.before as usize], points[c.after as usize]);
    let u: Point = std::array::from_fn(|k| a[k] - p[k]);
    let w: Point = std::array::from_fn(|k| b[k] - p[k]);
    let dot: f64 = (0..3).map(|k| u[k] * w[k]).sum();
    let len = |q: Point| (0..3).map(|k| q[k] * q[k]).sum::<f64>().sqrt();
    (dot / (len(u) * len(w)).max(f64::MIN_POSITIVE))
        .clamp(-1.0, 1.0)
        .acos()
}

fn unit(s: Point) -> Point {
    let l = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    if l > 0.0 { s.map(|c| c / l) } else { s }
}

/// The normal of the faces round a point for one piece `part` of its `fan`: all of them but
/// those across a picked edge (to the places `picked_at`) from the piece's own faces.
fn side_normal(
    fan: &[Corner],
    part: &[usize],
    picked_at: &BTreeSet<u32>,
    faces: &[Point],
    points: &[Point],
) -> Point {
    let edges_here: BTreeSet<u32> = part
        .iter()
        .flat_map(|&i| [fan[i].before, fan[i].after])
        .filter(|w| picked_at.contains(w))
        .collect();
    let mut sum = [0.0; 3];
    for (i, c) in fan.iter().enumerate() {
        let across =
            !part.contains(&i) && (edges_here.contains(&c.before) || edges_here.contains(&c.after));
        if !across {
            // Each face counts by its angle at the point, as Rhino's normals do (measured on a
            // bevelled box, where a smaller face next to the point counts the same).
            let weight = corner_angle(points, c);
            let n = unit(faces[c.face]);
            (0..3).for_each(|k| sum[k] += weight * n[k]);
        }
    }
    unit(sum)
}

impl Mesh {
    /// Each place's face corners, and each edge (by place) with how many faces use it.
    #[allow(clippy::type_complexity)]
    fn fans(&self, place: &[u32]) -> (BTreeMap<u32, Vec<Corner>>, BTreeMap<(u32, u32), usize>) {
        let at = |v: u32| place[v as usize];
        let mut fans: BTreeMap<u32, Vec<Corner>> = BTreeMap::new();
        let mut uses: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for f in 0..self.face_count() {
            let ring = self.face(f);
            let n = ring.len();
            for (k, c) in self.face_range(f).enumerate() {
                let (before, here, after) = (
                    at(ring[(k + n - 1) % n]),
                    at(ring[k]),
                    at(ring[(k + 1) % n]),
                );
                fans.entry(here).or_default().push(Corner {
                    face: f,
                    at: c,
                    here,
                    before,
                    after,
                });
                *uses.entry((here.min(after), here.max(after))).or_default() += 1;
            }
        }
        (fans, uses)
    }

    /// Each face's normal, as long as twice its area.
    fn face_normals(&self) -> Vec<Point> {
        (0..self.face_count())
            .map(|f| {
                let points: Vec<Point> = self
                    .face(f)
                    .iter()
                    .map(|&v| self.vertices()[v as usize])
                    .collect();
                newell_normal(&points)
            })
            .collect()
    }
}

impl Mesh {
    /// The picked edges unwelded, as Rhino's `UnweldEdge`: each point on them gets a vertex for
    /// each piece its faces part into (see the module notes), and with `modify_normals` each new
    /// vertex the normal of its side. Edges are given by their end vertices; vertices at one
    /// place count as one, as Rhino joins faces.
    ///
    /// # Errors
    /// When no picked edge is an inner edge of the mesh.
    pub fn with_edges_unwelded(
        &self,
        edges: &[[u32; 2]],
        modify_normals: bool,
    ) -> Result<Self, MeshError> {
        let place = self.places();
        let at = |v: u32| place[v as usize];
        let (fans, uses) = self.fans(&place);
        let picked: BTreeSet<(u32, u32)> = edges
            .iter()
            .map(|&[a, b]| (at(a).min(at(b)), at(a).max(at(b))))
            .filter(|key| uses.get(key) == Some(&2))
            .collect();
        if picked.is_empty() {
            return Err(MeshError::UnusableEdge {
                edge: edges.first().copied().unwrap_or([0, 0]),
            });
        }
        let mut cuts: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for &(a, b) in &picked {
            cuts.entry(a).or_default().insert(b);
            cuts.entry(b).or_default().insert(a);
        }
        let face_normals = self.face_normals();
        let mut corners = self.corners().to_vec();
        let mut points = self.vertices().to_vec();
        let mut sources: Vec<u32> = Vec::new();
        // The normal each changed corner gets, when normals are changed.
        let mut set_normal: Vec<(usize, Point)> = Vec::new();
        for (&v, cut) in &mut cuts {
            let fan = &fans[&v];
            let picked_at = cut.clone();
            if cut.len() == 1 {
                // An end: the face the picked edge leaves from parts off on its own.
                let o = *cut.iter().next().unwrap_or(&v);
                if let Some(c) = fan.iter().find(|c| c.after == o) {
                    cut.insert(c.before);
                }
            }
            let mut claimed: BTreeSet<u32> = BTreeSet::new();
            for part in &pieces(fan, cut) {
                let own: BTreeSet<u32> = part.iter().map(|&i| self.corners()[fan[i].at]).collect();
                let first = *own.iter().next().unwrap_or(&v);
                let vertex = if own.len() == 1 && claimed.insert(first) {
                    first
                } else {
                    points.push(points[first as usize]);
                    sources.push(first);
                    index(points.len() - 1)
                };
                for &i in part {
                    corners[fan[i].at] = vertex;
                }
                let normal = side_normal(fan, part, &picked_at, &face_normals, self.vertices());
                set_normal.extend(part.iter().map(|&i| (fan[i].at, normal)));
            }
        }
        self.unwelded_on(
            points,
            &corners,
            &sources,
            modify_normals.then_some(&set_normal),
        )
    }

    /// This mesh's faces on `corners` over `points`, its attributes carried over (new vertices
    /// copying vertex `sources[k]`), and the normals at the corners in `normals` set.
    fn unwelded_on(
        &self,
        points: Vec<Point>,
        corners: &[u32],
        sources: &[u32],
        normals: Option<&Vec<(usize, Point)>>,
    ) -> Result<Self, MeshError> {
        let faces: Vec<&[u32]> = (0..self.face_count())
            .map(|f| &corners[self.face_range(f)])
            .collect();
        let mut out_normals = extended(self.normals(), sources);
        if let Some(set) = normals {
            match &mut out_normals {
                Attribute::PerVertex(values) => {
                    for &(c, n) in set {
                        values[corners[c] as usize] = n;
                    }
                }
                Attribute::PerCorner(values) => {
                    for &(c, n) in set {
                        values[c] = n;
                    }
                }
                Attribute::None => {}
            }
        }
        Mesh::new(points, &faces)?
            .with_normals(out_normals)?
            .with_texture(extended(self.texture(), sources))?
            .with_colours(extended(self.colours(), sources))
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)] // normals rounded to 4 places, as compared
mod tests {
    use super::*;

    /// Rhino's 4 m mesh box of 2 × 2 × 2 faces at x = 200, welded, with its normals.
    fn mesh_box() -> Mesh {
        let subd = crate::SubD::subd_box(
            &crate::Frame {
                origin: [200.0, 0.0, 0.0],
                ..crate::Frame::world()
            },
            [4.0; 3],
            [2, 2, 2],
        );
        subd.cage().clone().with_rebuilt_normals()
    }

    fn at(mesh: &Mesh, p: Point) -> u32 {
        index(mesh.vertices().iter().position(|&q| q == p).unwrap())
    }

    /// The normals at the vertices at `p`, rounded, sorted.
    fn normals_at(mesh: &Mesh, p: Point) -> Vec<[f64; 3]> {
        let Attribute::PerVertex(n) = mesh.normals() else {
            panic!("per-vertex normals")
        };
        let mut out: Vec<[f64; 3]> = (0..mesh.vertex_count())
            .filter(|&v| mesh.vertices()[v] == p)
            .map(|v| n[v].map(|c| (c * 1e4).round() / 1e4))
            .collect();
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }

    #[test]
    fn one_folded_edge_unwelds_as_in_rhino() {
        let mesh = mesh_box();
        assert_eq!(mesh.vertex_count(), 26);
        let edge = [at(&mesh, [200.0, 0.0, 0.0]), at(&mesh, [200.0, 0.0, 2.0])];
        let out = mesh.with_edges_unwelded(&[edge], true).unwrap();
        assert_eq!((out.vertex_count(), out.face_count()), (28, 24));
        assert_eq!(
            normals_at(&out, [200.0, 0.0, 2.0]),
            vec![[-0.8944, -0.4472, 0.0], [-0.4472, -0.8944, 0.0]]
        );
        assert_eq!(
            normals_at(&out, [200.0, 0.0, 0.0]),
            vec![[-0.7071, 0.0, -0.7071], [0.0, -0.7071, -0.7071]]
        );
        let kept = mesh.with_edges_unwelded(&[edge], false).unwrap();
        assert_eq!(
            normals_at(&kept, [200.0, 0.0, 2.0]),
            vec![[-0.7071, -0.7071, 0.0]; 2]
        );
    }

    #[test]
    fn two_edges_in_a_row_unweld_as_in_rhino() {
        let mesh = mesh_box();
        let p = |z: f64| at(&mesh, [200.0, 0.0, z]);
        let out = mesh
            .with_edges_unwelded(&[[p(0.0), p(2.0)], [p(2.0), p(4.0)]], true)
            .unwrap();
        assert_eq!((out.vertex_count(), out.face_count()), (29, 24));
        assert_eq!(
            normals_at(&out, [200.0, 0.0, 2.0]),
            vec![[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]]
        );
        assert_eq!(
            normals_at(&out, [200.0, 0.0, 4.0]),
            vec![[-0.7071, 0.0, 0.7071], [0.0, -0.7071, 0.7071]]
        );
        assert_eq!(out.edge_count(), mesh.edge_count(), "faces still join");
    }

    #[test]
    fn a_flat_run_copies_every_point() {
        let mesh = mesh_box();
        let p = |z: f64| at(&mesh, [202.0, 0.0, z]);
        let out = mesh
            .with_edges_unwelded(&[[p(0.0), p(2.0)], [p(2.0), p(4.0)]], true)
            .unwrap();
        assert_eq!(out.vertex_count(), 29);
    }
}
