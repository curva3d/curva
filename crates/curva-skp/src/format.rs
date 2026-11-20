//! SketchUp's `.skp` files on Curva's list of formats (spec `docs/specs/file-formats.md` §3):
//! read only, opened as a new model or imported into the open one. Moved unchanged from the file
//! commands.

use curva_document::Document;
use curva_document::formats::{Done, Format};
use std::path::Path;

/// SketchUp's models, as the list holds them.
#[must_use]
pub fn format() -> Format {
    Format {
        open: Some(open),
        import: Some(import),
        // As before: `Import` reads a `.skp` typed in, but its dialog does not offer one.
        import_listed: false,
        ..Format::new("files.skp", &["skp"])
    }
}

/// A SketchUp file's model, read.
fn read_file(path: &Path) -> Result<crate::Model, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    crate::read(&data)
}

/// A `.skp` file as a new model, unsaved, named after the file, as `.3dm` files open.
fn open(path: &Path, shown: &str) -> Result<(Document, Done), String> {
    let (mut doc, report) = crate::open(&read_file(path)?);
    let beside = path.with_extension("curva");
    doc.set_path((!beside.exists()).then_some(beside));
    let done = Done {
        key: "done.open_3dm",
        args: vec![shown.to_owned(), report.summary()],
    };
    Ok((doc, done))
}

fn import(doc: &mut Document, path: &Path, shown: &str) -> Result<Done, String> {
    let report = crate::import(&read_file(path)?, doc);
    Ok(Done {
        key: "done.import",
        args: vec![shown.to_owned(), report.summary()],
    })
}
