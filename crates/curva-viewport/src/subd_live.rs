//! Smooth SubDs redrawn live while their cage points are dragged (#334, item 4; spec
//! `mesh-and-subd.md` §4, Stage C). Each SubD keeps the mesh stream's `LiveSurface`: its layout
//! and weights are found once for the cage's shape, and every new set of cage points only
//! re-applies the weights, for the surface and for the cage edges on it (`edge_runs`), with no
//! subdivision. A new shape (another editing command), new creases or a new level make a new one.

use std::hash::{Hash, Hasher};

use curva_kernel::{Curve, Point};
use curva_mesh::{Attribute, LiveSurface, Mesh, SubD, SubDDisplay};

use crate::subds::Drawing;

/// A SubD's live surface, with the shape it was made for.
pub(crate) struct Held {
    shape: u64,
    live: LiveSurface,
}
impl std::fmt::Debug for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Held")
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}

/// What the live surface's layout depends on: the cage's faces, its creases and the level.
fn shape_of(subd: &SubD, levels: u8) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for face in subd.cage().faces() {
        face.hash(&mut hasher);
    }
    subd.creases().collect::<Vec<_>>().hash(&mut hasher);
    levels.hash(&mut hasher);
    hasher.finish()
}

/// What a smooth SubD draws as, from its live surface in `held` (made or remade as its shape
/// needs): as [`crate::subds::drawing`] draws it, but with no subdivision while only its cage
/// points move. `None` for a SubD shown flat or with face colours, which draw as before, and
/// when the library refuses the cage.
pub(crate) fn live_drawing(subd: &SubD, held: &mut Option<Held>) -> Option<Drawing> {
    if subd.display() != SubDDisplay::Smooth || subd.face_colours().is_some() {
        return None;
    }
    let levels = crate::subds::levels_for(subd);
    let shape = shape_of(subd, levels);
    if held.as_ref().is_none_or(|held| held.shape != shape) {
        *held = Some(Held {
            shape,
            live: LiveSurface::new(subd, levels).ok()?,
        });
    }
    let live = &held.as_ref()?.live;
    let (positions, normals) = live.positions(subd.cage().vertices());
    let faces: Vec<&[u32]> = live.mesh().faces().collect();
    let shown = Mesh::new(positions.clone(), &faces)
        .ok()?
        .with_normals(Attribute::PerVertex(normals))
        .ok()?;
    // Creases and open borders are drawn in every view; the other cage edges as wires.
    let marked: std::collections::HashSet<(u32, u32)> = subd
        .creases()
        .chain(subd.cage().naked_edges())
        .map(|[a, b]| (a.min(b), a.max(b)))
        .collect();
    let (mut lines, mut wires) = (Vec::new(), Vec::new());
    for (edge, run) in live.edge_runs() {
        let points: Vec<Point> = run
            .iter()
            .filter_map(|&i| positions.get(i as usize).copied())
            .collect();
        let Ok(curve) = Curve::polyline(points, false) else {
            continue;
        };
        if marked.contains(edge) {
            lines.push(curve);
        } else {
            wires.push(curve);
        }
    }
    Some(Drawing {
        shown,
        lines,
        wires,
        flat: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Drawn live, a smooth SubD looks as it does drawn afresh: the same surface points and
    /// one wire for each of its twelve cage edges. After a cage point moves, the same live
    /// surface is kept and the drawing still matches a fresh one, so dragging needs no
    /// subdivision (#334).
    #[test]
    fn a_dragged_subd_is_drawn_live_as_it_would_be_afresh() {
        let mut held = None;
        let subd = cube();
        let live = live_drawing(&subd, &mut held).expect("drawn live");
        let fresh = crate::subds::drawing(&subd, [0.5; 3]);
        assert_eq!(live.shown.vertex_count(), fresh.shown.vertex_count());
        assert_eq!((live.wires.len(), live.lines.len()), (12, 0));
        let shape = held.as_ref().map(|held| held.shape);
        // Pull the top front corner up and out.
        let mut points = subd.cage().vertices().to_vec();
        points[6] = [13.0, 13.0, 14.0];
        let moved = subd.with_cage_points_moved(&[(6, points[6])]);
        let live = live_drawing(&moved, &mut held).expect("drawn live");
        assert_eq!(
            held.as_ref().map(|held| held.shape),
            shape,
            "kept, not made again"
        );
        let fresh = crate::subds::drawing(&moved, [0.5; 3]);
        let far = |mesh: &Mesh| {
            mesh.vertices()
                .iter()
                .map(|p| p[0] + p[1] + p[2])
                .fold(f64::MIN, f64::max)
        };
        assert!((far(&live.shown) - far(&fresh.shown)).abs() < 1e-9);
    }
}
