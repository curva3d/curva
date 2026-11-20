//! SketchUp faces as flat trimmed surfaces (Rhino's "Import faces as: Trimmed planes", issue
//! #432): each face built straight from its loops, its outer boundary and its holes, not from
//! triangles. SketchUp faces are already merged flat polygons, so the surfaces come out clean.
//! With "Join faces", the faces of a group or component that share edges are joined into
//! polysurfaces, which are solids where SketchUp's are closed.

use crate::{Reader, Report, placed};
use curva_document::blocks::Placement;
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve, Point};
use openskp::model::{Definition, EntityId};
use std::collections::{BTreeMap, HashMap};

/// One face read: its tag and material, its loops as closed curves, and the edges it uses.
struct Flat {
    tag: String,
    material: Option<EntityId>,
    loops: Vec<Curve>,
    edges: Vec<EntityId>,
}

impl Reader<'_> {
    /// The faces of `definition` as flat trimmed surfaces placed by `at`, joined where they
    /// share edges when the options say so.
    pub(crate) fn faces_as_planes(
        &mut self,
        doc: &mut Document,
        definition: &Definition,
        at: &Placement,
    ) -> Vec<u64> {
        let flats = self.flats(definition, at);
        let bodies = curva_kernel::parallel::each_in_parallel(flats.len(), |i| {
            Ok(Body::planar(&flats[i].loops).ok())
        })
        .unwrap_or_default();
        // Faces by tag and material, as the meshes are, so each keeps its layer and colour.
        let mut parts: BTreeMap<(String, Option<EntityId>), Vec<usize>> = BTreeMap::new();
        for (index, flat) in flats.iter().enumerate() {
            if bodies.get(index).is_some_and(Option::is_some) {
                parts
                    .entry((flat.tag.clone(), flat.material))
                    .or_default()
                    .push(index);
            } else {
                Report::count(&mut self.report.skipped, "unreadable faces", 1);
            }
        }
        let mut made = Vec::new();
        for ((tag, material), faces) in parts {
            let sets = if self.options.join_faces {
                connected(&faces, &flats)
            } else {
                faces.iter().map(|&f| vec![f]).collect()
            };
            for set in sets {
                let pieces: Vec<Body> = set.iter().filter_map(|&f| bodies[f].clone()).collect();
                for body in joined(pieces) {
                    let kind = if body.is_solid() {
                        "solids"
                    } else if body.face_count().unwrap_or(1) > 1 {
                        "polysurfaces"
                    } else {
                        "surfaces"
                    };
                    let id = self.placed_shape(doc, Shape::Body(body), &tag, material);
                    made.push(id);
                    Report::count(&mut self.report.brought, kind, 1);
                }
            }
        }
        made
    }

    /// The faces of `definition` that are shown, read as closed loops placed by `at`.
    fn flats(&mut self, definition: &Definition, at: &Placement) -> Vec<Flat> {
        let mut flats = Vec::new();
        let mut hidden = 0;
        for face in definition.faces.values() {
            if face.hidden {
                hidden += 1;
                continue;
            }
            let loops = crate::face_loops(definition, face);
            let curves: Vec<Curve> = loops
                .iter()
                .filter_map(|ids| {
                    let points: Vec<Point> = ids
                        .iter()
                        .filter_map(|id| definition.vertices.get(id))
                        .map(|v| placed([v.x * self.inch, v.y * self.inch, v.z * self.inch], at))
                        .collect();
                    Curve::polyline(points, true).ok()
                })
                .collect();
            if curves.is_empty() || curves.len() != loops.len() {
                Report::count(&mut self.report.skipped, "unreadable faces", 1);
                continue;
            }
            flats.push(Flat {
                tag: self.face_tag(face),
                material: face.material_id,
                loops: curves,
                edges: face.loops.iter().flatten().map(|c| c.edge_id).collect(),
            });
        }
        Report::count(&mut self.report.skipped, "hidden faces", hidden);
        flats
    }
}

/// `faces` (indices into `flats`) in sets that share edges, each set in the order first met.
fn connected(faces: &[usize], flats: &[Flat]) -> Vec<Vec<usize>> {
    let mut root: Vec<usize> = (0..faces.len()).collect();
    fn find(root: &mut [usize], mut k: usize) -> usize {
        while root[k] != k {
            root[k] = root[root[k]];
            k = root[k];
        }
        k
    }
    let mut first_with: HashMap<EntityId, usize> = HashMap::new();
    for (k, &face) in faces.iter().enumerate() {
        for edge in &flats[face].edges {
            match first_with.get(edge) {
                Some(&other) => {
                    let (a, b) = (find(&mut root, k), find(&mut root, other));
                    if a != b {
                        root[a.max(b)] = a.min(b);
                    }
                }
                None => {
                    first_with.insert(*edge, k);
                }
            }
        }
    }
    let mut sets: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, &face) in faces.iter().enumerate() {
        let top = find(&mut root, k);
        sets.entry(top).or_default().push(face);
    }
    sets.into_values().collect()
}

/// Faces joined into one polysurface when there are several; each face on its own when the
/// join fails, so nothing is lost.
fn joined(pieces: Vec<Body>) -> Vec<Body> {
    if pieces.len() < 2 {
        return pieces;
    }
    match Body::assembled(&pieces) {
        Ok(body) => vec![body],
        Err(_) => pieces,
    }
}
