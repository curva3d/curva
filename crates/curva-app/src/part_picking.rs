//! Picking parts of meshes and SubDs with the mouse (#333, and Rhino's Ctrl+Shift sub-object
//! picking, #485): a click takes the nearest point within reach, else the nearest edge, else the
//! face under the pointer, nearest the viewer; a window takes the parts inside it, or touching it
//! for a crossing window. A SubD is picked by its cage.

use crate::egui;
use curva_document::{MeshPart, MeshPick};
use curva_kernel::Point;

/// A mesh to pick on: its object, its points and its faces.
pub(crate) struct Pickable<'a> {
    pub(crate) id: u64,
    pub(crate) points: &'a [Point],
    pub(crate) faces: Vec<&'a [u32]>,
}

/// How far a point is on screen from `at`.
fn gap(p: [f64; 2], at: [f64; 2]) -> f64 {
    (p[0] - at[0]).hypot(p[1] - at[1])
}

/// How far `at` is on screen from the segment `a`–`b`.
fn to_segment(at: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((at[0] - a[0]) * dx + (at[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    gap([a[0] + dx * t, a[1] + dy * t], at)
}

/// Whether `at` is inside the screen polygon `corners`.
fn inside(at: [f64; 2], corners: &[[f64; 2]]) -> bool {
    let mut within = false;
    for k in 0..corners.len() {
        let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
        if (a[1] > at[1]) != (b[1] > at[1])
            && at[0] < a[0] + (at[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            within = !within;
        }
    }
    within
}

/// The part a click at `at` takes, and how far it is: points first, then edges, within `reach`
/// on screen; otherwise the face under the pointer nearest the viewer (`depth` grows away).
pub(crate) fn part_at(
    meshes: &[Pickable<'_>],
    project: &dyn Fn(Point) -> Option<[f64; 2]>,
    depth: &dyn Fn(Point) -> f64,
    at: [f64; 2],
    reach: f64,
) -> Option<MeshPick> {
    let mut point: Option<(f64, MeshPick)> = None;
    let mut edge: Option<(f64, MeshPick)> = None;
    let mut face: Option<(f64, MeshPick)> = None;
    let better = |held: &Option<(f64, MeshPick)>, d: f64| held.is_none_or(|(h, _)| d < h);
    for mesh in meshes {
        let screen: Vec<Option<[f64; 2]>> = mesh.points.iter().map(|p| project(*p)).collect();
        for (k, s) in screen.iter().enumerate() {
            if let Some(s) = s {
                let d = gap(*s, at);
                if d <= reach && better(&point, d) {
                    point = Some((d, (mesh.id, MeshPart::Point(k as u32))));
                }
            }
        }
        for (f, corners) in mesh.faces.iter().enumerate() {
            let on_screen: Option<Vec<[f64; 2]>> = corners
                .iter()
                .map(|c| screen.get(*c as usize).copied().flatten())
                .collect();
            let Some(on_screen) = on_screen else {
                continue;
            };
            for k in 0..corners.len() {
                let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
                let d = to_segment(at, on_screen[k], on_screen[(k + 1) % corners.len()]);
                if d <= reach && better(&edge, d) {
                    edge = Some((d, (mesh.id, MeshPart::edge(a, b))));
                }
            }
            if inside(at, &on_screen) {
                let middle: Point = std::array::from_fn(|i| {
                    corners
                        .iter()
                        .map(|c| mesh.points[*c as usize][i])
                        .sum::<f64>()
                        / corners.len() as f64
                });
                let d = depth(middle);
                if better(&face, d) {
                    face = Some((d, (mesh.id, MeshPart::Face(f as u32))));
                }
            }
        }
    }
    point.or(edge).or(face).map(|(_, pick)| pick)
}

/// The parts inside the screen box from `lo` to `hi`: points inside it; edges and faces with all
/// their points inside, or, for a `crossing` window, any of them.
pub(crate) fn parts_in_box(
    meshes: &[Pickable<'_>],
    project: &dyn Fn(Point) -> Option<[f64; 2]>,
    (lo, hi): ([f64; 2], [f64; 2]),
    crossing: bool,
) -> Vec<MeshPick> {
    let within = |s: [f64; 2]| (lo[0]..=hi[0]).contains(&s[0]) && (lo[1]..=hi[1]).contains(&s[1]);
    let mut picked = Vec::new();
    for mesh in meshes {
        let taken: Vec<bool> = mesh
            .points
            .iter()
            .map(|p| project(*p).is_some_and(within))
            .collect();
        for (k, yes) in taken.iter().enumerate() {
            if *yes {
                picked.push((mesh.id, MeshPart::Point(k as u32)));
            }
        }
        let counts = |list: &[u32]| {
            let hit = list.iter().filter(|c| taken[**c as usize]).count();
            if crossing { hit > 0 } else { hit == list.len() }
        };
        for (f, corners) in mesh.faces.iter().enumerate() {
            for k in 0..corners.len() {
                let pair = [corners[k], corners[(k + 1) % corners.len()]];
                let edge = (mesh.id, MeshPart::edge(pair[0], pair[1]));
                if counts(&pair) && !picked.contains(&edge) {
                    picked.push(edge);
                }
            }
            if counts(corners) {
                picked.push((mesh.id, MeshPart::Face(f as u32)));
            }
        }
    }
    picked
}

/// `list` with `items` added; a single item clicked that is already there is taken out instead,
/// so a second Ctrl+Shift click lets a part go.
fn toggled<T: PartialEq + Clone>(mut list: Vec<T>, items: Vec<T>, click: bool) -> Vec<T> {
    if click && items.len() == 1 && list.contains(&items[0]) {
        list.retain(|held| *held != items[0]);
        return list;
    }
    for item in items {
        if !list.contains(&item) {
            list.push(item);
        }
    }
    list
}

/// What a pick takes: parts of meshes and SubDs, edges of bodies, faces of bodies.
#[derive(Default)]
struct Picked {
    parts: Vec<MeshPick>,
    edges: Vec<curva_document::EdgePick>,
    faces: Vec<curva_document::FacePick>,
    corners: Vec<curva_document::CornerPick>,
}

impl crate::Curva {
    /// Ctrl+Shift with a click or a window, or any pick while a command asks for parts of
    /// meshes: parts of objects, as Rhino picks sub-objects (#485, #333). A click takes the
    /// nearest point or edge, else the face under the pointer; a window from left to right takes
    /// the parts inside it, from right to left those it touches.
    pub(crate) fn pick_parts(
        &mut self,
        p: &crate::mouse::Picking,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        let click = !crate::mouse::is_box_drag(from, to);
        let picked = self.parts_under(p, from, to, click);
        let parts = toggled(self.doc.mesh_part_selection(), picked.parts, click);
        let edges = toggled(self.doc.edge_selection(), picked.edges, click);
        let faces = toggled(self.doc.face_selection(), picked.faces, click);
        let corners = toggled(self.doc.corner_selection(), picked.corners, click);
        self.doc.select_mesh_parts(&parts);
        self.doc.select_edges(&edges);
        self.doc.select_faces(&faces);
        self.doc.select_corners(&corners);
        self.remember_parts_picked();
    }

    /// A plain click at `at` near a point of a SubD whose cage is on show because parts of it
    /// are picked takes that point instead, as the cage's points are there to be picked (owner,
    /// 25 November 2026); Shift adds it, Ctrl takes it away. Returns whether a point was taken.
    pub(crate) fn pick_cage_point(&mut self, p: &crate::mouse::Picking, at: egui::Pos2) -> bool {
        let edited: Vec<u64> = self
            .doc
            .mesh_part_selection()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        let cages: Vec<Pickable<'_>> = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| edited.contains(&o.id) && !o.hidden)
            .filter_map(|o| {
                let cage = o.shape.subd()?.cage();
                Some(Pickable {
                    id: o.id,
                    points: cage.vertices(),
                    faces: Vec::new(),
                })
            })
            .collect();
        if cages.is_empty() {
            return false;
        }
        let (camera, size) = (p.camera, p.size);
        let project = |q: Point| camera.project(q, size);
        let Some(hit) = part_at(&cages, &project, &|_| 0.0, p.local(at), crate::PICK_REACH) else {
            return false;
        };
        let mut parts = if p.shift || p.remove {
            self.doc.mesh_part_selection()
        } else {
            Vec::new()
        };
        parts.retain(|held| *held != hit);
        if !p.remove {
            parts.push(hit);
        }
        self.doc.select_mesh_parts(&parts);
        self.remember_parts_picked();
        true
    }

    /// Every corner of the surfaces and solids on show, with where it is.
    fn body_corners(&self) -> Vec<(curva_document::CornerPick, Point)> {
        let mut corners = Vec::new();
        for object in self.doc.state().objects.iter().filter(|o| !o.hidden) {
            if let Some(points) = object.shape.body().and_then(|b| b.corner_points().ok()) {
                corners.extend(
                    points
                        .into_iter()
                        .enumerate()
                        .map(|(k, at)| ((object.id, k), at)),
                );
            }
        }
        corners
    }

    fn parts_under(
        &self,
        p: &crate::mouse::Picking,
        from: egui::Pos2,
        to: egui::Pos2,
        click: bool,
    ) -> Picked {
        let meshes: Vec<Pickable<'_>> = self
            .doc
            .state()
            .objects
            .iter()
            .filter_map(|o| {
                let mesh = match &o.shape {
                    curva_document::Shape::Mesh(mesh) => mesh,
                    curva_document::Shape::SubD(subd) => subd.cage(),
                    _ => return None,
                };
                Some(Pickable {
                    id: o.id,
                    points: mesh.vertices(),
                    faces: mesh.faces().collect(),
                })
            })
            .collect();
        let (camera, size) = (p.camera, p.size);
        let project = |q: Point| camera.project(q, size);
        let forward = camera.forward();
        let depth = |q: Point| (0..3).map(|i| q[i] * forward[i]).sum::<f64>();
        let (a, b) = (p.local(from), p.local(to));
        let mut picked = Picked::default();
        if !click {
            let lo = [a[0].min(b[0]), a[1].min(b[1])];
            let hi = [a[0].max(b[0]), a[1].max(b[1])];
            picked.parts = parts_in_box(&meshes, &project, (lo, hi), b[0] < a[0]);
            picked.edges = self.outlines.edges_in_box(&camera, a, b, size);
            picked.faces = self.outlines.faces_in_box(&camera, a, b, size);
            picked.corners = self
                .body_corners()
                .into_iter()
                .filter(|(_, at)| {
                    project(*at).is_some_and(|s| {
                        (lo[0]..=hi[0]).contains(&s[0]) && (lo[1]..=hi[1]).contains(&s[1])
                    })
                })
                .map(|(pick, _)| pick)
                .collect();
            return picked;
        }
        // A corner of a surface or solid within reach comes first, as Rhino picks a vertex
        // before the edges meeting at it.
        let corner = self
            .body_corners()
            .into_iter()
            .filter_map(|(pick, at)| Some((pick, gap(project(at)?, a))))
            .filter(|(_, d)| *d <= crate::PICK_REACH)
            .min_by(|x, y| x.1.total_cmp(&y.1));
        let part = part_at(&meshes, &project, &depth, a, crate::PICK_REACH);
        let edge = self.outlines.pick_edge(&camera, a, size, crate::PICK_REACH);
        let mesh_point = part.is_some_and(|(_, p)| matches!(p, MeshPart::Point(_)));
        if let Some((pick, _)) = corner.filter(|_| !mesh_point) {
            picked.corners.push(pick);
            return picked;
        }
        match (part, edge) {
            (Some(part), _) if !matches!(part.1, MeshPart::Face(_)) => picked.parts.push(part),
            (_, Some(edge)) => picked.edges.push(edge),
            (Some(face), None) => picked.parts.push(face),
            (None, None) => picked
                .faces
                .extend(self.outlines.pick_face(&camera, a, size)),
        }
        picked
    }
}

/// Whether any object in `picked_on` (its id and the body its parts were picked on) is gone
/// or holds another body since, so its numbered edges and faces may name others.
pub(crate) fn changed_since(
    picked_on: &[(u64, curva_kernel::Body)],
    objects: &[std::sync::Arc<curva_document::Object>],
) -> bool {
    // The body held here keeps its faces in memory, so another body cannot take their place.
    picked_on.iter().any(|(id, body)| {
        objects
            .iter()
            .find(|o| o.id == *id)
            .and_then(|o| o.shape.body())
            .is_none_or(|now| now.faces_key() != body.faces_key())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit square face seen from above, one model unit to ten pixels.
    fn square() -> (Vec<Point>, Vec<u32>) {
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        (points, vec![0, 1, 2, 3])
    }

    fn project(p: Point) -> Option<[f64; 2]> {
        Some([p[0] * 10.0, p[1] * 10.0])
    }

    fn depth(p: Point) -> f64 {
        -p[2]
    }

    #[test]
    fn a_second_click_lets_a_part_go_and_a_window_adds() {
        assert_eq!(toggled(vec![1, 2], vec![2], true), vec![1]);
        assert_eq!(toggled(vec![1], vec![2], true), vec![1, 2]);
        assert_eq!(toggled(vec![1], vec![1, 3], false), vec![1, 3]);
    }

    #[test]
    fn a_click_takes_a_point_then_an_edge_then_the_face() {
        let (points, face) = square();
        let meshes = [Pickable {
            id: 7,
            points: &points,
            faces: vec![&face],
        }];
        let at = |x: f64, y: f64| part_at(&meshes, &project, &depth, [x, y], 2.0);
        assert_eq!(at(0.5, 0.5), Some((7, MeshPart::Point(0))));
        assert_eq!(at(5.0, 0.5), Some((7, MeshPart::Edge(0, 1))));
        assert_eq!(at(5.0, 5.0), Some((7, MeshPart::Face(0))));
        assert_eq!(at(30.0, 30.0), None);
    }

    #[test]
    fn a_window_takes_what_is_inside_or_crossing() {
        let (points, face) = square();
        let meshes = [Pickable {
            id: 7,
            points: &points,
            faces: vec![&face],
        }];
        // Around the bottom edge only: its two points and the edge; not the face.
        let inside = parts_in_box(&meshes, &project, ([-1.0, -1.0], [11.0, 1.0]), false);
        assert!(
            inside.contains(&(7, MeshPart::Point(0))) && inside.contains(&(7, MeshPart::Point(1)))
        );
        assert!(inside.contains(&(7, MeshPart::Edge(0, 1))));
        assert!(!inside.contains(&(7, MeshPart::Face(0))));
        // A crossing window touching it takes the face and the edges reaching in.
        let crossing = parts_in_box(&meshes, &project, ([-1.0, -1.0], [11.0, 1.0]), true);
        assert!(crossing.contains(&(7, MeshPart::Face(0))));
        assert!(crossing.contains(&(7, MeshPart::Edge(1, 2))));
    }

    #[test]
    fn picked_parts_hold_until_their_object_changes() {
        // Ctrl+Shift picks on a solid stay picked; they go only once the solid itself changes,
        // as its numbered faces may then be others.
        let mut doc = curva_document::Document::default();
        let id = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0; 3]).unwrap());
        let body = |doc: &curva_document::Document| {
            let o = doc.state().objects.iter().find(|o| o.id == id).unwrap();
            o.shape.body().unwrap().clone()
        };
        let picked_on = vec![(id, body(&doc))];
        assert!(!changed_since(&picked_on, &doc.state().objects));
        doc.select(&[id]);
        assert!(
            !changed_since(&picked_on, &doc.state().objects),
            "selecting is no change"
        );
        let moved = curva_kernel::Body::cuboid([2.0; 3], [1.0; 3]).unwrap();
        doc.replace_shape(id, curva_document::Shape::Body(moved));
        assert!(changed_since(&picked_on, &doc.state().objects));
        let held = picked_on[0].1.clone();
        doc.remove(&[id]);
        assert!(changed_since(&[(id, held)], &doc.state().objects), "gone");
    }
}
