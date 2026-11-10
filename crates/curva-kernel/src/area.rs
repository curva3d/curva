//! The area inside closed flat curves.

use crate::{Curve, JOIN_TOLERANCE, KernelError, Point, cross, dot, gap, sub, unit};
use std::f64::consts::PI;

impl Curve {
    /// The area a closed, flat curve encloses.
    ///
    /// Circles are exact. Other curves use the drawn path, which follows the curve closely
    /// enough for measuring.
    pub fn area(&self) -> Result<f64, KernelError> {
        if let Self::Circle { radius, .. } = self {
            return Ok(PI * radius * radius);
        }
        if !self.is_closed()? {
            return Err(KernelError("the curve is not closed".to_owned()));
        }
        let path = self.sample()?;
        let first = path[0];
        // Newell's method: half the length of the summed cross products around the loop.
        let mut summed: Point = [0.0; 3];
        for pair in path.windows(2) {
            let step = cross(sub(pair[0], first), sub(pair[1], first));
            summed = std::array::from_fn(|axis| summed[axis] + step[axis]);
        }
        let Some(normal) = unit(summed) else {
            return Err(KernelError("the curve encloses no area".to_owned()));
        };
        if path
            .iter()
            .any(|p| dot(sub(*p, first), normal).abs() > JOIN_TOLERANCE)
        {
            return Err(KernelError("the curve is not flat".to_owned()));
        }
        Ok(gap(summed, [0.0; 3]) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::Curve;
    use std::f64::consts::PI;

    #[test]
    fn a_rectangle_is_exact_whichever_way_it_runs() {
        let corners = vec![
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 3.0, 0.0],
            [0.0, 3.0, 0.0],
        ];
        let forward = Curve::polyline(corners.clone(), true).unwrap();
        let backward = Curve::polyline(corners.into_iter().rev().collect(), true).unwrap();
        assert!((forward.area().unwrap() - 12.0).abs() < 1e-12);
        assert!((backward.area().unwrap() - 12.0).abs() < 1e-12);
    }

    #[test]
    fn a_circle_is_exact() {
        let circle = Curve::circle([5.0, 1.0, 2.0], 2.0).unwrap();
        assert!((circle.area().unwrap() - 4.0 * PI).abs() < 1e-12);
    }

    #[test]
    fn an_ellipse_is_close() {
        let ellipse = Curve::ellipse([0.0; 3], [3.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let area = ellipse.area().unwrap();
        assert!((area - 3.0 * PI).abs() < 3.0 * PI * 1e-3, "{area}");
    }

    #[test]
    fn a_square_standing_up_measures_the_same() {
        let square = Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 2.0, 2.0],
                [0.0, 0.0, 2.0],
            ],
            true,
        )
        .unwrap();
        assert!((square.area().unwrap() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn open_or_bent_curves_are_refused() {
        let open = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        assert!(open.area().is_err());
        let bent = Curve::polyline(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
            ],
            true,
        )
        .unwrap();
        assert!(bent.area().is_err());
    }
}
