//! STL, OBJ and PLY in `Import`, `Export` and `ExportSelected` (spec `docs/specs/mesh-and-subd.md`
//! §5, `export-formats.md` §1), through `curva-meshfiles`. Coming in, each mesh in the file is a
//! mesh object, named as in the file. Going out, meshes are written as they are, SubDs as their
//! smooth surface, and surfaces and solids meshed at the viewport's density until the mesh panel
//! comes (`export-formats.md` §2). These formats carry no unit: the numbers are in the model's.

use curva_document::{Document, Shape};
use curva_mesh::Mesh;
use std::path::Path;

/// The SubD levels a SubD is smoothed by when written to a mesh file: Rhino's default.
const SUBD_LEVELS: u8 = 4;

/// A mesh file format Curva reads and writes itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshFormat {
    /// STL, written binary (as Rhino does by default).
    Stl,
    /// Wavefront OBJ.
    Obj,
    /// PLY, written binary: Rhino reads text PLY at single precision.
    Ply,
}

impl MeshFormat {
    /// The format a path's extension names, if Curva has it.
    pub fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "stl" => Some(Self::Stl),
            "obj" => Some(Self::Obj),
            "ply" => Some(Self::Ply),
            _ => None,
        }
    }
}

/// The meshes in a mesh file added to the model, each a mesh object named as in the file.
/// Returns a summary for the command line.
///
/// # Errors
/// When the file cannot be read or holds no meshes.
pub fn import_mesh_file(
    doc: &mut Document,
    path: &Path,
    format: MeshFormat,
) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let contents = match format {
        MeshFormat::Stl => curva_meshfiles::stl::read(&data),
        MeshFormat::Obj => curva_meshfiles::obj::read(&data),
        MeshFormat::Ply => curva_meshfiles::ply::read(&data),
    }
    .map_err(|e| e.to_string())?;
    if contents.meshes.is_empty() {
        return Err("the file holds no meshes".to_owned());
    }
    let count = contents.meshes.len();
    for named in contents.meshes {
        let id = doc.add_shape(Shape::Mesh(named.mesh));
        if !named.name.is_empty() {
            doc.rename(&[id], &named.name);
        }
    }
    let skipped: usize = contents.skipped.iter().map(|(_, n)| n).sum();
    Ok(if skipped == 0 {
        format!("{count} meshes")
    } else {
        format!("{count} meshes; {skipped} other items left out")
    })
}

/// The model's meshes, SubDs, surfaces and solids, or those among the objects in `only`, written
/// to a mesh file through a partial file renamed into place. Returns a summary for the command line.
///
/// # Errors
/// When there is nothing to write, or the file cannot be saved.
pub fn export_mesh_file(
    doc: &Document,
    only: Option<&[u64]>,
    path: &Path,
    format: MeshFormat,
) -> Result<String, String> {
    let mut meshes: Vec<(String, Mesh)> = Vec::new();
    let mut other = 0usize;
    for object in &doc.state().objects {
        if only.is_some_and(|ids| !ids.contains(&object.id))
            || (only.is_none() && (object.space.is_some() || object.hidden))
        {
            continue;
        }
        let mesh = match &object.shape {
            Shape::Mesh(mesh) => Some(mesh.clone()),
            Shape::SubD(subd) => subd.surface(SUBD_LEVELS).ok(),
            Shape::Body(body) => Mesh::from_body(body).ok(),
            Shape::Curve(_) | Shape::Instance(_) | Shape::Annotation(_) => None,
        };
        match mesh {
            Some(mesh) if mesh.face_count() > 0 => meshes.push((object.name.clone(), mesh)),
            _ => other += 1,
        }
    }
    if meshes.is_empty() {
        return Err("there are no meshes, SubDs, surfaces or solids to write".to_owned());
    }
    let list: Vec<(&str, &Mesh)> = meshes.iter().map(|(n, m)| (n.as_str(), m)).collect();
    let bytes = match format {
        MeshFormat::Stl => curva_meshfiles::stl::write_binary(&list),
        MeshFormat::Obj => curva_meshfiles::obj::write(&list).into_bytes(),
        MeshFormat::Ply => curva_meshfiles::ply::write_binary(&list),
    };
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    Ok(if other == 0 {
        format!("{} meshes", meshes.len())
    } else {
        format!("{} meshes; {other} other objects left out", meshes.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Body;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("curva-meshfiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn formats_are_known_by_their_extension() {
        assert_eq!(MeshFormat::of(Path::new("a.STL")), Some(MeshFormat::Stl));
        assert_eq!(MeshFormat::of(Path::new("a.obj")), Some(MeshFormat::Obj));
        assert_eq!(MeshFormat::of(Path::new("a.ply")), Some(MeshFormat::Ply));
        assert_eq!(MeshFormat::of(Path::new("a.3dm")), None);
    }

    #[test]
    fn a_solid_goes_out_as_a_mesh_and_comes_back_in_each_format() {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [10.0; 3]).unwrap());
        doc.rename(&[id], "Block");
        for (name, format) in [
            ("box.stl", MeshFormat::Stl),
            ("box.obj", MeshFormat::Obj),
            ("box.ply", MeshFormat::Ply),
        ] {
            let path = scratch(name);
            assert_eq!(
                export_mesh_file(&doc, None, &path, format).unwrap(),
                "1 meshes"
            );
            let mut back = Document::default();
            assert_eq!(
                import_mesh_file(&mut back, &path, format).unwrap(),
                "1 meshes"
            );
            let mesh = back.state().objects[0].shape.mesh().unwrap();
            assert!((mesh.volume().unwrap() - 1000.0).abs() < 1e-6, "{name}");
            std::fs::remove_file(&path).ok();
        }
    }

    #[test]
    fn export_selected_writes_only_the_chosen_objects() {
        let mut doc = Document::default();
        let a = doc.add_body(Body::cuboid([0.0; 3], [10.0; 3]).unwrap());
        doc.add_body(Body::cuboid([20.0, 0.0, 0.0], [10.0; 3]).unwrap());
        let path = scratch("one.stl");
        export_mesh_file(&doc, Some(&[a]), &path, MeshFormat::Stl).unwrap();
        let mut back = Document::default();
        import_mesh_file(&mut back, &path, MeshFormat::Stl).unwrap();
        let mesh = back.state().objects[0].shape.mesh().unwrap();
        assert!((mesh.volume().unwrap() - 1000.0).abs() < 1e-6);
        std::fs::remove_file(&path).ok();
    }
}
