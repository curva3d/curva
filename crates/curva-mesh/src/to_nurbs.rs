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
    /// patches stray from the true surface at most (0 when the SubD is regular everywhere). With
    /// `pack` the patches are joined into as few faces as Rhino's `PackFaces` makes.
    ///
    /// # Errors
    /// When the smooth surface cannot be worked out or the kernel cannot sew the patches.
    pub fn to_nurbs(&self, levels: u8, pack: bool) -> Result<(Body, f64), ToNurbsError> {
        let (patches, quads, from_quads) = self
            .patches_and_quads(levels)
            .map_err(ToNurbsError::Smooth)?;
        let surfaces = if pack {
            crate::pack::packed(&patches, &quads, &from_quads)
        } else {
            patches
                .iter()
                .map(|p| crate::pack::Packed {
                    net: p.net.iter().map(|row| row.to_vec()).collect(),
                    spans: [1, 1],
                    off: p.off,
                })
                .collect()
        };
        let off = surfaces.iter().fold(0.0_f64, |m, s| m.max(s.off));
        let bodies = surfaces
            .iter()
            .map(|s| {
                let [u, v] = s.spans.map(crate::pack::Packed::knots);
                NurbsSurface::new([3, 3], u, v, &s.net, None)?.to_body()
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
        let (body, off) = subd.to_nurbs(1, false).unwrap();
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
        let (body, off) = cube.to_nurbs(2, false).unwrap();
        assert!(body.is_solid(), "the patches meet without gaps");
        let limit = cube.surface(6).unwrap().volume().unwrap();
        let volume = body.volume().unwrap();
        assert!(
            (volume / limit - 1.0).abs() < 2e-3,
            "{volume} against {limit}"
        );
        // Two levels in, the fitted patches stray well under a hundredth of the cube's size.
        assert!(off < 0.02 && off > 0.0, "off {off}");
        let (_, finer) = cube.to_nurbs(3, false).unwrap();
        assert!(
            finer < off,
            "finer patches stray less: {finer} against {off}"
        );
    }

    /// An open 3 × 3 grid, bent: the kernel cannot yet sew patches lying flat in one plane
    /// (status 1007), though each builds alone.
    fn bent_grid() -> SubD {
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
        SubD::new(Mesh::new(points, &faces).unwrap())
    }

    #[test]
    fn an_open_creased_subd_becomes_an_open_polysurface() {
        let grid = bent_grid();
        let (body, _) = grid.to_nurbs(1, false).unwrap();
        assert!(!body.is_solid());
        assert_eq!(body.face_count().unwrap(), 36);
        let limit = grid.surface(6).unwrap().area();
        let area = body.area().unwrap();
        assert!((area / limit - 1.0).abs() < 2e-3, "{area} against {limit}");
    }

    /// The volume of a solid, the area of an open polysurface.
    fn size(body: &Body) -> f64 {
        if body.is_solid() {
            body.volume().unwrap()
        } else {
            body.area().unwrap()
        }
    }

    #[test]
    fn faces_pack_as_rhinos_pack_faces() {
        // Measured in Rhino 8 (`SubD.ToBrep` with `PackFaces`): boxes of 1, 2 or 3 faces a side
        // and a 2 × 1 × 1 box pack into 6 faces, an open 3 × 3 plane into 1, a cylinder with
        // triangle caps into 49, an open tube into 1, a torus into 1 closed both ways. The
        // volumes are Rhino's for the same boxes; elsewhere packing must leave the size as the
        // unpacked patches give it.
        let world = Frame::world();
        let cases = [
            (
                "cube",
                SubD::subd_box(&world, [2.0; 3], [1, 1, 1]),
                6,
                Some(2.620_38),
            ),
            (
                "2 a side",
                SubD::subd_box(&world, [2.0; 3], [2, 2, 2]),
                6,
                Some(6.294_84),
            ),
            // Unpacked, the middle patch of each side is flat and the kernel cannot sew it.
            (
                "3 a side",
                SubD::subd_box(&world, [2.0; 3], [3, 3, 3]),
                6,
                Some(7.198_47),
            ),
            (
                "2 x 1 x 1",
                SubD::subd_box(&world, [4.0, 2.0, 1.0], [2, 1, 1]),
                6,
                Some(3.978_48),
            ),
            ("plane", bent_grid(), 1, None),
            // The side packs into one face closed around; each cap triangle keeps its 3 quads.
            (
                "triangle caps",
                SubD::subd_cylinder(&world, 1.0, 3.0, (4, 8), true, false),
                49,
                None,
            ),
            (
                "open tube",
                SubD::subd_cylinder(&world, 1.0, 3.0, (4, 8), false, false),
                1,
                None,
            ),
            // Rhino makes the torus 1 face closed both ways; Curva's kernel cannot make a solid
            // of that, so it is cut into 2, each closed one way.
            ("torus", torus_cage(), 2, None),
        ];
        for (name, subd, faces, rhino) in cases {
            let (packed, _) = subd.to_nurbs(1, true).expect(name);
            assert_eq!(packed.face_count().unwrap(), faces, "{name}");
            let ours = size(&packed);
            let (expected, within) = match rhino {
                Some(volume) => {
                    assert!(packed.is_solid(), "{name} is solid");
                    (volume, 2e-3)
                }
                None => {
                    let (apart, _) = subd.to_nurbs(1, false).expect(name);
                    assert_eq!(packed.is_solid(), apart.is_solid(), "{name}");
                    (size(&apart), 1e-6)
                }
            };
            assert!(
                (ours / expected - 1.0).abs() < within,
                "{name}: {ours} against {expected}"
            );
        }
    }
}

#[cfg(test)]
mod flat_patches {
    use crate::{Frame, SubD};

    /// #506: a box of 3 × 3 × 3 faces, whose middle patches lie flat, becomes a solid without
    /// packing (the kernel refused to sew flat patches).
    #[test]
    fn flat_patches_are_sewn() {
        let cube = SubD::subd_box(&Frame::world(), [2.0; 3], [3, 3, 3]);
        let (body, _) = cube.to_nurbs(1, false).unwrap();
        assert!(body.is_solid());
        assert_eq!(body.face_count().unwrap(), 216);
    }
}
