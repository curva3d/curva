//! Turning objects into meshes with the shared mesh settings (spec `export-formats.md` §2): what
//! the `Mesh` command, mesh file export and the mesh panel all use, so the meshing lives in one
//! place. The settings are remembered as the `Mesh` command's options.

use curva_document::{Document, Shape};
use curva_mesh::{Mesh, MeshSettings};

/// Where each setting is remembered: the `Mesh` command's option keys.
const DENSITY: &str = "mesh.density";
const MAX_ANGLE: &str = "mesh.maxangle";
const MAX_DISTANCE: &str = "mesh.maxdistance";
const SUBD_LEVEL: &str = "mesh.subdlevel";

/// The mesh settings last used, by the `Mesh` command or the mesh panel, or the defaults.
#[must_use]
pub fn remembered_settings(doc: &Document) -> MeshSettings {
    let start = MeshSettings::default();
    let limit = |key| doc.remembered(key).filter(|v| *v > 0.0);
    MeshSettings {
        density: doc.remembered(DENSITY).unwrap_or(start.density),
        max_angle: limit(MAX_ANGLE),
        max_distance: limit(MAX_DISTANCE),
        subd_level: doc
            .remembered(SUBD_LEVEL)
            .map_or(start.subd_level, |v| v.round().clamp(1.0, 255.0) as u8),
    }
    .checked()
}

/// Keeps mesh settings for the next `Mesh` or export, as the mesh panel's OK does.
pub fn remember_settings(doc: &mut Document, settings: &MeshSettings) {
    let settings = settings.checked();
    doc.remember(DENSITY, settings.density);
    doc.remember(MAX_ANGLE, settings.max_angle.unwrap_or(0.0));
    doc.remember(MAX_DISTANCE, settings.max_distance.unwrap_or(0.0));
    doc.remember(SUBD_LEVEL, f64::from(settings.subd_level));
}

/// A surface, solid or SubD as a mesh with these settings; a mesh as it is. `None` for anything
/// else (curves, block copies, annotation) or when meshing fails.
#[must_use]
pub fn mesh_of(shape: &Shape, settings: &MeshSettings) -> Option<Mesh> {
    let mesh = match shape {
        Shape::Mesh(mesh) => Some(mesh.clone()),
        Shape::SubD(subd) => subd.surface(settings.checked().subd_level).ok(),
        Shape::Body(body) => Mesh::from_body_with(body, settings).ok(),
        Shape::Curve(_) | Shape::Instance(_) | Shape::Annotation(_) => None,
    };
    mesh.filter(|m| m.face_count() > 0)
}

/// What meshing would give, for the mesh panel's line under the slider: *about 12,400 faces ·
/// strays at most 0.04 mm · about 0.6 MB*.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshNumbers {
    /// Faces in every mesh made, together.
    pub faces: usize,
    /// The farthest any facet may stand off its surface, in model units: the largest meshing
    /// tolerance used. Zero when only meshes and SubDs are meshed.
    pub strays: f64,
    /// Objects that cannot be meshed, left out.
    pub left_out: usize,
}

/// The meshes of the objects with these ids, with their names, and the numbers for the panel.
#[must_use]
pub fn mesh_objects(
    doc: &Document,
    ids: &[u64],
    settings: &MeshSettings,
) -> (Vec<(u64, String, Mesh)>, MeshNumbers) {
    let mut meshes = Vec::new();
    let mut numbers = MeshNumbers::default();
    for object in doc.state().objects.iter().filter(|o| ids.contains(&o.id)) {
        let Some(mesh) = mesh_of(&object.shape, settings) else {
            numbers.left_out += 1;
            continue;
        };
        if let Shape::Body(body) = &object.shape
            && let Ok((low, high)) = body.bounds()
        {
            let size = (0..3)
                .map(|k| (high[k] - low[k]).powi(2))
                .sum::<f64>()
                .sqrt();
            numbers.strays = numbers.strays.max(settings.tolerances(size).0);
        }
        numbers.faces += mesh.face_count();
        meshes.push((object.id, object.name.clone(), mesh));
    }
    (meshes, numbers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Body;
    use curva_mesh::MeshPreset;

    #[test]
    fn settings_are_remembered_and_read_back() {
        let mut doc = Document::default();
        assert_eq!(remembered_settings(&doc), MeshSettings::default());
        let smooth = MeshSettings::preset(MeshPreset::Smooth);
        remember_settings(&mut doc, &smooth);
        assert_eq!(remembered_settings(&doc), smooth);
    }

    #[test]
    fn the_numbers_follow_the_slider() {
        let mut doc = Document::default();
        let ball = doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        let line = doc.add_shape(Shape::Curve(
            curva_kernel::Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap(),
        ));
        let coarse = mesh_objects(
            &doc,
            &[ball, line],
            &MeshSettings::preset(MeshPreset::Coarse),
        )
        .1;
        let smooth = mesh_objects(
            &doc,
            &[ball, line],
            &MeshSettings::preset(MeshPreset::Smooth),
        )
        .1;
        assert_eq!(coarse.left_out, 1);
        assert!(coarse.faces < smooth.faces);
        assert!(coarse.strays > smooth.strays && smooth.strays > 0.0);
    }
}
