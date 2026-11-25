//! Control points shown on curves, and the points of meshes and SubD cages, picked for editing. Which curves show them, and which
//! points are picked, is not saved and not part of undo; moving points is.

use crate::Document;
use curva_kernel::{Curve, KernelError, Point};
use std::collections::BTreeSet;

/// One control point: the object it belongs to and its place in
/// [`Curve::control_points`].
pub type Grip = (u64, usize);

/// The points an object shows with PointsOn: a curve's control points, a mesh's points, a SubD's
/// cage points (#563); none for other objects.
fn points_of(shape: &crate::Shape) -> Vec<Point> {
    match shape {
        crate::Shape::Curve(curve) => curve.control_points().unwrap_or_default(),
        crate::Shape::Mesh(mesh) => mesh.vertices().to_vec(),
        crate::Shape::SubD(subd) => subd.cage().vertices().to_vec(),
        _ => Vec::new(),
    }
}

impl Document {
    /// Shows the control points of these objects, leaving out those that have none.
    /// Returns how many objects now show them.
    pub fn points_on(&mut self, ids: &[u64]) -> usize {
        let editable: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id) && !points_of(&o.shape).is_empty())
            .map(|o| o.id)
            .collect();
        self.showing_points.extend(editable.iter().copied());
        editable.len()
    }

    /// Hides every control point and forgets which were picked.
    pub fn points_off(&mut self) {
        self.showing_points.clear();
        self.grip_selection.clear();
    }

    /// The objects showing their control points that still exist, in id order.
    #[must_use]
    pub fn showing_points(&self) -> Vec<u64> {
        self.existing(&self.showing_points.iter().copied().collect::<Vec<_>>())
    }

    /// Every control point on show, with where it is.
    #[must_use]
    pub fn shown_grips(&self) -> Vec<(Grip, Point)> {
        self.state
            .objects
            .iter()
            .filter(|o| !o.hidden && self.showing_points.contains(&o.id))
            .flat_map(|o| {
                points_of(&o.shape)
                    .into_iter()
                    .enumerate()
                    .map(move |(index, at)| ((o.id, index), at))
            })
            .collect()
    }

    /// The picked control points, leaving out any that no longer exist.
    #[must_use]
    pub fn grip_selection(&self) -> Vec<Grip> {
        let shown: BTreeSet<Grip> = self.shown_grips().into_iter().map(|(g, _)| g).collect();
        self.grip_selection
            .iter()
            .copied()
            .filter(|grip| shown.contains(grip))
            .collect()
    }

    /// Replaces the picked control points, ignoring any not on show.
    pub fn select_grips(&mut self, grips: &[Grip]) {
        let shown: BTreeSet<Grip> = self.shown_grips().into_iter().map(|(g, _)| g).collect();
        self.grip_selection = grips
            .iter()
            .copied()
            .filter(|g| shown.contains(g))
            .collect();
    }

    /// The curves as they would be with each picked control point passed through `change`.
    pub fn grips_changed(
        &self,
        grips: &[Grip],
        change: impl Fn(Point) -> Point,
    ) -> Result<Vec<(u64, Curve)>, KernelError> {
        let owners: BTreeSet<u64> = grips.iter().map(|(id, _)| *id).collect();
        self.state
            .objects
            .iter()
            .filter(|o| owners.contains(&o.id))
            .filter_map(|o| o.curve().map(|curve| (o.id, curve)))
            .map(|(id, curve)| {
                let mut points = curve.control_points()?;
                for (index, point) in points.iter_mut().enumerate() {
                    if grips.contains(&(id, index)) {
                        *point = change(*point);
                    }
                }
                Ok((id, curve.with_control_points(points)?))
            })
            .collect()
    }

    /// Every object with picked points as it would be with each passed through `change`: curves by
    /// their control points, meshes by their points and SubDs by their cage points (#563).
    pub fn grips_reshaped(
        &self,
        grips: &[Grip],
        change: impl Fn(Point) -> Point,
    ) -> Result<Vec<(u64, crate::Shape)>, KernelError> {
        let mut made: Vec<(u64, crate::Shape)> = self
            .grips_changed(grips, &change)?
            .into_iter()
            .map(|(id, curve)| (id, crate::Shape::Curve(curve)))
            .collect();
        for (id, indices) in Self::by_owner(grips) {
            let Some(object) = self.state.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            let moved = |points: &[Point]| -> Vec<(u32, Point)> {
                indices
                    .iter()
                    .filter_map(|&i| Some((u32::try_from(i).ok()?, change(*points.get(i)?))))
                    .collect()
            };
            match &object.shape {
                crate::Shape::Mesh(mesh) => {
                    // A mesh's grips move its points by place, as Rhino's do, so a mesh unwelded at
                    // its hard edges moves together rather than tearing open.
                    let all = mesh.vertices();
                    let picked = indices.iter().filter_map(|i| u32::try_from(*i).ok());
                    let moves: Vec<(u32, Point)> =
                        crate::part_moves::with_same_places(all, picked.collect())
                            .into_iter()
                            .filter_map(|i| Some((i, change(*all.get(i as usize)?))))
                            .collect();
                    made.push((id, crate::Shape::Mesh(mesh.with_points_moved(&moves))));
                }
                crate::Shape::SubD(subd) => made.push((
                    id,
                    crate::Shape::SubD(subd.with_cage_points_moved(&moved(subd.cage().vertices()))),
                )),
                _ => {}
            }
        }
        Ok(made)
    }

    /// The control points of each object among `grips`, by object.
    fn by_owner(grips: &[Grip]) -> Vec<(u64, Vec<usize>)> {
        let mut owners: Vec<(u64, Vec<usize>)> = Vec::new();
        for &(id, index) in grips {
            match owners.iter_mut().find(|(owner, _)| *owner == id) {
                Some((_, indices)) => indices.push(index),
                None => owners.push((id, vec![index])),
            }
        }
        owners
    }

    /// The curves as they would be with each picked control point evened out by `factor`.
    pub fn grips_smoothed(
        &self,
        grips: &[Grip],
        factor: f64,
    ) -> Result<Vec<(u64, Curve)>, KernelError> {
        Self::by_owner(grips)
            .into_iter()
            .filter_map(|(id, indices)| self.curve(id).map(|curve| (id, curve, indices)))
            .map(|(id, curve, indices)| Ok((id, curve.smoothed(&indices, factor)?)))
            .collect()
    }

    /// The curves as they would be without the picked control points.
    pub fn grips_removed(&self, grips: &[Grip]) -> Result<Vec<(u64, Curve)>, KernelError> {
        Self::by_owner(grips)
            .into_iter()
            .filter_map(|(id, indices)| self.curve(id).map(|curve| (id, curve, indices)))
            .map(|(id, mut curve, mut indices)| {
                // From the last, so that the places of those still to go do not shift.
                indices.sort_unstable();
                indices.dedup();
                for index in indices.into_iter().rev() {
                    curve = curve.without_control_point(index)?;
                }
                Ok((id, curve))
            })
            .collect()
    }

    fn curve(&self, id: u64) -> Option<Curve> {
        self.state
            .objects
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| o.curve().cloned())
    }

    /// The editable curve passing within `reach` of `at`, preferring one that shows its
    /// control points, for commands that pick a place on a curve.
    #[must_use]
    pub fn editable_curve_at(&self, at: Point, reach: f64) -> Option<(u64, Curve)> {
        let near: Vec<&crate::Object> = self
            .state
            .objects
            .iter()
            .filter(|o| !o.hidden && o.curve().is_some_and(|c| c.control_points().is_ok()))
            .filter(|o| {
                o.curve().is_some_and(|c| {
                    c.closest_point(at)
                        .is_ok_and(|p| curva_kernel::gap(p, at) <= reach)
                })
            })
            .map(|o| &**o)
            .collect();
        near.iter()
            .find(|o| self.showing_points.contains(&o.id))
            .or_else(|| near.first())
            .and_then(|o| Some((o.id, o.curve()?.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_cage_and_mesh_points_move_and_nothing_else() {
        let mut doc = Document::default();
        let frame = curva_mesh::Frame::world();
        let a = doc.add_shape(crate::Shape::SubD(curva_mesh::SubD::subd_box(
            &frame,
            [2.0; 3],
            [1, 1, 1],
        )));
        let b = doc.add_shape(crate::Shape::Mesh(curva_mesh::Mesh::mesh_box(
            &frame,
            [2.0; 3],
            [1, 1, 1],
        )));
        doc.points_on(&[a, b]);
        let made = doc
            .grips_reshaped(&[(a, 0), (b, 1)], |p| [p[0], p[1], p[2] + 5.0])
            .unwrap();
        assert_eq!(made.len(), 2);
        for (id, shape) in made {
            let (points, moved) = match &shape {
                crate::Shape::SubD(subd) => (subd.cage().vertices().to_vec(), 0),
                crate::Shape::Mesh(mesh) => (mesh.vertices().to_vec(), 1),
                _ => panic!("unexpected shape"),
            };
            let before = match &doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .unwrap()
                .shape
            {
                crate::Shape::SubD(subd) => subd.cage().vertices().to_vec(),
                crate::Shape::Mesh(mesh) => mesh.vertices().to_vec(),
                _ => unreachable!(),
            };
            for (k, (p, q)) in points.iter().zip(&before).enumerate() {
                let lift = if k == moved { 5.0 } else { 0.0 };
                assert!((p[2] - q[2] - lift).abs() < 1e-12, "point {k} of {id}");
            }
        }
    }

    #[test]
    fn a_subd_shows_its_cage_points_and_a_mesh_its_points() {
        let mut doc = Document::default();
        let frame = curva_mesh::Frame::world();
        let subd = curva_mesh::SubD::subd_box(&frame, [2.0; 3], [1, 1, 1]);
        let cage = subd.cage().vertices().len();
        let a = doc.add_shape(crate::Shape::SubD(subd));
        let mesh = curva_mesh::Mesh::mesh_box(&frame, [2.0; 3], [1, 1, 1]);
        let points = mesh.vertices().len();
        let b = doc.add_shape(crate::Shape::Mesh(mesh));
        assert_eq!(doc.points_on(&[a, b]), 2);
        let shown = doc.shown_grips();
        assert_eq!(shown.iter().filter(|((id, _), _)| *id == a).count(), cage);
        assert_eq!(shown.iter().filter(|((id, _), _)| *id == b).count(), points);
        doc.select_grips(&[(a, 0)]);
        assert_eq!(doc.grip_selection(), vec![(a, 0)]);
    }

    fn square() -> Curve {
        Curve::polyline(
            vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
            true,
        )
        .unwrap()
    }

    #[test]
    fn points_show_only_on_curves_that_have_them() {
        let mut doc = Document::default();
        let a = doc.add(square());
        let dot = doc.add(Curve::Point { at: [1.0; 3] });
        assert_eq!(doc.points_on(&[a, dot]), 1);
        assert_eq!(doc.shown_grips().len(), 4);
        doc.select_grips(&[(a, 2), (dot, 0), (a, 9)]);
        assert_eq!(doc.grip_selection(), [(a, 2)]);
        doc.points_off();
        assert!(doc.shown_grips().is_empty() && doc.grip_selection().is_empty());
    }

    #[test]
    fn moving_picked_points_changes_only_them() {
        let mut doc = Document::default();
        let a = doc.add(square());
        doc.points_on(&[a]);
        let changed = doc
            .grips_changed(&[(a, 2)], |p| [p[0] + 1.0, p[1], p[2]])
            .unwrap();
        let points = changed[0].1.control_points().unwrap();
        assert_eq!(points[2], [5.0, 4.0, 0.0]);
        assert_eq!(points[1], [4.0, 0.0, 0.0]);
    }
}
