//! The list of file formats Curva reads and writes (spec `docs/specs/file-formats.md`). Each
//! format describes itself once and joins the list; the file commands ask the list rather than
//! knowing every format. The list travels with the open model and is never saved.

use crate::Document;
use std::path::Path;
use std::sync::Arc;

/// What a format reports when its work is done: the message's key in `locales/en.txt` and its
/// values, so each format keeps the wording it has always had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
    /// The message's key, such as `done.open_3dm`.
    pub key: &'static str,
    /// The values the message shows, in order.
    pub args: Vec<String>,
}

/// Reads a file into a new model. Given the file and how to name it in the message.
pub type Opener = fn(&Path, &str) -> Result<(Document, Done), String>;
/// Reads a file into the open model.
pub type Importer = fn(&mut Document, &Path, &str) -> Result<Done, String>;
/// Writes the whole model to a file, for `Save` and `SaveAs`.
pub type Saver = fn(&mut Document, &Path, &str) -> Result<Done, String>;
/// Writes the model, or only the objects given, to a file, for `Export` and `ExportSelected`.
pub type Exporter = fn(&Document, Option<&[u64]>, &Path, &str) -> Result<Done, String>;
/// Writes pages the viewport has drawn from layouts to a file.
pub type PageWriter = fn(&[crate::drawing::Page], &Path, &str) -> Result<Done, String>;

/// One kind of file and what Curva can do with it.
#[derive(Clone, Debug)]
pub struct Format {
    /// The key of its name for file dialogs, such as `files.rhino`.
    pub name: &'static str,
    /// Its file extensions, the usual one first, without the dot.
    pub extensions: &'static [&'static str],
    /// Reading a file as a new model, for `Open`.
    pub open: Option<Opener>,
    /// Reading a file into the open model, for `Import`.
    pub import: Option<Importer>,
    /// Writing the model, for `Save` and `SaveAs`.
    pub save: Option<Saver>,
    /// Writing the model or the chosen objects, for `Export` and `ExportSelected`.
    pub export: Option<Exporter>,
    /// Writing printed pages, for `Print` and `ExportPDF` (spec `drawing-output.md` §1).
    pub print: Option<PageWriter>,
    /// Writing layouts as drawings, as `Export` offers them: the open layout, or every one.
    pub drawing: Option<PageWriter>,
    /// Whether `Import`'s dialog lists it when it can be imported. Only SketchUp says no: it has
    /// always been read when its file is typed in, but never offered there.
    pub import_listed: bool,
}

impl Format {
    /// A format that can do nothing yet, to fill in.
    #[must_use]
    pub fn new(name: &'static str, extensions: &'static [&'static str]) -> Self {
        Self {
            name,
            extensions,
            open: None,
            import: None,
            save: None,
            export: None,
            print: None,
            drawing: None,
            import_listed: true,
        }
    }

    /// Its name and extensions, as a file dialog lists them.
    #[must_use]
    pub fn listed(&self) -> (&'static str, &'static [&'static str]) {
        (self.name, self.extensions)
    }

    /// Whether a file's location ends in one of its extensions, in any case.
    #[must_use]
    pub fn matches(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| {
            self.extensions
                .iter()
                .any(|ours| e.eq_ignore_ascii_case(ours))
        })
    }
}

/// The formats Curva knows, in the order file dialogs list them.
#[derive(Clone, Debug)]
pub struct Formats(Vec<Format>);

impl Default for Formats {
    /// Curva's own files, which every model knows.
    fn default() -> Self {
        Self(vec![curva()])
    }
}

impl Formats {
    /// No formats at all.
    #[must_use]
    pub fn none() -> Self {
        Self(Vec::new())
    }

    /// Adds a format at the end of the list.
    pub fn add(&mut self, format: Format) {
        self.0.push(format);
    }

    /// Every format, in order.
    #[must_use]
    pub fn all(&self) -> &[Format] {
        &self.0
    }

    /// The format a file is, by its extension, in any case.
    #[must_use]
    pub fn of(&self, path: &Path) -> Option<&Format> {
        self.0.iter().find(|f| f.matches(path))
    }

    /// The formats that can do something, as a file dialog lists them.
    fn listed(
        &self,
        can: impl Fn(&Format) -> bool,
    ) -> Vec<(&'static str, &'static [&'static str])> {
        self.0
            .iter()
            .filter(|f| can(f))
            .map(Format::listed)
            .collect()
    }

    /// What `Open` offers.
    #[must_use]
    pub fn opening(&self) -> Vec<(&'static str, &'static [&'static str])> {
        self.listed(|f| f.open.is_some())
    }

    /// What `Import` offers.
    #[must_use]
    pub fn importing(&self) -> Vec<(&'static str, &'static [&'static str])> {
        self.listed(|f| f.import.is_some() && f.import_listed)
    }

    /// What `Save` and `SaveAs` offer.
    #[must_use]
    pub fn saving(&self) -> Vec<(&'static str, &'static [&'static str])> {
        self.listed(|f| f.save.is_some())
    }

    /// What `Export` and `ExportSelected` offer.
    #[must_use]
    pub fn exporting(&self) -> Vec<(&'static str, &'static [&'static str])> {
        self.listed(|f| f.export.is_some() || f.drawing.is_some())
    }

    /// What `Print` and `ExportPDF` write to a file.
    #[must_use]
    pub fn printing(&self) -> Vec<(&'static str, &'static [&'static str])> {
        self.listed(|f| f.print.is_some())
    }
}

/// Curva's own `.curva` files: opened as a new model, and saved as the model's own file.
#[must_use]
pub fn curva() -> Format {
    Format {
        open: Some(open_curva),
        save: Some(save_curva),
        ..Format::new("files.curva", &["curva"])
    }
}

fn open_curva(path: &Path, shown: &str) -> Result<(Document, Done), String> {
    let opened = Document::open(path).map_err(|e| e.to_string())?;
    let done = Done {
        key: "done.open",
        args: vec![shown.to_owned()],
    };
    Ok((opened, done))
}

fn save_curva(doc: &mut Document, path: &Path, shown: &str) -> Result<Done, String> {
    doc.save(path).map_err(|e| e.to_string())?;
    Ok(Done {
        key: "done.save",
        args: vec![shown.to_owned()],
    })
}

impl Document {
    /// The formats the file commands offer for this model.
    #[must_use]
    pub fn formats(&self) -> &Arc<Formats> {
        &self.formats
    }

    /// Sets the formats, as the interface does when it starts.
    pub fn set_formats(&mut self, formats: Arc<Formats>) {
        self.formats = formats;
    }

    /// Replaces this model with `other`, which keeps this one's formats, as `Open` and `New` do.
    pub fn replace_with(&mut self, mut other: Document) {
        other.formats = Arc::clone(&self.formats);
        *self = other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rhino() -> Format {
        Format {
            open: Some(|_, _| Err("not here".to_owned())),
            ..Format::new("files.rhino", &["3dm"])
        }
    }

    #[test]
    fn a_format_is_found_by_its_extension_in_any_case() {
        let mut formats = Formats::default();
        formats.add(rhino());
        assert_eq!(
            formats.of(Path::new("a/Plan.3DM")).map(|f| f.name),
            Some("files.rhino")
        );
        assert_eq!(
            formats.of(Path::new("plan.CURVA")).map(|f| f.name),
            Some("files.curva")
        );
        assert!(formats.of(Path::new("plan.dxf")).is_none());
        assert!(formats.of(Path::new("plan")).is_none());
    }

    #[test]
    fn a_format_that_cannot_export_is_not_offered_for_export() {
        let mut formats = Formats::default();
        formats.add(rhino());
        assert_eq!(
            formats.opening(),
            vec![
                ("files.curva", &["curva"][..]),
                ("files.rhino", &["3dm"][..])
            ]
        );
        assert!(formats.exporting().is_empty());
        assert_eq!(formats.saving(), vec![("files.curva", &["curva"][..])]);
    }

    #[test]
    fn a_model_that_replaces_another_keeps_its_formats() {
        let mut formats = Formats::default();
        formats.add(rhino());
        let mut doc = Document::default();
        doc.set_formats(Arc::new(formats));
        doc.replace_with(Document::default());
        assert_eq!(doc.formats().all().len(), 2);
    }

    #[test]
    fn a_curva_file_opens_through_the_list() {
        let folder = std::env::temp_dir().join(format!("curva-formats-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("model.curva");
        let mut doc = Document::default();
        let formats = Arc::clone(doc.formats());
        let save = formats.of(&file).and_then(|f| f.save).unwrap();
        assert_eq!(save(&mut doc, &file, "model").unwrap().key, "done.save");
        assert_eq!(doc.path(), Some(file.as_path()));
        let open = formats.of(&file).and_then(|f| f.open).unwrap();
        let (opened, done) = open(&file, "model").unwrap();
        assert_eq!(done.args, vec!["model".to_owned()]);
        assert_eq!(opened.path(), Some(file.as_path()));
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
