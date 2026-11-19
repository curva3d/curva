//! How finely surfaces, solids and SubDs are turned into meshes: one set of settings shared by
//! the `Mesh` command, mesh file export and the mesh panel (spec `docs/specs/export-formats.md`
//! §2), so none of them has its own copy.
//!
//! The simple view is one slider, the density, and Rhino's presets. The advanced view adds the
//! limits Curva's mesher takes today: the most a facet may turn and the farthest it may stand off
//! the surface. Rhino's other detailed controls (aspect ratio, edge lengths, grid quads, refine,
//! jagged seams, simple planes) come when the kernel's mesher takes them.

use crate::{Mesh, density_tolerances};
use curva_kernel::{Body, KernelError};

/// The SubD level a SubD is smoothed to by default: Rhino's.
pub const DEFAULT_SUBD_LEVEL: u8 = 4;

/// The deepest SubD level the settings allow, as Rhino's absolute level.
pub const MOST_SUBD_LEVEL: u8 = 6;

/// One of Rhino's ready-made settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshPreset {
    /// Few faces, quick: Rhino's "Jagged & faster".
    Coarse,
    /// The `Mesh` command's starting density, 0.5.
    Default,
    /// Many faces: Rhino's "Smooth & slower", density 0.8 with facets turning at most 20°.
    Smooth,
}

/// How finely to mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshSettings {
    /// From 0 (fewer polygons) to 1 (more polygons), as Rhino's slider; 0.5 by default.
    pub density: f64,
    /// The most a facet may turn, in degrees, when tighter than the density's own; `None` leaves
    /// it to the density.
    pub max_angle: Option<f64>,
    /// The farthest a facet may stand off the surface, in model units, when tighter than the
    /// density's own; `None` leaves it to the density.
    pub max_distance: Option<f64>,
    /// How many times SubDs are subdivided, from 1 to [`MOST_SUBD_LEVEL`].
    pub subd_level: u8,
}

impl Default for MeshSettings {
    fn default() -> Self {
        Self::preset(MeshPreset::Default)
    }
}

impl MeshSettings {
    /// One of Rhino's ready-made settings. Rhino's presets are measured in Rhino 8
    /// (`MeshingParameters.FastRenderMesh` and `QualityRenderMesh`); Rhino's own density scale
    /// belongs to its mesher, so Coarse is set where Curva's slider gives a jagged mesh, and Smooth
    /// keeps Rhino's density and 20° limit.
    #[must_use]
    pub fn preset(preset: MeshPreset) -> Self {
        let (density, max_angle) = match preset {
            MeshPreset::Coarse => (0.2, None),
            MeshPreset::Default => (0.5, None),
            MeshPreset::Smooth => (0.8, Some(20.0)),
        };
        Self {
            density,
            max_angle,
            max_distance: None,
            subd_level: DEFAULT_SUBD_LEVEL,
        }
    }

    /// The preset these settings are, if they are one.
    #[must_use]
    pub fn as_preset(&self) -> Option<MeshPreset> {
        [MeshPreset::Coarse, MeshPreset::Default, MeshPreset::Smooth]
            .into_iter()
            .find(|p| Self::preset(*p) == *self)
    }

    /// The same settings with every value in its range: the density from 0 to 1 (an unreadable
    /// one as 0.5), limits that are not more than zero dropped, and the SubD level from 1 to
    /// [`MOST_SUBD_LEVEL`].
    #[must_use]
    pub fn checked(self) -> Self {
        let positive = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
        Self {
            density: if self.density.is_finite() {
                self.density.clamp(0.0, 1.0)
            } else {
                0.5
            },
            max_angle: positive(self.max_angle),
            max_distance: positive(self.max_distance),
            subd_level: self.subd_level.clamp(1, MOST_SUBD_LEVEL),
        }
    }

    /// The meshing tolerances for an object `size` across (its box's diagonal): the farthest a
    /// facet may stand off the surface, and the most it may turn, in radians. The density gives
    /// both; the advanced limits only tighten them.
    #[must_use]
    pub fn tolerances(&self, size: f64) -> (f64, f64) {
        let checked = self.checked();
        let (mut chord, mut angle) = density_tolerances(checked.density, size);
        if let Some(distance) = checked.max_distance {
            chord = chord.min(distance);
        }
        if let Some(degrees) = checked.max_angle {
            angle = angle.min(degrees.to_radians());
        }
        (chord, angle)
    }
}

impl Mesh {
    /// A surface or solid meshed with these settings, sized by its box's diagonal.
    ///
    /// # Errors
    /// When the kernel cannot measure or mesh the body.
    pub fn from_body_with(body: &Body, settings: &MeshSettings) -> Result<Self, KernelError> {
        let (low, high) = body.bounds()?;
        let size = (0..3)
            .map(|k| (high[k] - low[k]).powi(2))
            .sum::<f64>()
            .sqrt();
        let (chord, angle) = settings.tolerances(size);
        Ok(Self::from_body_mesh(&body.mesh_with(chord, Some(angle))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_middle_of_the_slider_and_rhinos_subd_level() {
        let settings = MeshSettings::default();
        assert_eq!(settings.density, 0.5);
        assert_eq!(settings.subd_level, 4);
        assert_eq!(settings.tolerances(10.0), density_tolerances(0.5, 10.0));
    }

    #[test]
    fn smooth_keeps_rhinos_density_and_twenty_degrees() {
        let smooth = MeshSettings::preset(MeshPreset::Smooth);
        assert_eq!(smooth.density, 0.8);
        let (_, angle) = smooth.tolerances(10.0);
        assert!(angle <= 20f64.to_radians() + 1e-12);
        assert_eq!(smooth.as_preset(), Some(MeshPreset::Smooth));
    }

    #[test]
    fn presets_go_from_fewer_to_more_polygons() {
        let chord = |p| MeshSettings::preset(p).tolerances(10.0).0;
        assert!(chord(MeshPreset::Coarse) > chord(MeshPreset::Default));
        assert!(chord(MeshPreset::Default) > chord(MeshPreset::Smooth));
    }

    #[test]
    fn advanced_limits_only_tighten() {
        let loose = MeshSettings {
            max_distance: Some(1000.0),
            max_angle: Some(90.0),
            ..MeshSettings::default()
        };
        assert_eq!(loose.tolerances(10.0), density_tolerances(0.5, 10.0));
        let tight = MeshSettings {
            max_distance: Some(0.001),
            max_angle: Some(2.0),
            ..MeshSettings::default()
        };
        let (chord, angle) = tight.tolerances(10.0);
        assert_eq!(chord, 0.001);
        assert!((angle - 2f64.to_radians()).abs() < 1e-12);
        assert_eq!(tight.as_preset(), None);
    }

    #[test]
    fn more_density_gives_more_faces() {
        let body = Body::sphere([0.0; 3], 5.0).unwrap();
        let faces = |p| {
            Mesh::from_body_with(&body, &MeshSettings::preset(p))
                .unwrap()
                .face_count()
        };
        let (coarse, default, smooth) = (
            faces(MeshPreset::Coarse),
            faces(MeshPreset::Default),
            faces(MeshPreset::Smooth),
        );
        assert!(
            coarse < default && default < smooth,
            "{coarse} {default} {smooth}"
        );
    }

    #[test]
    fn values_out_of_range_are_brought_in() {
        let wild = MeshSettings {
            density: f64::NAN,
            max_angle: Some(-3.0),
            max_distance: Some(0.0),
            subd_level: 40,
        }
        .checked();
        assert_eq!(wild.density, 0.5);
        assert_eq!(wild.max_angle, None);
        assert_eq!(wild.max_distance, None);
        assert_eq!(wild.subd_level, MOST_SUBD_LEVEL);
    }
}
