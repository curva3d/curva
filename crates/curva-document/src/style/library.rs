//! The user's own styles ("My styles", `docs/specs/views-and-styles.md` §2.4): one plain-text
//! `.curvastyle` file per style in a folder, `Curva/styles/` in the user's app data. A style's
//! name is the one written inside its file, so renaming a file changes nothing; the file is named
//! after the style when it is saved. A file that cannot be read is skipped, never fatal.

use super::kept::{StyleError, built_in_name};
use super::{Problem, ViewStyle};
use std::fmt;
use std::path::{Path, PathBuf};

/// The ending of a style file.
pub const EXTENSION: &str = "curvastyle";

/// A folder of style files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    folder: PathBuf,
}

/// A style read from the library, with where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryStyle {
    /// The style, with any bad setting at its default.
    pub style: ViewStyle,
    /// The lines of its file that could not be used.
    pub problems: Vec<Problem>,
    /// Its file.
    pub path: PathBuf,
}

/// Why the library could not save or delete a style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryError {
    /// The style's name cannot be saved: empty, or a built-in style's.
    Name(StyleError),
    /// The file could not be written or removed.
    File(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => error.fmt(f),
            Self::File(error) => write!(f, "the style file could not be written: {error}"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl Library {
    /// The library in `folder`, which need not exist yet: saving makes it.
    #[must_use]
    pub fn at(folder: impl Into<PathBuf>) -> Self {
        Self {
            folder: folder.into(),
        }
    }

    /// The folder the style files are in.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Every style in the library, by name. Files that cannot be read are left out; a missing
    /// folder is an empty library.
    #[must_use]
    pub fn list(&self) -> Vec<LibraryStyle> {
        let Ok(entries) = std::fs::read_dir(&self.folder) else {
            return Vec::new();
        };
        let mut styles: Vec<LibraryStyle> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|ending| ending.eq_ignore_ascii_case(EXTENSION))
            })
            .filter_map(|path| read(&path))
            .collect();
        styles.sort_by(|a, b| {
            a.style
                .name
                .to_lowercase()
                .cmp(&b.style.name.to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        styles
    }

    /// The library's style called `name`, ignoring case.
    #[must_use]
    pub fn load(&self, name: &str) -> Option<LibraryStyle> {
        self.list()
            .into_iter()
            .find(|held| held.style.name.trim().eq_ignore_ascii_case(name.trim()))
    }

    /// Saves `style` in the library, replacing the library's style of that name. Returns the
    /// file it is in. Built-in styles' names are refused.
    pub fn save(&self, style: &ViewStyle) -> Result<PathBuf, LibraryError> {
        let name = style.name.trim();
        if name.is_empty() {
            return Err(LibraryError::Name(StyleError::NoName));
        }
        if let Some(built) = built_in_name(name) {
            return Err(LibraryError::Name(StyleError::BuiltIn(built.to_owned())));
        }
        let failed = |error: std::io::Error| LibraryError::File(error.to_string());
        std::fs::create_dir_all(&self.folder).map_err(failed)?;
        let path = match self.load(name) {
            Some(held) => held.path,
            None => self.free_path(name),
        };
        let mut style = style.clone();
        name.clone_into(&mut style.name);
        // Written beside and renamed, so a half-written file never replaces a whole one.
        let partial = path.with_extension(format!("{EXTENSION}.partial"));
        std::fs::write(&partial, style.to_text()).map_err(failed)?;
        std::fs::rename(&partial, &path).map_err(failed)?;
        Ok(path)
    }

    /// Removes the library's style called `name`. Returns false if it had none of that name.
    pub fn delete(&self, name: &str) -> Result<bool, LibraryError> {
        match self.load(name) {
            Some(held) => std::fs::remove_file(&held.path)
                .map(|()| true)
                .map_err(|error| LibraryError::File(error.to_string())),
            None => Ok(false),
        }
    }

    /// A file for a new style called `name` that no other file has taken.
    fn free_path(&self, name: &str) -> PathBuf {
        let stem = file_stem(name);
        (1..)
            .map(|n| {
                let stem = if n == 1 {
                    stem.clone()
                } else {
                    format!("{stem} {n}")
                };
                self.folder.join(format!("{stem}.{EXTENSION}"))
            })
            .find(|path| !path.exists())
            .unwrap_or_else(|| self.folder.join(format!("style.{EXTENSION}")))
    }
}

/// The style in the file at `path`, if it can be read.
fn read(path: &Path) -> Option<LibraryStyle> {
    let text = std::fs::read_to_string(path).ok()?;
    let read = ViewStyle::from_text(&text);
    Some(LibraryStyle {
        style: read.style,
        problems: read.problems,
        path: path.to_owned(),
    })
}

/// A file name for a style called `name` on every system: characters files cannot have become
/// `_`, and names Windows keeps for devices get a `_` in front.
fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_owned();
    let device = ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|kept| cleaned.eq_ignore_ascii_case(kept))
        || ["COM", "LPT"].iter().any(|kept| {
            cleaned.len() == 4
                && cleaned
                    .get(..3)
                    .is_some_and(|start| start.eq_ignore_ascii_case(kept))
                && cleaned
                    .get(3..)
                    .is_some_and(|end| end.chars().all(|c| c.is_ascii_digit()))
        });
    match (cleaned.is_empty(), device) {
        (true, _) => "style".to_owned(),
        (false, true) => format!("_{cleaned}"),
        (false, false) => cleaned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_on_every_system() {
        assert_eq!(file_stem("Plan / 1:100"), "Plan _ 1_100");
        assert_eq!(file_stem("Con"), "_Con");
        assert_eq!(file_stem("com1"), "_com1");
        assert_eq!(file_stem("Comic"), "Comic");
        assert_eq!(file_stem("  ...  "), "style");
        assert_eq!(file_stem("Diagram."), "Diagram");
        assert_eq!(file_stem("a€"), "a€");
    }
}
