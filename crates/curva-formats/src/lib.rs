//! Every file format Curva reads and writes, gathered into one list (spec
//! `docs/specs/file-formats.md` §3). The interface and the agent server hand this list to the
//! model they work on; the file commands then ask the model's list and name no format themselves.

use curva_document::Document;
use curva_document::formats::{self, Formats};
use std::sync::Arc;

/// The full list, in the order file dialogs show it: Curva's own files first.
#[must_use]
pub fn all() -> Arc<Formats> {
    let mut list = Formats::none();
    list.add(formats::Format {
        import: Some(curva_3dm::format::import_curva),
        ..formats::curva()
    });
    list.add(curva_3dm::format::format());
    Arc::new(list)
}

/// An empty model that knows every format, as Curva starts with.
#[must_use]
pub fn new_document() -> Document {
    let mut doc = Document::default();
    doc.set_formats(all());
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curva_files_come_first_then_rhino() {
        let names: Vec<_> = all().all().iter().map(|f| f.name).collect();
        assert_eq!(names, ["files.curva", "files.rhino"]);
    }
}
