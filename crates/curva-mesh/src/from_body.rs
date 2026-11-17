//! Rhino's `Mesh` command: NURBS surfaces, polysurfaces and solids as meshes, from the kernel's
//! triangles through `curva-kernel` (spec `docs/specs/mesh-and-subd.md` §1), with Rhino's simple
//! density slider.
//!
//! Rhino's slider runs from 0 (coarse) to 1 (fine), 0.5 by default, and its meaning lives in
//! Rhino's own mesher, which openNURBS does not publish. Curva's is its own: 0.5 is what the
//! viewport draws (facets within a thousandth of the object's size, none turning more than 20°),
//! and each end is about forty times coarser or finer.

use crate::{Attribute, Mesh};
use curva_kernel::{Body, BodyMesh, KernelError};

/// The meshing tolerances for a density from 0 to 1 and an object `size` (its box's diagonal):
/// the farthest a facet may stand off the surface, and the most it may turn, in radians. Densities
/// outside 0 to 1 are taken as the nearer end, and an unreadable one as 0.5, as Rhino does.
#[must_use]
pub fn density_tolerances(density: f64, size: f64) -> (f64, f64) {
    let d = if density.is_finite() {
        density.clamp(0.0, 1.0)
    } else {
        0.5
    };
    // Size fractions from 1/50 at 0, through 1/1000 at 0.5, to 1/20000 at 1, evenly in their
    // logarithm; angles from 45° through 20° to 5°.
    let chord = size / 50.0 * 400f64.powf(-d);
    let degrees = if d <= 0.5 {
        45.0 - 50.0 * d
    } else {
        20.0 - 30.0 * (d - 0.5)
    };
    (chord, degrees.to_radians())
}

impl Mesh {
    /// The kernel's triangles of a body as a mesh, with their normals. Each face of the body keeps
    /// its own points along its edges, as Rhino's `Mesh` gives one point per face at each place;
    /// Curva still sees the faces as joined there (see [`Mesh::places`]). Triangles the kernel
    /// leaves with a point twice are left out.
    #[must_use]
    pub fn from_body_mesh(made: &BodyMesh) -> Self {
        let count = made.positions.len();
        let faces: Vec<[u32; 3]> = made
            .triangles
            .iter()
            .copied()
            .filter(|t| {
                t.iter().all(|&v| (v as usize) < count)
                    && t[0] != t[1]
                    && t[1] != t[2]
                    && t[0] != t[2]
            })
            .collect();
        let Ok(mesh) = Mesh::new(made.positions.clone(), &faces) else {
            return Mesh::default();
        };
        if made.normals.len() == count && count > 0 {
            mesh.clone()
                .with_normals(Attribute::PerVertex(made.normals.clone()))
                .unwrap_or(mesh)
        } else {
            mesh
        }
    }

    /// A body as a mesh at the viewport's density (0.5 on Rhino's slider).
    ///
    /// # Errors
    /// When the kernel cannot mesh the body.
    pub fn from_body(body: &Body) -> Result<Self, KernelError> {
        Ok(Self::from_body_mesh(&body.mesh()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    #[test]
    fn the_middle_of_the_slider_is_what_the_viewport_draws() {
        let (chord, angle) = density_tolerances(0.5, 10.0);
        assert!(close(chord, 0.01));
        assert!(close(angle, 20f64.to_radians()));
        let (coarse, wide) = density_tolerances(0.0, 10.0);
        let (fine, narrow) = density_tolerances(1.0, 10.0);
        assert!(close(coarse, 0.2) && close(wide, 45f64.to_radians()));
        assert!(close(fine, 0.0005) && close(narrow, 5f64.to_radians()));
        assert_eq!(density_tolerances(-3.0, 10.0), (coarse, wide));
        assert_eq!(
            density_tolerances(f64::NAN, 10.0),
            density_tolerances(0.5, 10.0)
        );
    }

    #[test]
    fn a_box_becomes_a_closed_mesh_of_its_volume() {
        let body = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let mesh = Mesh::from_body(&body).unwrap();
        assert!(mesh.face_count() >= 12);
        assert!(mesh.is_closed(), "faces meet by place");
        assert!(close(mesh.volume().unwrap(), 24.0));
        assert!(close(mesh.area(), 52.0));
        assert!(
            matches!(mesh.normals(), Attribute::PerVertex(n) if n.len() == mesh.vertex_count())
        );
    }

    #[test]
    fn a_sphere_becomes_a_closed_mesh_close_to_its_volume() {
        let body = Body::sphere([1.0, 2.0, 3.0], 5.0).unwrap();
        let mesh = Mesh::from_body(&body).unwrap();
        assert!(mesh.is_closed());
        let ball = 4.0 / 3.0 * std::f64::consts::PI * 125.0;
        let kept = mesh.volume().unwrap() / ball;
        // Facets within a thousandth of the size lie inside the sphere, a little short of it.
        assert!(kept > 0.99 && kept < 1.0, "kept {kept}");
        assert!(mesh.check(1e-9).is_good(), "{:?}", mesh.check(1e-9));
    }
}
