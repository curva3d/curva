//! `Export` and `ExportSelected` to STEP (`.stp`, `.step`): surfaces, polysurfaces and solids kept
//! exact, through the kernel's STEP writer (spec `docs/specs/export-formats.md`, step 1). Curves,
//! block copies, annotation, meshes and SubDs are left out and counted (meshes as faceted solids
//! come later, spec §8 step 6).

use curva_document::{Document, Shape};
use curva_kernel::step::{StepPart, StepSchema, write_step};
use std::path::Path;

/// Whether a path names a STEP file.
pub fn is_step(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("stp") || e.eq_ignore_ascii_case("step"))
}

/// The model's surfaces and solids, or those among the objects in `only`, written to a STEP file
/// in `schema`, through a partial file renamed into place. Returns a summary for the command line:
/// how many were written and how many left out.
///
/// # Errors
/// When nothing could be written, or the file cannot be saved.
pub fn write_step_file(
    doc: &Document,
    only: Option<&[u64]>,
    path: &Path,
    schema: StepSchema,
) -> Result<String, String> {
    let mut parts = Vec::new();
    let mut other = 0usize;
    for object in &doc.state().objects {
        if only.is_some_and(|ids| !ids.contains(&object.id))
            || (only.is_none() && (object.space.is_some() || object.hidden))
        {
            continue;
        }
        match &object.shape {
            Shape::Body(body) => parts.push(StepPart {
                body,
                name: (!object.name.is_empty()).then_some(object.name.as_str()),
            }),
            Shape::Curve(_)
            | Shape::Instance(_)
            | Shape::Annotation(_)
            | Shape::Mesh(_)
            | Shape::SubD(_) => other += 1,
        }
    }
    if parts.is_empty() {
        return Err("there are no surfaces or solids to write".to_owned());
    }
    let name = path.file_name().and_then(|n| n.to_str());
    let file = write_step(&parts, schema, doc.state().unit.millimetres(), name)
        .map_err(|e| e.to_string())?;
    let partial = path.with_extension("partial");
    std::fs::write(&partial, file.text.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    let left_out = other + file.left_out.len();
    Ok(if left_out == 0 {
        format!("{} surfaces and solids", file.written)
    } else {
        format!(
            "{} surfaces and solids; {left_out} other objects left out",
            file.written
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Body;

    #[test]
    fn step_files_are_known_by_their_extension() {
        assert!(is_step(Path::new("part.STP")));
        assert!(is_step(Path::new("part.step")));
        assert!(!is_step(Path::new("part.stl")));
    }

    #[test]
    fn a_model_and_a_selection_are_written() {
        let mut doc = Document::default();
        let a = doc.add_body(Body::cuboid([0.0; 3], [10.0; 3]).unwrap());
        doc.add_body(Body::sphere([30.0, 0.0, 0.0], 5.0).unwrap());
        let dir = std::env::temp_dir().join(format!("curva-step-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let all = dir.join("all.stp");
        let summary = write_step_file(&doc, None, &all, StepSchema::Ap214).unwrap();
        assert_eq!(summary, "2 surfaces and solids");
        let text = std::fs::read_to_string(&all).unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 2);
        let one = dir.join("one.step");
        write_step_file(&doc, Some(&[a]), &one, StepSchema::Ap214).unwrap();
        let text = std::fs::read_to_string(&one).unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
