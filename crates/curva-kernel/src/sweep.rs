//! Sweeping curves into surfaces and solids: revolving, along a rail, lofting, and moving a
//! profile along a path.

use crate::extrude::{failed, pieces, pieces_of, sweep_pieces};
use crate::{Body, Curve, KernelError, Point, cross, dot, sub, unit};
use solidcore_ops::sweep::{Affine, Ends, SweepFrame, loft, revolve, sweep_placed, sweep_stations};
use std::f64::consts::TAU;

impl Body {
    /// The curves spun about the line through `origin` along `axis`, by `angle` radians up to
    /// a full turn, right-handed. With `cap`, a closed curve turned part of the way is closed
    /// at both ends into a solid.
    pub fn revolved(
        curves: &[Curve],
        origin: Point,
        axis: Point,
        angle: f64,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let axis =
            crate::unit(axis).ok_or_else(|| KernelError("the axis has no length".to_owned()))?;
        let made = revolve(&pieces_of(curves)?, origin, axis, angle.min(TAU), cap)
            .map_err(|e| failed("revolving", e))?;
        Self::made(&made)
    }

    /// A skin stretched through the curves in order, smooth across them (degree 3 where there
    /// are enough curves). With `cap`, closed curves are closed at the first and last into a
    /// solid.
    pub fn lofted(curves: &[Curve], cap: bool) -> Result<Self, KernelError> {
        let sections = curves
            .iter()
            .map(sweep_pieces)
            .collect::<Result<Vec<_>, _>>()?;
        let degree = u32::try_from(curves.len().saturating_sub(1).clamp(1, 3)).unwrap_or(3);
        let made = loft(&sections, degree, cap).map_err(|e| failed("lofting", e))?;
        Self::made(&made)
    }

    /// The cross-section curves swept along `rail`, turning with it. With `cap`, closed
    /// sections are closed at both ends into a solid.
    pub fn swept(sections: &[Curve], rail: &Curve, cap: bool) -> Result<Self, KernelError> {
        let (made, ..) = sweep_placed(
            &pieces_of(sections)?,
            &sweep_pieces(rail)?,
            SweepFrame::Follow,
            cap,
        )
        .map_err(|e| failed("sweeping", e))?;
        Self::made(&made)
    }

    /// The curves moved along `path` without turning, as Rhino's ExtrudeCrvAlongCrv; closed
    /// flat curves closed at both ends when `cap`.
    pub fn swept_translated(
        curves: &[Curve],
        path: &Curve,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let (made, ..) = sweep_placed(
            &pieces(curves)?,
            &sweep_pieces(path)?,
            SweepFrame::Translate,
            cap,
        )
        .map_err(|e| failed("extruding along a curve", e))?;
        Self::made(&made)
    }

    /// `profile` turned a full turn about the axis through `origin` along `axis`, stretched away
    /// from the axis at each angle so that its point `touch` follows `rail`, as Rhino's
    /// RailRevolve. The surface passes exactly through the profile placed at `stations` angles,
    /// skinned between them.
    pub fn rail_revolved(
        profile: &Curve,
        rail: &Curve,
        origin: Point,
        axis: Point,
        touch: Point,
        stations: usize,
    ) -> Result<Self, KernelError> {
        // CUSTOM KERNEL OPERATION: rail-revolve
        let axis = unit(axis).ok_or_else(|| KernelError("the axis has no length".to_owned()))?;
        let radial = |p: Point| {
            let d = sub(p, origin);
            sub(d, axis.map(|v| v * dot(d, axis)))
        };
        let from = radial(touch);
        let start_radius = dot(from, from).sqrt();
        let x = unit(from).ok_or_else(|| KernelError("the profile touches the axis".to_owned()))?;
        let y = cross(axis, x);
        let rail_points = rail.sample()?;
        let count = stations.clamp(4, 720);
        let mut maps = Vec::with_capacity(count);
        for i in 0..count {
            let angle = std::f64::consts::TAU * i as f64 / count as f64;
            let (c, s) = (angle.cos(), angle.sin());
            let towards: Point = std::array::from_fn(|k| x[k] * c + y[k] * s);
            // How far the rail is from the axis at this angle: from the rail point most nearly
            // in that direction.
            let off = |r: &Point| {
                let along = dot(*r, towards);
                let side = sub(*r, towards.map(|v| v * along));
                dot(side, side).sqrt() / along.max(1e-12)
            };
            let radius = rail_points
                .iter()
                .map(|p| radial(*p))
                .filter(|r| dot(*r, towards) > 0.0)
                .min_by(|a, b| off(a).total_cmp(&off(b)))
                .map(|r| dot(r, towards))
                .ok_or_else(|| KernelError("the rail does not go round the axis".to_owned()))?;
            let stretch = radius / start_radius;
            // Stretched away from the axis (along x), then turned about it by `angle`; the axis
            // itself stays where it is.
            let image = |v: Point| -> Point {
                let (vx, vy, vz) = (dot(v, x) * stretch, dot(v, y), dot(v, axis));
                let (tx, ty) = (vx * c - vy * s, vx * s + vy * c);
                std::array::from_fn(|k| x[k] * tx + y[k] * ty + axis[k] * vz)
            };
            maps.push(Affine {
                m: [
                    image([1.0, 0.0, 0.0]),
                    image([0.0, 1.0, 0.0]),
                    image([0.0, 0.0, 1.0]),
                ],
                t: sub(origin, image(origin)),
            });
        }
        // Round to the start again, then the seam sewn.
        maps.push(maps[0]);
        let made = sweep_stations(&pieces(std::slice::from_ref(profile))?, &maps, Ends::Open)
            .map_err(|e| failed("revolving along a rail", e))?;
        Body::assembled(&[Self::made(&made)?])
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};
    use std::f64::consts::{PI, TAU};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn square(side: f64) -> Curve {
        Curve::polyline(
            vec![
                [0.0; 3],
                [side, 0.0, 0.0],
                [side, side, 0.0],
                [0.0, side, 0.0],
            ],
            true,
        )
        .unwrap()
    }

    #[test]
    fn a_rectangle_revolved_whole_is_a_ring() {
        // A 1 by 2 rectangle from 3 to 4 out from the Z axis: a ring of volume pi (4^2 - 3^2) 2.
        let profile = Curve::polyline(
            vec![
                [3.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [4.0, 0.0, 2.0],
                [3.0, 0.0, 2.0],
            ],
            true,
        )
        .unwrap();
        let ring = Body::revolved(
            std::slice::from_ref(&profile),
            [0.0; 3],
            [0.0, 0.0, 1.0],
            TAU,
            true,
        )
        .unwrap();
        assert!(close(ring.volume().unwrap(), PI * 7.0 * 2.0));
        let quarter =
            Body::revolved(&[profile], [0.0; 3], [0.0, 0.0, 1.0], PI / 2.0, true).unwrap();
        assert!(close(quarter.volume().unwrap(), PI * 7.0 * 2.0 / 4.0));
    }

    #[test]
    fn a_line_revolved_is_a_cylinder_wall() {
        let line = Curve::line([2.0, 0.0, 0.0], [2.0, 0.0, 5.0]).unwrap();
        let wall = Body::revolved(&[line], [0.0; 3], [0.0, 0.0, 1.0], TAU, false).unwrap();
        assert!(close(wall.area().unwrap(), TAU * 2.0 * 5.0));
    }

    #[test]
    fn two_equal_circles_loft_into_a_cylinder() {
        let low = Curve::circle([0.0; 3], 2.0).unwrap();
        let high = Curve::circle([0.0, 0.0, 3.0], 2.0).unwrap();
        let tube = Body::lofted(&[low.clone(), high.clone()], true).unwrap();
        assert!(close(tube.volume().unwrap(), PI * 4.0 * 3.0));
        let open = Body::lofted(&[low, high], false).unwrap();
        assert!(close(open.area().unwrap(), TAU * 2.0 * 3.0));
    }

    #[test]
    fn three_sections_loft_smoothly_through_all_of_them() {
        let sections: Vec<Curve> = [(0.0, 2.0), (2.0, 1.0), (4.0, 2.0)]
            .iter()
            .map(|(z, r)| Curve::circle([0.0, 0.0, *z], *r).unwrap())
            .collect();
        let body = Body::lofted(&sections, true).unwrap();
        let (low, high) = body.bounds().unwrap();
        assert!(close(low[2], 0.0) && close(high[2], 4.0));
        assert!(body.volume().unwrap() > 0.0);
    }

    #[test]
    fn a_circle_swept_along_a_line_is_a_cylinder() {
        let section = Curve::circle([0.0; 3], 1.0).unwrap();
        let rail = Curve::line([0.0; 3], [0.0, 0.0, 6.0]).unwrap();
        let body = Body::swept(&[section], &rail, true).unwrap();
        assert!((body.volume().unwrap() - PI * 6.0).abs() < 1e-3);
    }

    #[test]
    fn a_square_moved_along_a_slanted_line_keeps_its_volume() {
        let path = Curve::line([0.0; 3], [3.0, 0.0, 4.0]).unwrap();
        let solid = Body::swept_translated(&[square(2.0)], &path, true).unwrap();
        // Base 4, height 4: a slanted prism.
        assert!(
            (solid.volume().unwrap() - 16.0).abs() < 1e-3,
            "{}",
            solid.volume().unwrap()
        );
    }

    #[test]
    fn a_rail_revolve_on_a_circle_rail_is_a_revolve() {
        // A profile from the axis out to radius 3 and up, following a circle of radius 3: the
        // same as revolving it, a cylinder of radius 3 and height 2.
        let profile = Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [3.0, 0.0, 2.0],
                [0.0, 0.0, 2.0],
            ],
            false,
        )
        .unwrap();
        let rail = Curve::circle([0.0; 3], 3.0).unwrap();
        let body = Body::rail_revolved(
            &profile,
            &rail,
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [3.0, 0.0, 0.0],
            72,
        )
        .unwrap();
        let area = body.area().unwrap();
        let expected = 2.0 * std::f64::consts::PI * 9.0 + 2.0 * std::f64::consts::PI * 3.0 * 2.0;
        assert!(
            (area - expected).abs() < 0.01 * expected,
            "{area} against {expected}"
        );
    }
}
