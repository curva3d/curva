//! Working on bodies already made: pipes along curves and offset faces.

use crate::making::sweep_pieces;
use crate::{Body, Curve, KernelError, Point, cross, sub, unit};
use solidcore_ops::sweep::{SweepFrame, sweep_placed};
use solidcore_ops::{offset_sheet, thicken};

fn failed(what: &str, error: impl std::fmt::Debug) -> KernelError {
    KernelError(format!("{what} failed: {error:?}"))
}

/// Two directions at right angles to `along` and to each other, both of length 1.
fn across(along: Point) -> Option<(Point, Point)> {
    let along = unit(along)?;
    let x = unit(cross(along, [0.0, 0.0, 1.0])).or_else(|| unit(cross(along, [1.0, 0.0, 0.0])))?;
    Some((x, cross(along, x)))
}

impl Body {
    /// A round tube of `radius` along `rail`, its ends closed with `cap`. With `inner`, a wall
    /// between the two radii.
    pub fn pipe(
        rail: &Curve,
        radius: f64,
        inner: Option<f64>,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let path = rail.sample()?;
        let (Some(start), Some(next)) = (path.first(), path.get(1)) else {
            return Err(KernelError("the rail is too short".to_owned()));
        };
        let (x, y) = across(sub(*next, *start))
            .ok_or_else(|| KernelError("the rail has no direction".to_owned()))?;
        let mut sections = vec![Curve::circle_on(*start, radius, x, y)?];
        if let Some(inner) = inner {
            sections.push(Curve::circle_on(*start, inner, x, y)?);
        }
        let mut profile = Vec::new();
        for section in &sections {
            profile.extend(sweep_pieces(section)?);
        }
        let (made, ..) = sweep_placed(&profile, &sweep_pieces(rail)?, SweepFrame::Follow, cap)
            .map_err(|e| failed("making a pipe", e))?;
        Self::made(&made)
    }

    /// The faces moved out along their surfaces by `distance` (negative goes the other way).
    /// With `solid`, an open surface is thickened into a solid instead, between where it was
    /// and the offset.
    pub fn offset(&self, distance: f64, solid: bool) -> Result<Self, KernelError> {
        let body = self.engine_body()?;
        if solid {
            thicken(&body, distance, false).map_err(|e| failed("thickening", e))?;
        } else {
            offset_sheet(&body, distance).map_err(|e| failed("offsetting", e))?;
        }
        Self::made(&body)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};
    use std::f64::consts::PI;

    fn close(a: f64, b: f64, share: f64) -> bool {
        (a - b).abs() <= share * b.abs().max(1.0)
    }

    #[test]
    fn a_straight_pipe_is_a_cylinder() {
        let rail = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        let solid = Body::pipe(&rail, 2.0, None, true).unwrap();
        assert!(close(solid.volume().unwrap(), PI * 4.0 * 10.0, 1e-4));
        let wall = Body::pipe(&rail, 2.0, Some(1.0), true).unwrap();
        assert!(close(wall.volume().unwrap(), PI * 3.0 * 10.0, 1e-4));
    }

    #[test]
    fn a_pipe_follows_a_bend() {
        let rail =
            Curve::polyline(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false).unwrap();
        let bent =
            Curve::interpolated(vec![[0.0; 3], [10.0, 2.0, 0.0], [20.0, 0.0, 0.0]], false).unwrap();
        let tube = Body::pipe(&bent, 1.0, None, true).unwrap();
        // One radius above and below the rail's plane, and one past the bend's peak at 2.
        // Measured by the box, as a curved solid's volume is slow to work out in a test build.
        let (low, high) = tube.bounds().unwrap();
        assert!(
            close(low[2], -1.0, 1e-3) && close(high[2], 1.0, 1e-3),
            "{low:?} {high:?}"
        );
        assert!(close(high[1], 3.0, 1e-3), "{high:?}");
        // A sharp corner in the rail is refused or mitred by the engine, but must not panic.
        let _ = Body::pipe(&rail, 1.0, None, true);
    }

    #[test]
    fn an_open_box_side_is_offset_outward() {
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let sheet = Body::planar(&[square]).unwrap();
        let moved = sheet.offset(2.0, false).unwrap();
        assert!(close(moved.area().unwrap(), 100.0, 1e-6));
        let (low, high) = moved.bounds().unwrap();
        assert!(close(low[2].abs(), 2.0, 1e-6) && close(high[2].abs(), 2.0, 1e-6));
        let slab = sheet.offset(2.0, true).unwrap();
        assert!(close(slab.volume().unwrap(), 200.0, 1e-6));
    }
}
