//! SubD primitives, as Rhino 8's SubDBox, SubDPlane, SubDSphere, SubDCylinder, SubDCone and
//! SubDTorus make them (spec `docs/specs/mesh-and-subd.md` §1, Stage C), built in a [`Frame`].
//!
//! Each cage has the face counts and cap styles of the mesh primitive of the same name. Where
//! Rhino puts the size, measured on its own primitives (17 November 2026):
//! - **Box and plane:** the cage is the plain grid on the shape; a flat side's middle points stay
//!   on it, so the smooth surface still reaches the size given, its corners rounded. A plane's
//!   border is creased, with no corners.
//! - **Cylinder:** the rings are widened so the side's smooth surface stands at the radius: a
//!   ring of n points makes a smooth curve (1 + 4 + 1) / 6 of the way, so the cage stands
//!   6 / (4 + 2 cos(2π/n)) times out. Ends and heights are the plain cage's.
//! - **Cone:** the plain cage, its smooth surface a little inside it.
//! - **Sphere and torus:** every cage point placed so that its point on the smooth surface lies
//!   on the shape ([`SubD::fitted`]).
//!
//! All edges are smooth: Rhino's primitives have no creases.

use crate::{Frame, Mesh, Point, SmoothError, SphereStyle, SubD};

/// A mesh's points and faces alone, without its normals: a SubD's cage.
fn bare(mesh: &Mesh) -> Mesh {
    let faces: Vec<Vec<u32>> = mesh.faces().map(<[u32]>::to_vec).collect();
    Mesh::new(mesh.vertices().to_vec(), &faces).unwrap_or_default()
}

impl SubD {
    /// A box from the frame's origin, `size` along its three directions, with `counts` faces
    /// along each, as Rhino's SubDBox.
    #[must_use]
    pub fn subd_box(frame: &Frame, size: [f64; 3], counts: [u32; 3]) -> Self {
        Self::new(bare(&Mesh::mesh_box(frame, size, counts)))
    }

    /// A flat rectangle from the frame's origin, `size` along its first two directions, with
    /// `counts` faces along each, its border creased, as Rhino's SubDPlane.
    #[must_use]
    pub fn subd_plane(frame: &Frame, size: [f64; 2], counts: [u32; 2]) -> Self {
        Self::new(bare(&Mesh::mesh_plane(frame, size, counts)))
    }

    /// A sphere about the frame's origin whose smooth surface passes through each cage point's
    /// place on the sphere, as Rhino's SubDSphere in each of its styles.
    ///
    /// # Errors
    /// When the cage cannot be fitted.
    pub fn subd_sphere(
        frame: &Frame,
        radius: f64,
        style: SphereStyle,
    ) -> Result<Self, SmoothError> {
        let shape = bare(&Mesh::mesh_sphere(frame, radius, style));
        let targets: Vec<Point> = shape.vertices().to_vec();
        Self::new(shape).fitted(&targets)
    }

    /// A cylinder standing on the frame's origin, as Rhino's SubDCylinder: its side's smooth
    /// surface at `radius`, with `vertical` faces up it and `around` it, closed at both ends when
    /// `solid`, the ends filled with quads when `quad_caps` and `around` is even.
    #[must_use]
    pub fn subd_cylinder(
        frame: &Frame,
        radius: f64,
        height: f64,
        (vertical, around): (u32, u32),
        solid: bool,
        quad_caps: bool,
    ) -> Self {
        let n = f64::from(around.max(3));
        let widened = radius * 6.0 / (4.0 + 2.0 * (std::f64::consts::TAU / n).cos());
        Self::new(bare(&Mesh::mesh_cylinder(
            frame,
            widened,
            height,
            (vertical, around),
            solid,
            quad_caps,
        )))
    }

    /// A cone standing on the frame's origin with its point `height` up, as Rhino's SubDCone:
    /// the plain cage, `vertical` faces from the base to the point and `around` it, closed at the
    /// base when `solid`, with quads there when `quad_caps` and `around` is even.
    #[must_use]
    pub fn subd_cone(
        frame: &Frame,
        radius: f64,
        height: f64,
        (vertical, around): (u32, u32),
        solid: bool,
        quad_caps: bool,
    ) -> Self {
        Self::new(bare(&Mesh::mesh_cone(
            frame,
            radius,
            height,
            (vertical, around),
            solid,
            quad_caps,
        )))
    }

    /// A torus about the frame's origin whose smooth surface passes through each cage point's
    /// place on the torus, as Rhino's SubDTorus: `major` the ring's radius, `minor` the tube's,
    /// `vertical` faces round the tube and `around` round the ring.
    ///
    /// # Errors
    /// When the cage cannot be fitted.
    pub fn subd_torus(
        frame: &Frame,
        major: f64,
        minor: f64,
        (vertical, around): (u32, u32),
    ) -> Result<Self, SmoothError> {
        let shape = bare(&Mesh::mesh_torus(frame, major, minor, (vertical, around)));
        let targets: Vec<Point> = shape.vertices().to_vec();
        Self::new(shape).fitted(&targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VertexTag;

    // Rhino 8's own primitives, made with its commands' defaults (17 November 2026): SubDSphere
    // radius 1 (Quads, 24 faces); SubDBox 2 × 2 × 2 (2 faces a side); SubDCylinder radius 1,
    // height 2 (8 around, 4 up, triangle ends); SubDCone the same; SubDTorus 3 and 1 (10 around,
    // 6 round the tube); SubDPlane 3 × 3 (2 × 2 faces). Their smooth surfaces' volume and area
    // (Rhino's `ToBrep`) and extents are compared at six subdivision levels, whose facets stand
    // just inside the true surface (0.05% allowed).

    fn near(a: f64, b: f64) -> bool {
        (a / b - 1.0).abs() < 5e-4
    }

    fn sizes(subd: &SubD) -> (f64, f64, (Point, Point)) {
        let s = subd.surface(6).unwrap();
        (s.volume().unwrap_or(0.0), s.area(), s.bounds().unwrap())
    }

    fn world() -> Frame {
        Frame::world()
    }

    #[test]
    fn a_sphere_matches_rhinos() {
        let subd =
            SubD::subd_sphere(&world(), 1.0, SphereStyle::Quads { subdivisions: 1 }).unwrap();
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (26, 24)
        );
        assert!((0..26).all(|v| subd.vertex_tag(v) == VertexTag::Smooth));
        let (volume, area, (lo, hi)) = sizes(&subd);
        assert!(near(volume, 4.171_469), "{volume}");
        assert!(near(area, 12.531_904), "{area}");
        assert!((hi[0] - 1.0).abs() < 1e-6 && (lo[2] + 1.0).abs() < 1e-6);
        // Rhino's cage stands out by 1.114611, 1.207253 and 1.235463 at its three kinds of point.
        let mut radii: Vec<f64> = subd
            .cage()
            .vertices()
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
            .collect();
        radii.sort_by(f64::total_cmp);
        radii.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        assert_eq!(radii.len(), 3);
        for (ours, rhinos) in radii.iter().zip([1.114_611, 1.207_253, 1.235_463]) {
            assert!((ours - rhinos).abs() < 1e-5, "{ours} against {rhinos}");
        }
    }

    #[test]
    fn a_box_matches_rhinos() {
        let subd = SubD::subd_box(&world(), [2.0; 3], [2, 2, 2]);
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (26, 24)
        );
        let (volume, area, (lo, hi)) = sizes(&subd);
        assert!(near(volume, 6.294_838), "{volume}");
        assert!(near(area, 17.171_627), "{area}");
        assert!(lo.iter().all(|c| c.abs() < 1e-6) && hi.iter().all(|c| (c - 2.0).abs() < 1e-6));
    }

    #[test]
    fn a_cylinder_matches_rhinos() {
        let subd = SubD::subd_cylinder(&world(), 1.0, 2.0, (4, 8), true, false);
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (42, 48)
        );
        let radius =
            (subd.cage().vertices()[0][0].powi(2) + subd.cage().vertices()[0][1].powi(2)).sqrt();
        assert!(
            (radius - 1.1082).abs() < 1e-4,
            "Rhino's cage radius is 1.1082: {radius}"
        );
        let (volume, area, (lo, hi)) = sizes(&subd);
        assert!(near(volume, 5.921_422), "{volume}");
        assert!(near(area, 16.740_162), "{area}");
        assert!((hi[0] - 1.0).abs() < 1e-3 && lo[2].abs() < 1e-6 && (hi[2] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_cone_matches_rhinos() {
        let subd = SubD::subd_cone(&world(), 1.0, 2.0, (4, 8), true, false);
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (34, 40)
        );
        let (volume, area, (_, hi)) = sizes(&subd);
        assert!(near(volume, 1.561_66), "{volume}");
        // The point, where eight faces meet, settles more slowly under subdivision: 0.1%.
        assert!((area / 7.363_838 - 1.0).abs() < 1e-3, "{area}");
        assert!(
            (hi[2] - 1.8205).abs() < 1e-3,
            "Rhino's surface rises to 1.8205: {}",
            hi[2]
        );
    }

    #[test]
    fn a_torus_matches_rhinos() {
        let subd = SubD::subd_torus(&world(), 3.0, 1.0, (6, 10)).unwrap();
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (60, 60)
        );
        let (volume, area, _) = sizes(&subd);
        assert!(near(volume, 58.934_979), "{volume}");
        assert!(near(area, 118.142_56), "{area}");
        // Rhino's cage rings stand at 1.9224, 2.5632, 3.8448 and 4.4856 from the axis.
        let mut radii: Vec<f64> = subd
            .cage()
            .vertices()
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
            .collect();
        radii.sort_by(f64::total_cmp);
        radii.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        for (ours, rhinos) in radii.iter().zip([1.9224, 2.5632, 3.8448, 4.4856]) {
            assert!((ours - rhinos).abs() < 1e-3, "{ours} against {rhinos}");
        }
    }

    #[test]
    fn a_plane_matches_rhinos() {
        let subd = SubD::subd_plane(&world(), [3.0, 3.0], [2, 2]);
        assert_eq!(
            (subd.cage().vertex_count(), subd.cage().face_count()),
            (9, 4)
        );
        assert_eq!(subd.creases().count(), 8);
        let (_, area, _) = sizes(&subd);
        assert!(near(area, 8.25), "{area}");
    }

    #[test]
    fn far_from_the_origin_the_fit_keeps_its_digits() {
        let frame = Frame {
            origin: [1.0e6, -2.0e6, 5.0e5],
            ..Frame::world()
        };
        let subd = SubD::subd_sphere(&frame, 1.0, SphereStyle::Quads { subdivisions: 1 }).unwrap();
        let (volume, _, _) = sizes(&subd);
        assert!(near(volume, 4.171_469), "{volume}");
    }
}
