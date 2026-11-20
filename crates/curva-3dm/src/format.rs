//! Rhino's `.3dm` files on Curva's list of formats (spec `docs/specs/file-formats.md` §3): opened
//! as a new model, imported into the open one, saved beside the model with `SaveAs`, and exported
//! whole or only the chosen objects. Moved unchanged from the file commands.

use crate::import::Report;
use curva_document::Document;
use curva_document::formats::{Done, Format};
use std::path::{Path, PathBuf};

/// Rhino's files, as the list holds them.
#[must_use]
pub fn format() -> Format {
    Format {
        open: Some(open),
        import: Some(import),
        save: Some(save),
        export: Some(export),
        ..Format::new("files.rhino", &["3dm"])
    }
}

/// A `.3dm` file as a new model, unsaved, named after the file: it saves as a `.curva` beside
/// it unless one is there already, and never over the `.3dm`.
///
/// # Errors
/// When the file cannot be read or is not a `.3dm` file.
pub fn open_file(path: &Path) -> Result<(Document, Report), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let model = crate::read::read(&data).map_err(|e| e.to_string())?;
    let (mut doc, report) = crate::import::open(&model);
    let beside = path.with_extension("curva");
    doc.set_path((!beside.exists()).then_some(beside));
    Ok((doc, report))
}

/// The model, or the objects in `only`, written to a `.3dm` file.
///
/// # Errors
/// When the file cannot be written.
pub fn write_file(doc: &Document, only: Option<&[u64]>, path: &Path) -> Result<Report, String> {
    let (model, report) = crate::export::export_document(doc, only);
    let bytes = crate::write::write(&model);
    // A half-written file never replaces a whole one, as with `.curva` files.
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    Ok(report)
}

/// A Curva file's contents added to the open model the way a `.3dm` file's are, as plain
/// geometry, its layers merged by name: `Import` for `.curva` files.
///
/// # Errors
/// When the file cannot be read.
pub fn import_curva(doc: &mut Document, path: &Path, shown: &str) -> Result<Done, String> {
    let other = Document::open(path).map_err(|e| e.to_string())?;
    let model = crate::export::export_document(&other, None).0;
    Ok(imported(&crate::import::import(&model, doc), shown))
}

fn imported(report: &Report, shown: &str) -> Done {
    Done {
        key: "done.import",
        args: vec![shown.to_owned(), report.summary()],
    }
}

/// What was left out and the notes, as ` (…)` after a message, or nothing.
fn left_out(report: &Report) -> String {
    let mut notes: Vec<String> = report
        .skipped
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
    notes.extend(report.notes.iter().cloned());
    if notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", notes.join("; "))
    }
}

fn open(path: &Path, shown: &str) -> Result<(Document, Done), String> {
    let (doc, report) = open_file(path)?;
    let done = Done {
        key: "done.open_3dm",
        args: vec![shown.to_owned(), report.summary()],
    };
    Ok((doc, done))
}

fn import(doc: &mut Document, path: &Path, shown: &str) -> Result<Done, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let model = crate::read::read(&data).map_err(|e| e.to_string())?;
    Ok(imported(&crate::import::import(&model, doc), shown))
}

/// A Rhino file written beside the model, which keeps saving as Curva.
fn save(doc: &mut Document, path: &Path, shown: &str) -> Result<Done, String> {
    let report = write_file(doc, None, path)?;
    Ok(Done {
        key: "done.save_3dm",
        args: vec![shown.to_owned(), left_out(&report)],
    })
}

/// What was written, for the command bar: the count of objects and anything left out.
fn export(doc: &Document, only: Option<&[u64]>, path: &Path, shown: &str) -> Result<Done, String> {
    let report = write_file(doc, only, path)?;
    let objects: usize = report.brought.iter().map(|(_, n)| n).sum();
    Ok(Done {
        key: "done.export",
        args: vec![objects.to_string(), shown.to_owned(), left_out(&report)],
    })
}
