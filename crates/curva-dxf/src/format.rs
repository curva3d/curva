//! AutoCAD's `.dxf` and `.dwg` files on Curva's list of formats (spec
//! `docs/specs/file-formats.md` §3): opened as a new model, imported into the open one, and
//! exported whole or only the chosen objects. Moved unchanged from the file commands.

use curva_document::Document;
use curva_document::formats::{Done, Format as Listed};
use std::path::Path;

/// Which of the two files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// AutoCAD's text exchange file.
    Dxf,
    /// AutoCAD's own drawing file.
    Dwg,
}

impl Format {
    /// The format a file name's extension names, if either.
    #[must_use]
    pub fn of(path: &std::path::Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "dxf" => Some(Self::Dxf),
            "dwg" => Some(Self::Dwg),
            _ => None,
        }
    }
}

/// `.dxf` files, as the list holds them.
#[must_use]
pub fn dxf() -> Listed {
    Listed {
        open: Some(|path, shown, _| open(path, shown, Format::Dxf)),
        import: Some(|doc, path, shown| import(doc, path, shown, Format::Dxf)),
        export: Some(|doc, only, path, shown| export(doc, only, path, shown, Format::Dxf)),
        ..Listed::new("files.dxf", &["dxf"])
    }
}

/// `.dwg` files, as the list holds them.
#[must_use]
pub fn dwg() -> Listed {
    Listed {
        open: Some(|path, shown, _| open(path, shown, Format::Dwg)),
        import: Some(|doc, path, shown| import(doc, path, shown, Format::Dwg)),
        export: Some(|doc, only, path, shown| export(doc, only, path, shown, Format::Dwg)),
        ..Listed::new("files.dwg", &["dwg"])
    }
}

/// A `.dxf` or `.dwg` file as a new model, unsaved, named after the file, as `.3dm` files open.
fn open(path: &Path, shown: &str, format: Format) -> Result<(Document, Done), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let cad = crate::read(&data, format)?;
    let mut doc = Document::default();
    let report = crate::import(&cad, &mut doc);
    let beside = path.with_extension("curva");
    doc.set_path((!beside.exists()).then_some(beside));
    let done = Done {
        key: "done.open_3dm",
        args: vec![shown.to_owned(), report.summary()],
    };
    Ok((doc, done))
}

fn import(doc: &mut Document, path: &Path, shown: &str, format: Format) -> Result<Done, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let cad = crate::read(&data, format)?;
    let report = crate::import(&cad, doc);
    Ok(Done {
        key: "done.import",
        args: vec![shown.to_owned(), report.summary()],
    })
}

/// The model, or the objects in `only`, written to a `.dxf` or `.dwg` file, through a partial
/// file so a failed write never leaves half a drawing.
fn export(
    doc: &Document,
    only: Option<&[u64]>,
    path: &Path,
    shown: &str,
    format: Format,
) -> Result<Done, String> {
    let (bytes, report) = crate::export(doc, only, format)?;
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    Ok(Done {
        key: "done.export_cad",
        args: vec![shown.to_owned(), report.summary()],
    })
}
