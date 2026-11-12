//! Autosave and recovery, as in Rhino: every few minutes, and only if the model changed, a copy
//! goes to a recovery file in the user's app-data folder, never over the user's own file. A
//! normal close deletes it, so a recovery file found at start means Curva did not close
//! normally, and the user is offered it back.

use curva_document::{Document, State};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How often a changed model is saved to the recovery file.
pub const INTERVAL: Duration = Duration::from_secs(5 * 60);
const EXTENSION: &str = "curva";
/// Beside each recovery file, the file the model was saved to, if any, so a restored model
/// saves back to it.
const SOURCE_EXTENSION: &str = "source";

/// The folder recovery files go in: the user's app-data folder for Curva.
#[must_use]
pub fn folder() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::home_dir().map(|home| home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(".local").join("share")))
    };
    Some(base?.join("Curva").join("recovery"))
}

/// Keeps one running Curva's recovery file up to date.
pub struct Autosave {
    file: PathBuf,
    /// The model as last written, or as it started, so an unchanged model is not written.
    written: State,
    last: Instant,
}

impl Autosave {
    /// Autosave into `folder`, under a name of this process's own so that two Curvas running at
    /// once do not overwrite each other. `doc` is the model as it starts.
    #[must_use]
    pub fn new(folder: &Path, doc: &Document, now: Instant) -> Self {
        Self {
            file: folder.join(format!("autosave-{}.{EXTENSION}", std::process::id())),
            written: doc.state().clone(),
            last: now,
        }
    }

    /// Writes the recovery file if the interval has passed and the model changed since it was
    /// last written. Returns whether it wrote.
    pub fn tick(&mut self, doc: &Document, now: Instant) -> bool {
        if now.duration_since(self.last) < INTERVAL {
            return false;
        }
        self.last = now;
        if doc.state() == &self.written {
            return false;
        }
        if let Some(folder) = self.file.parent() {
            // A folder that cannot be made leaves the write below to fail, which is reported
            // as not written; autosave never stops the user working.
            let _ = std::fs::create_dir_all(folder);
        }
        if doc.write_copy(&self.file).is_err() {
            return false;
        }
        let source = self.file.with_extension(SOURCE_EXTENSION);
        let _ = match doc.path() {
            Some(path) => std::fs::write(&source, path.to_string_lossy().as_bytes()),
            None => std::fs::remove_file(&source),
        };
        self.written = doc.state().clone();
        true
    }

    /// Deletes the recovery file, on a normal close.
    pub fn finish(&self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_file(self.file.with_extension(SOURCE_EXTENSION));
    }
}

/// A recovery file left by a Curva that did not close normally: the newest, if several.
#[must_use]
pub fn left_behind(folder: &Path) -> Option<PathBuf> {
    std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| Some((std::fs::metadata(&path).ok()?.modified().ok()?, path)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// Opens a recovery file as the model it was made from, saving back to that model's own file,
/// then deletes it; the running Curva autosaves the model afresh under its own name.
pub fn restore(file: &Path) -> Result<Document, String> {
    let mut doc = Document::open(file).map_err(|error| error.0)?;
    let source = file.with_extension(SOURCE_EXTENSION);
    let original = std::fs::read_to_string(&source).ok().map(PathBuf::from);
    doc.set_path(original);
    discard(file);
    Ok(doc)
}

/// Deletes a recovery file the user chose not to restore.
pub fn discard(file: &Path) {
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_file(file.with_extension(SOURCE_EXTENSION));
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("curva-recovery-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn writes_only_after_the_interval_and_only_when_changed() {
        let folder = scratch("tick");
        let mut doc = Document::default();
        let start = Instant::now();
        let mut autosave = Autosave::new(&folder, &doc, start);
        // Unchanged: nothing written even when the interval has passed.
        assert!(!autosave.tick(&doc, start + INTERVAL));
        assert!(left_behind(&folder).is_none());
        doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        // Changed, but too soon.
        assert!(!autosave.tick(&doc, start + INTERVAL + Duration::from_secs(1)));
        // Changed, and the interval has passed again.
        assert!(autosave.tick(&doc, start + INTERVAL * 2 + Duration::from_secs(1)));
        assert!(left_behind(&folder).is_some());
        autosave.finish();
        assert!(left_behind(&folder).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn restoring_brings_the_same_objects_back_saving_to_the_original_file() {
        let folder = scratch("restore");
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [3.0, 0.0, 0.0]).unwrap());
        let original = folder.join("model.curva");
        std::fs::create_dir_all(&folder).unwrap();
        doc.save(&original).unwrap();
        doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        let start = Instant::now();
        let mut autosave = Autosave::new(&folder.join("recovery"), &Document::default(), start);
        assert!(autosave.tick(&doc, start + INTERVAL));
        let found = left_behind(&folder.join("recovery")).unwrap();
        let restored = restore(&found).unwrap();
        assert_eq!(restored.state().objects, doc.state().objects);
        assert_eq!(restored.path(), Some(original.as_path()));
        assert!(left_behind(&folder.join("recovery")).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
