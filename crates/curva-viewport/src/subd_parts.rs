//! Where a SubD's picked parts lie as it is drawn (#334), so they are highlighted there, as
//! Rhino 8 highlights them: on the smooth surface when the SubD is shown smooth (a point at its
//! limit point, an edge along its run on the surface, a face as its patch of the surface), on the
//! cage when it is shown flat.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use curva_document::MeshPart;
use curva_kernel::Point;
use curva_mesh::{Mesh, SubD, SubDDisplay};

/// Where one picked part lies.
#[derive(Clone, Debug, PartialEq)]
pub enum PartShape {
    /// A point.
    Point(Point),
    /// An edge, as a run of points.
    Line(Vec<Point>),
    /// A face, as triangles.
    Patch(Vec<[Point; 3]>),
}

/// Most SubDs whose parts are held: highlights repaint every frame, and working the surface out
/// again each time would take a tenth of a second for a large SubD.
const HELD: usize = 32;

/// Parts already found, by a fingerprint of the SubD and the parts asked for.
static FOUND: Mutex<Option<HashMap<u64, Vec<Option<PartShape>>>>> = Mutex::new(None);

/// Where each of `parts` lies on `subd` as it is drawn, `None` for a part it lacks. Kept while
/// the SubD and the parts stay the same, so a still highlight costs only the fingerprint.
#[must_use]
pub fn subd_parts(subd: &SubD, parts: &[MeshPart]) -> Vec<Option<PartShape>> {
    let key = fingerprint(subd, parts);
    if let Some(found) = FOUND
        .lock()
        .ok()
        .and_then(|held| held.as_ref()?.get(&key).cloned())
    {
        return found;
    }
    let found = if subd.display() == SubDDisplay::Smooth {
        on_surface(subd, parts).unwrap_or_else(|| on_cage(subd.cage(), parts))
    } else {
        on_cage(subd.cage(), parts)
    };
    if let Ok(mut held) = FOUND.lock() {
        let held = held.get_or_insert_with(HashMap::new);
        if held.len() >= HELD {
            held.clear();
        }
        held.insert(key, found.clone());
    }
    found
}

/// The SubD's cage, how it is shown and its creases, and the parts: what the answer depends on.
fn fingerprint(subd: &SubD, parts: &[MeshPart]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let cage = subd.cage();
    for p in cage.vertices() {
        p.map(f64::to_bits).hash(&mut hasher);
    }
    cage.corners().hash(&mut hasher);
    (subd.display() == SubDDisplay::Smooth).hash(&mut hasher);
    subd.creases().collect::<Vec<_>>().hash(&mut hasher);
    parts.hash(&mut hasher);
    hasher.finish()
}

/// The parts on the smooth surface, worked out as finely as the SubD is drawn; `None` when the
/// library refuses the cage.
fn on_surface(subd: &SubD, parts: &[MeshPart]) -> Option<Vec<Option<PartShape>>> {
    let levels = crate::subds::levels_for(subd);
    let wants = |test: fn(&MeshPart) -> bool| parts.iter().any(test);
    let limits = wants(|p| matches!(p, MeshPart::Point(_)))
        .then(|| subd.limit_points(levels).ok())
        .flatten();
    let edges = wants(|p| matches!(p, MeshPart::Edge(..)))
        .then(|| subd.edges_on_surface(levels).ok())
        .flatten();
    let faces = wants(|p| matches!(p, MeshPart::Face(_)))
        .then(|| subd.surface_with_faces(levels).ok())
        .flatten();
    Some(
        parts
            .iter()
            .map(|part| match *part {
                MeshPart::Point(i) => Some(PartShape::Point(*limits.as_ref()?.get(i as usize)?)),
                MeshPart::Edge(a, b) => edges
                    .as_ref()?
                    .get(&(a.min(b), a.max(b)))
                    .map(|run| PartShape::Line(run.clone())),
                MeshPart::Face(f) => {
                    let (surface, roots) = faces.as_ref()?;
                    let patch = triangles(surface, |q| roots.get(q) == Some(&f));
                    (!patch.is_empty()).then_some(PartShape::Patch(patch))
                }
            })
            .collect(),
    )
}

/// The parts on the flat cage itself.
fn on_cage(cage: &Mesh, parts: &[MeshPart]) -> Vec<Option<PartShape>> {
    let point = |i: u32| cage.vertices().get(i as usize).copied();
    parts
        .iter()
        .map(|part| match *part {
            MeshPart::Point(i) => Some(PartShape::Point(point(i)?)),
            MeshPart::Edge(a, b) => Some(PartShape::Line(vec![point(a)?, point(b)?])),
            MeshPart::Face(f) => {
                let f = f as usize;
                (f < cage.face_count()).then(|| PartShape::Patch(triangles(cage, |q| q == f)))
            }
        })
        .collect()
}

/// The faces of `mesh` that `keep` picks, split into triangles fanning from each face's first
/// corner.
fn triangles(mesh: &Mesh, keep: impl Fn(usize) -> bool) -> Vec<[Point; 3]> {
    let points = mesh.vertices();
    (0..mesh.face_count())
        .filter(|&q| keep(q))
        .flat_map(|q| {
            let face = mesh.face(q);
            (1..face.len().saturating_sub(1))
                .map(move |k| [face[0], face[k], face[k + 1]].map(|v| points[v as usize]))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cube cage, shown smooth unless said otherwise.
    fn cube() -> SubD {
        let points = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ];
        let faces = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ];
        SubD::new(Mesh::new(points, &faces).unwrap())
    }

    /// Shown smooth, a cube's picked corner, edge and face lie on its smooth surface, inside the
    /// cage, as Rhino highlights them; shown flat, on the cage itself (#334).
    #[test]
    fn a_subds_picked_parts_lie_where_it_is_drawn() {
        let parts = [MeshPart::Point(0), MeshPart::edge(0, 1), MeshPart::Face(1)];
        let smooth = subd_parts(&cube(), &parts);
        let inside = |p: &Point| p.iter().all(|&c| c > 0.5 && c < 9.5);
        let Some(PartShape::Point(corner)) = &smooth[0] else {
            panic!("a point: {:?}", smooth[0]);
        };
        assert!(inside(corner), "{corner:?}");
        let Some(PartShape::Line(run)) = &smooth[1] else {
            panic!("a line: {:?}", smooth[1]);
        };
        assert!(run.len() > 2 && run.iter().all(inside));
        let Some(PartShape::Patch(patch)) = &smooth[2] else {
            panic!("a patch: {:?}", smooth[2]);
        };
        assert!(!patch.is_empty() && patch.iter().flatten().all(|p| p[2] > 5.0));
        let flat = subd_parts(&cube().with_display(SubDDisplay::Flat), &parts);
        assert_eq!(flat[0], Some(PartShape::Point([0.0; 3])));
        assert_eq!(
            flat[1],
            Some(PartShape::Line(vec![[0.0; 3], [10.0, 0.0, 0.0]]))
        );
        assert!(matches!(&flat[2], Some(PartShape::Patch(t)) if t.len() == 2));
        // A part the cage lacks is nothing.
        assert_eq!(subd_parts(&cube(), &[MeshPart::Point(99)]), vec![None]);
    }
}
