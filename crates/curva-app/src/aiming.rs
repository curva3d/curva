//! The planes and corners the pointer aims on: Planar's level plane and Shift's square corner.

use crate::*;

/// `plane` moved along its normal to pass through `base`, keeping its axes and grid.
/// `point` moved straight onto `plane`, as Rhino's Project moves snapped points.
pub(crate) fn onto(plane: &CPlane, point: Point) -> Point {
    let height: f64 = (0..3)
        .map(|i| (point[i] - plane.origin[i]) * plane.normal[i])
        .sum();
    std::array::from_fn(|i| point[i] - plane.normal[i] * height)
}

pub(crate) fn level_with(plane: &CPlane, base: Point) -> CPlane {
    let height: f64 = (0..3)
        .map(|i| (base[i] - plane.origin[i]) * plane.normal[i])
        .sum();
    CPlane {
        origin: std::array::from_fn(|i| plane.origin[i] + plane.normal[i] * height),
        ..*plane
    }
}

/// The opposite corner from `base` moved to make a square on `plane`: both sides as long as the
/// longer one, each keeping its direction (#524).
pub(crate) fn squared(plane: &CPlane, base: Point, at: Point) -> Point {
    let off: Point = std::array::from_fn(|i| at[i] - base[i]);
    let along = |axis: Point| (0..3).map(|i| off[i] * axis[i]).sum::<f64>();
    let (dx, dy) = (along(plane.x), along(plane.y));
    let side = dx.abs().max(dy.abs());
    let (sx, sy) = (side.copysign(dx), side.copysign(dy));
    std::array::from_fn(|i| base[i] + plane.x[i] * sx + plane.y[i] * sy)
}

/// The snaps that may catch the pointer for `request`, the points just clicked that it may snap
/// to, and whether the held points' guidelines may: all of them for a point, but for a height or
/// distance along a line (#484) only the point snaps on other objects. Near, the points just
/// clicked (the base's own corners, at height zero) and the guidelines (all on the plane) used to
/// catch the pointer first, so the height jumped instead of following the mouse.
pub(crate) fn snaps_for(
    request: &curva_commands::Request,
    snaps: SnapSet,
) -> (SnapSet, &[Point], bool) {
    if request.along.is_some() {
        let set = SnapSet {
            near: false,
            perpendicular: false,
            tangent: false,
            ..snaps
        };
        (set, &[], false)
    } else {
        (snaps, &request.preview, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_on_an_opposite_corner_makes_a_square() {
        let plane = CPlane {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        assert_eq!(
            squared(&plane, [1.0, 1.0, 0.0], [5.0, 3.0, 0.0]),
            [5.0, 5.0, 0.0]
        );
        assert_eq!(
            squared(&plane, [0.0; 3], [-2.0, 6.0, 0.0]),
            [-6.0, 6.0, 0.0]
        );
    }

    #[test]
    fn project_moves_a_snapped_point_straight_onto_the_plane() {
        let plane = CPlane {
            origin: [0.0, 0.0, 2.0],
            ..CPlane::WORLD_XY
        };
        assert_eq!(onto(&plane, [3.0, -4.0, 9.5]), [3.0, -4.0, 2.0]);
    }

    #[test]
    fn planar_keeps_the_plane_level_with_the_last_point() {
        let level = level_with(&CPlane::WORLD_XY, [3.0, 4.0, 7.5]);
        assert_eq!(level.origin, [0.0, 0.0, 7.5]);
        assert_eq!(level.normal, CPlane::WORLD_XY.normal);
    }
}
