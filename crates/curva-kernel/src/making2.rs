//! More ways of making surfaces and solids, through the engine's sweeps: drafted extrusions,
//! sweeps that only move the profile, pyramids and cones to a point, revolving along a rail, and
//! cutting a solid with a surface.

use crate::body::failed;
use crate::making::sweep_pieces;
use crate::{Body, Curve, KernelError, Point, cross, dot, sub, unit};
use solidcore_ops::sweep::{
    Affine, DraftCorner, Ends, SweepFrame, extrude, loft_to_point, sweep_placed, sweep_stations,
};

/// How the corners of a drafted extrusion close, as Rhino's ExtrudeCrvTapered `Corners`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaperCorners {
    /// Each wall continues until it meets the next (Rhino's `Sharp`).
    Sharp,
    /// An arc about each corner (Rhino's `Round`).
    Round,
    /// Each wall continues straight along its end (Rhino's `Smooth`).
    Smooth,
}

fn pieces(curves: &[Curve]) -> Result<Vec<solidcore_ops::sweep::Curve>, KernelError> {
    let mut out = Vec::new();
    for curve in curves {
        out.extend(sweep_pieces(curve)?);
    }
    Ok(out)
}

const SAME: Affine = Affine {
    m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    t: [0.0; 3],
};

impl Body {
    /// A closed flat curve pulled along `direction` by `distance`, its walls leaning in by
    /// `draft` radians (out when negative), closed at both ends when `cap`.
    pub fn extruded_tapered(
        curve: &Curve,
        direction: Point,
        distance: f64,
        draft: f64,
        corners: TaperCorners,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let corner = match corners {
            TaperCorners::Sharp => DraftCorner::Curvature,
            TaperCorners::Round => DraftCorner::Round,
            TaperCorners::Smooth => DraftCorner::Tangent,
        };
        let (direction, distance) = if distance < 0.0 {
            (direction.map(|v| -v), -distance)
        } else {
            (direction, distance)
        };
        let made = extrude(
            &pieces(std::slice::from_ref(curve))?,
            direction,
            distance,
            draft,
            corner,
            cap,
        )
        .map_err(|e| failed("extruding with a draft", e))?;
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

    /// A pyramid on a closed polygon of lines, to `apex`.
    pub fn pyramid(base: &Curve, apex: Point) -> Result<Self, KernelError> {
        let made =
            loft_to_point(&sweep_pieces(base)?, apex).map_err(|e| failed("making a pyramid", e))?;
        Self::made(&made)
    }

    /// Lines from every point of `curve` to `apex`, as Rhino's ExtrudeCrvToPoint: a pyramid for
    /// a closed polygon when `cap`; otherwise a ruled surface whose tip closes to a ten-thousandth
    /// of the curve's size, as the engine's sections cannot shrink to nothing.
    pub fn extruded_to_point(curve: &Curve, apex: Point, cap: bool) -> Result<Self, KernelError> {
        if cap && matches!(curve, Curve::Polyline { closed: true, .. }) {
            return Self::pyramid(curve, apex);
        }
        // CUSTOM KERNEL OPERATION: cone-to-point
        let ends = if cap { Ends::Capped } else { Ends::Open };
        let profile = pieces(std::slice::from_ref(curve))?;
        // The smallest tip the engine closes reliably, then a larger one if it will not.
        let mut last = None;
        for tiny in [1e-4, 1e-3] {
            let toward = Affine {
                m: [[tiny, 0.0, 0.0], [0.0, tiny, 0.0], [0.0, 0.0, tiny]],
                t: apex.map(|v| v * (1.0 - tiny)),
            };
            match sweep_stations(&profile, &[SAME, toward], ends) {
                Ok(made) => return Self::made(&made),
                Err(e) => last = Some(e),
            }
        }
        Err(failed("extruding to a point", last))
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

    /// The solid cut in two by a surface: the parts behind and in front of it, or `None` when
    /// the surface does not divide it.
    pub fn split_by(&self, sheet: &Body) -> Result<Option<(Body, Body)>, KernelError> {
        let body = self.engine_body()?;
        let tool = sheet.engine_body()?;
        let (_, order) = tool
            .to_record()
            .map_err(|e| failed("reading the cutter", e))?;
        let parts = solidcore_ops::split::split_body_by_sheet(&body, &tool, &order, 0)
            .map_err(|e| failed("cutting", e))?;
        parts
            .map(|(a, b)| Ok((Self::made(&a)?, Self::made(&b)?)))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve, TaperCorners};

    fn square(size: f64) -> Curve {
        Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [size, 0.0, 0.0],
                [size, size, 0.0],
                [0.0, size, 0.0],
            ],
            true,
        )
        .unwrap()
    }

    #[test]
    fn a_square_tapered_in_makes_a_frustum() {
        // 10 × 10, 2 high, walls leaning in 45°: the top is 6 × 6; volume h/3 (A + B + √AB).
        let solid = Body::extruded_tapered(
            &square(10.0),
            [0.0, 0.0, 1.0],
            2.0,
            std::f64::consts::FRAC_PI_4,
            TaperCorners::Sharp,
            true,
        )
        .unwrap();
        let expected = 2.0 / 3.0 * (100.0 + 36.0 + 60.0);
        assert!(
            (solid.volume().unwrap() - expected).abs() < 1e-3,
            "{}",
            solid.volume().unwrap()
        );
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
    fn pyramids_and_cones_to_a_point() {
        let pyramid = Body::extruded_to_point(&square(3.0), [1.5, 1.5, 4.0], true).unwrap();
        assert!((pyramid.volume().unwrap() - 12.0).abs() < 1e-6);
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let cone = Body::extruded_to_point(&circle, [0.0, 0.0, 6.0], true).unwrap();
        let expected = std::f64::consts::PI * 4.0 * 6.0 / 3.0;
        assert!((cone.volume().unwrap() - expected).abs() < 1e-3 * expected);
    }

    #[test]
    fn a_box_cut_by_a_flat_sheet_falls_in_two() {
        let cube = Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap();
        let cutter = Body::extruded(
            &[Curve::line([-1.0, 1.0, -1.0], [3.0, 1.0, -1.0]).unwrap()],
            [0.0, 0.0, 1.0],
            4.0,
            false,
        )
        .unwrap();
        let (a, b) = cube.split_by(&cutter).unwrap().unwrap();
        assert!((a.volume().unwrap() - 4.0).abs() < 1e-6);
        assert!((b.volume().unwrap() - 4.0).abs() < 1e-6);
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
