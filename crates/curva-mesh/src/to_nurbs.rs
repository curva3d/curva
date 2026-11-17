//! ToNURBS: a SubD's smooth surface as a NURBS polysurface, through Curva's kernel (spec
//! `docs/specs/mesh-and-subd.md` §3.3). The patches come from [`SubD::bezier_patches`]: exact on
//! regular quads, fitted elsewhere; each becomes a bicubic NURBS surface, and the kernel sews them
//! into one polysurface, a solid when the SubD is closed.

use crate::{SmoothError, SubD};
use curva_kernel::{Body, KernelError, NurbsSurface};
use std::fmt;

/// Why a SubD could not be turned into NURBS.
#[derive(Clone, Debug, PartialEq)]
pub enum ToNurbsError {
    /// The smooth surface could not be worked out.
    Smooth(SmoothError),
    /// The kernel could not make or sew the patches.
    Kernel(String),
}

impl fmt::Display for ToNurbsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Smooth(e) => write!(f, "{e}"),
            Self::Kernel(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for ToNurbsError {}

impl From<KernelError> for ToNurbsError {
    fn from(e: KernelError) -> Self {
        Self::Kernel(e.0)
    }
}

impl SubD {
    /// The smooth surface as a polysurface of bicubic patches, one per quad of the cage
    /// subdivided `levels` times (1 is one patch per corner of each cage face), and how far the
    /// patches stray from the true surface at most (0 when the SubD is regular everywhere).
    ///
    /// # Errors
    /// When the smooth surface cannot be worked out or the kernel cannot sew the patches.
    pub fn to_nurbs(&self, levels: u8) -> Result<(Body, f64), ToNurbsError> {
        let patches = self.bezier_patches(levels).map_err(ToNurbsError::Smooth)?;
        let clamped = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let mut off: f64 = 0.0;
        let bodies = patches
            .iter()
            .map(|patch| {
                off = off.max(patch.off);
                let net: Vec<Vec<[f64; 3]>> = patch.net.iter().map(|row| row.to_vec()).collect();
                NurbsSurface::new([3, 3], clamped.clone(), clamped.clone(), &net, None)?.to_body()
            })
            .collect::<Result<Vec<Body>, KernelError>>()?;
        Ok((Body::assembled(&bodies)?, off))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame, Mesh, Point};

    fn torus_cage() -> SubD {
        SubD::subd_torus(&Frame::world(), 3.0, 1.0, (6, 10)).unwrap()
    }

    #[test]
    fn a_regular_subd_becomes_exact_patches_and_a_solid() {
        // A torus cage is regular everywhere: every patch is the smooth surface itself.
        let subd = torus_cage();
        let (body, off) = subd.to_nurbs(1).unwrap();
        assert_eq!(off, 0.0);
        assert!(body.is_solid());
        assert_eq!(body.face_count().unwrap(), 4 * 60);
        // Rhino gives this torus's limit surface as 58.934979.
        let volume = body.volume().unwrap();
        assert!((volume / 58.934_979 - 1.0).abs() < 1e-5, "{volume}");
    }

    #[test]
    fn around_star_points_the_patches_stay_close_and_close_up() {
        // A cube's corners meet three faces each.
        let cube = SubD::subd_box(&Frame::world(), [2.0; 3], [1, 1, 1]);
        let (body, off) = cube.to_nurbs(2).unwrap();
        assert!(body.is_solid(), "the patches meet without gaps");
        let limit = cube.surface(6).unwrap().volume().unwrap();
        let volume = body.volume().unwrap();
        assert!(
            (volume / limit - 1.0).abs() < 2e-3,
            "{volume} against {limit}"
        );
        // Two levels in, the fitted patches stray well under a hundredth of the cube's size.
        assert!(off < 0.02 && off > 0.0, "off {off}");
        let (_, finer) = cube.to_nurbs(3).unwrap();
        assert!(
            finer < off,
            "finer patches stray less: {finer} against {off}"
        );
    }

    #[test]
    fn an_open_creased_subd_becomes_an_open_polysurface() {
        // Bent, as the kernel cannot yet make a flat bicubic patch into a body (#328).
        let points: Vec<Point> = (0..4u32)
            .flat_map(|j| {
                (0..4u32)
                    .map(move |i| [f64::from(i), f64::from(j), 0.1 * f64::from((i * 3 + j) % 4)])
            })
            .collect();
        let at = |i: u32, j: u32| j * 4 + i;
        let faces: Vec<[u32; 4]> = (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)])
            })
            .collect();
        let grid = SubD::new(Mesh::new(points, &faces).unwrap());
        let (body, _) = grid.to_nurbs(1).unwrap();
        assert!(!body.is_solid());
        assert_eq!(body.face_count().unwrap(), 36);
        let limit = grid.surface(6).unwrap().area();
        let area = body.area().unwrap();
        assert!((area / limit - 1.0).abs() < 2e-3, "{area} against {limit}");
    }
}
