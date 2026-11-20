//! STL, OBJ and PLY on Curva's list of formats (spec `docs/specs/file-formats.md` §3), moved
//! unchanged from the file commands: `Open`, `Import`, `Export` and `ExportSelected` (spec `docs/specs/mesh-and-subd.md`
//! §5, `export-formats.md` §1), through `curva-meshfiles`. Coming in, each mesh in the file is a
//! mesh object, named as in the file. Going out, meshes are written as they are, SubDs as their
//! smooth surface, and surfaces and solids meshed with the mesh settings last used by `Mesh` or
//! the mesh panel (`export-formats.md` §2). These formats carry no unit: the numbers are in the
//! model's.

use curva_document::formats::{Done, Format};
use curva_document::meshing::{mesh_objects, remembered_settings};
use curva_document::{Document, Shape};
use curva_mesh::Mesh;
use std::path::Path;

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

/// STL files, as the list holds them.
#[must_use]
pub fn stl() -> Format {
    Format {
        open: Some(|path, shown| open(path, shown, MeshFormat::Stl)),
        import: Some(|doc, path, shown| import(doc, path, shown, MeshFormat::Stl)),
        export: Some(|doc, only, path, shown| export(doc, only, path, shown, MeshFormat::Stl)),
        ..Format::new("files.stl", &["stl"])
    }
}

/// Wavefront OBJ files, as the list holds them.
#[must_use]
pub fn obj() -> Format {
    Format {
        open: Some(|path, shown| open(path, shown, MeshFormat::Obj)),
        import: Some(|doc, path, shown| import(doc, path, shown, MeshFormat::Obj)),
        export: Some(|doc, only, path, shown| export(doc, only, path, shown, MeshFormat::Obj)),
        ..Format::new("files.obj", &["obj"])
    }
}

/// PLY files, as the list holds them.
#[must_use]
pub fn ply() -> Format {
    Format {
        open: Some(|path, shown| open(path, shown, MeshFormat::Ply)),
        import: Some(|doc, path, shown| import(doc, path, shown, MeshFormat::Ply)),
        export: Some(|doc, only, path, shown| export(doc, only, path, shown, MeshFormat::Ply)),
        ..Format::new("files.ply", &["ply"])
    }
}

fn open(path: &Path, shown: &str, format: MeshFormat) -> Result<(Document, Done), String> {
    let (doc, summary) = open_mesh_file(path, format)?;
    let done = Done {
        key: "done.open_3dm",
        args: vec![shown.to_owned(), summary],
    };
    Ok((doc, done))
}

fn import(
    doc: &mut Document,
    path: &Path,
    shown: &str,
    format: MeshFormat,
) -> Result<Done, String> {
    let summary = import_mesh_file(doc, path, format)?;
    Ok(Done {
        key: "done.import",
        args: vec![shown.to_owned(), summary],
    })
}

fn export(
    doc: &Document,
    only: Option<&[u64]>,
    path: &Path,
    shown: &str,
    format: MeshFormat,
) -> Result<Done, String> {
    let summary = export_mesh_file(doc, only, path, format)?;
    Ok(Done {
        key: "done.export_cad",
        args: vec![shown.to_owned(), summary],
    })
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
        MeshFormat::Stl => crate::stl::read(&data),
        MeshFormat::Obj => crate::obj::read(&data),
        MeshFormat::Ply => crate::ply::read(&data),
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

/// A mesh file as a new model, unsaved, named after the file, as `.3dm` files open. Returns the
/// model and a summary for the command line.
///
/// # Errors
/// When the file cannot be read or holds no meshes.
pub fn open_mesh_file(path: &Path, format: MeshFormat) -> Result<(Document, String), String> {
    let mut doc = Document::default();
    let summary = import_mesh_file(&mut doc, path, format)?;
    let beside = path.with_extension("curva");
    doc.set_path((!beside.exists()).then_some(beside));
    Ok((doc, summary))
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
    let ids = exported_ids(doc, only);
    let (meshes, numbers) = mesh_objects(doc, &ids, &remembered_settings(doc));
    if meshes.is_empty() {
        return Err("there are no meshes, SubDs, surfaces or solids to write".to_owned());
    }
    let bytes = file_bytes(&meshes, format);
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    let other = numbers.left_out;
    Ok(if other == 0 {
        format!("{} meshes", meshes.len())
    } else {
        format!("{} meshes; {other} other objects left out", meshes.len())
    })
}

/// The objects an export writes: those in `only`, or every visible one in the model's space.
#[must_use]
pub fn exported_ids(doc: &Document, only: Option<&[u64]>) -> Vec<u64> {
    doc.state()
        .objects
        .iter()
        .filter(|o| match only {
            Some(ids) => ids.contains(&o.id),
            None => o.space.is_none() && !o.hidden,
        })
        .map(|o| o.id)
        .collect()
}

/// Meshes as a file's bytes, for writing and for the mesh panel's file size.
#[must_use]
pub fn file_bytes(meshes: &[(u64, String, Mesh)], format: MeshFormat) -> Vec<u8> {
    let list: Vec<(&str, &Mesh)> = meshes.iter().map(|(_, n, m)| (n.as_str(), m)).collect();
    match format {
        MeshFormat::Stl => crate::stl::write_binary(&list),
        MeshFormat::Obj => crate::obj::write(&list).into_bytes(),
        MeshFormat::Ply => crate::ply::write_binary(&list),
    }
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

    #[test]
    fn export_uses_the_density_last_given_to_mesh() {
        let mut doc = Document::default();
        doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        let faces = |doc: &Document, name: &str| {
            let path = scratch(name);
            export_mesh_file(doc, None, &path, MeshFormat::Stl).unwrap();
            let mut back = Document::default();
            import_mesh_file(&mut back, &path, MeshFormat::Stl).unwrap();
            std::fs::remove_file(&path).ok();
            back.state().objects[0].shape.mesh().unwrap().face_count()
        };
        let before = faces(&doc, "before.stl");
        doc.remember("mesh.density", 0.9);
        assert!(faces(&doc, "after.stl") > before);
    }

    #[test]
    fn a_mesh_file_opens_as_a_new_unsaved_model() {
        let mut doc = Document::default();
        doc.add_body(Body::cuboid([0.0; 3], [10.0; 3]).unwrap());
        let path = scratch("opened.obj");
        export_mesh_file(&doc, None, &path, MeshFormat::Obj).unwrap();
        let (opened, summary) = open_mesh_file(&path, MeshFormat::Obj).unwrap();
        assert_eq!(summary, "1 meshes");
        assert_eq!(opened.state().objects.len(), 1);
        assert_eq!(opened.path(), Some(path.with_extension("curva").as_path()));
        std::fs::remove_file(&path).ok();
    }
}
