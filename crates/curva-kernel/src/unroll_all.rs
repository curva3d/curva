//! Every face of a surface or polysurface laid flat, as Rhino's UnrollSrf does (#709): flat faces
//! turned onto the world XY plane, ruled developable faces unrolled, the pieces side by side.

use crate::record::Surface;
use crate::{Body, KernelError, Point, Similarity};

fn plus(p: Point, d: Point) -> Point {
    std::array::from_fn(|i| p[i] + d[i])
}

/// Space between pieces laid side by side, as a share of the widest.
const SPACING: f64 = 0.1;

impl Body {
    /// Each face laid flat on the world XY plane, the pieces side by side along X from the origin:
    /// a flat face turned into the plane, a ruled developable face unrolled (`Body::unrolled`).
    /// A face that cannot be laid flat is refused.
    pub fn unrolled_faces(&self) -> Result<Vec<Body>, KernelError> {
        let count = self.face_count()?;
        let surfaces = self.face_surfaces()?;
        let mut pieces = Vec::new();
        for face in 0..count {
            let one = if count == 1 {
                self.clone()
            } else {
                self.faces_only(&[face])?
            };
            let flat = match surfaces.get(face) {
                Some(Surface::Plane {
                    origin,
                    u_axis,
                    v_axis,
                }) => {
                    let from = [*origin, plus(*origin, *u_axis), plus(*origin, *v_axis)];
                    let to = [
                        *origin,
                        plus(*origin, [1.0, 0.0, 0.0]),
                        plus(*origin, [0.0, 1.0, 0.0]),
                    ];
                    let turn = Similarity::between_frames(*origin, from, to)
                        .ok_or_else(|| KernelError("a flat face has no plane".to_owned()))?;
                    one.mapped(&turn).translated(origin.map(|v| -v))
                }
                _ => one.unrolled()?,
            };
            pieces.push(flat);
        }
        let boxes = pieces
            .iter()
            .map(Body::bounds)
            .collect::<Result<Vec<_>, _>>()?;
        let widest = boxes
            .iter()
            .map(|(lo, hi)| hi[0] - lo[0])
            .fold(0.0, f64::max);
        let mut x = 0.0;
        let mut laid = Vec::with_capacity(pieces.len());
        for (piece, (lo, hi)) in pieces.into_iter().zip(boxes) {
            laid.push(piece.translated([x - lo[0], -lo[1], -lo[2]]));
            x += hi[0] - lo[0] + SPACING * widest.max(1.0);
        }
        Ok(laid)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    #[test]
    fn a_closed_cylinder_lays_out_its_side_and_both_caps_as_rhino_does() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let cylinder = Body::extruded(&[circle], [0.0, 0.0, 1.0], 10.0, true).unwrap();
        let pieces = cylinder.unrolled_faces().unwrap();
        assert_eq!(pieces.len(), 3);
        let mut areas: Vec<f64> = pieces.iter().map(|p| p.area().unwrap()).collect();
        areas.sort_by(f64::total_cmp);
        let disc = std::f64::consts::PI * 25.0;
        assert!(
            (areas[0] - disc).abs() < 1e-3 && (areas[1] - disc).abs() < 1e-3,
            "{areas:?}"
        );
        assert!((areas[2] - 314.1593).abs() < 1e-2, "{areas:?}");
        // All flat on the world XY plane.
        for piece in &pieces {
            let (lo, hi) = piece.bounds().unwrap();
            assert!(lo[2].abs() < 1e-9 && hi[2].abs() < 1e-9);
        }
    }
}
