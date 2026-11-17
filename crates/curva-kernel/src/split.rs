//! Cutting a solid in two with a surface.

use crate::body::failed;
use crate::{Body, KernelError};

impl Body {
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
}
