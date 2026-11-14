//! Control points shown on curves and picked for editing. Which curves show them, and which
//! points are picked, is not saved and not part of undo; moving points is.

use crate::Document;
use curva_kernel::{Curve, KernelError, Point};
use std::collections::BTreeSet;

/// One control point: the object it belongs to and its place in
/// [`Curve::control_points`].
pub type Grip = (u64, usize);

impl Document {
    /// Shows the control points of these objects, leaving out those that have none.
    /// Returns how many objects now show them.
    pub fn points_on(&mut self, ids: &[u64]) -> usize {
        let editable: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|o| {
                ids.contains(&o.id) && o.curve().is_some_and(|c| c.control_points().is_ok())
            })
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
                o.curve()
                    .and_then(|c| c.control_points().ok())
                    .unwrap_or_default()
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
