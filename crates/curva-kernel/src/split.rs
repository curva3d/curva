//! Cutting a solid in two with a surface.

use crate::body::failed;
use crate::{Body, KernelError};

impl Body {
    /// The solid cut in two by a surface: the parts behind and in front of it, or `None` when
    /// the surface does not divide it.
    pub fn split_by(&self, sheet: &Body) -> Result<Option<(Body, Body)>, KernelError> {
        // A cutter whose box does not reach the solid's box cannot divide it.
        let (low, high) = self.bounds()?;
        let (sheet_low, sheet_high) = sheet.bounds()?;
        if (0..3).any(|i| sheet_high[i] < low[i] || sheet_low[i] > high[i]) {
            return Ok(None);
        }
        let body = self.engine_body()?;
        let tool = sheet.engine_body()?;
        let (_, order) = tool
            .to_record()
            .map_err(|e| failed("reading the cutter", e))?;
        let parts = solidcore_ops::split::split_body_by_sheet(&body, &tool, &order, 0)
            .map_err(|e| failed("cutting", e))?;
        let Some((a, b)) = parts else {
            return Ok(None);
        };
        let (a, b) = (Self::made(&a)?, Self::made(&b)?);
        // The engine gives two whole copies when the cutter misses the solid.
        let whole = |part: &Body| -> Result<bool, KernelError> {
            let (lo, hi) = part.bounds()?;
            let size = (0..3).map(|i| high[i] - low[i]).fold(1.0, f64::max);
            Ok((0..3).all(|i| {
                (lo[i] - low[i]).abs() <= 1e-9 * size && (hi[i] - high[i]).abs() <= 1e-9 * size
            }))
        };
        if whole(&a)? && whole(&b)? {
            return Ok(None);
        }
        Ok(Some((a, b)))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

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
    fn a_sheet_that_misses_a_box_does_not_divide_it() {
        let cube = Body::cuboid([0.0, 0.0, 2000.0], [1000.0, 1000.0, 200.0]).unwrap();
        let sheet = |size: f64| {
            Body::planar(&[Curve::polyline(
                vec![
                    [-size, -size, 1200.0],
                    [size, -size, 1200.0],
                    [size, size, 1200.0],
                    [-size, size, 1200.0],
                ],
                true,
            )
            .unwrap()])
            .unwrap()
        };
        // Wide enough to pass under the whole box, and narrower than it.
        assert!(cube.split_by(&sheet(9000.0)).unwrap().is_none());
        assert!(cube.split_by(&sheet(400.0)).unwrap().is_none());
        // An upright sheet past the box's corner, whose own box overlaps the box's.
        let past_the_corner = Body::extruded(
            &[Curve::line([500.0, -600.0, 1900.0], [-600.0, 500.0, 1900.0]).unwrap()],
            [0.0, 0.0, 1.0],
            400.0,
            false,
        )
        .unwrap();
        assert!(cube.split_by(&past_the_corner).unwrap().is_none());
    }
}
