//! Mesh files Curva reads and writes in its own Rust (spec `docs/specs/mesh-and-subd.md` §5):
//! STL, text and binary; OBJ; and PLY, text and binary. They are simple enough that no outside
//! library is needed.
//!
//! None of them records units: numbers are read and written as they are, and the command that
//! opens or saves the file says which unit they are in.

pub mod obj;
pub mod ply;
pub mod stl;

use curva_mesh::Mesh;
use std::fmt;

/// A mesh read from a file, with the name the file gave it (empty when it gave none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Named {
    /// The object's name in the file.
    pub name: String,
    /// Its mesh.
    pub mesh: Mesh,
}

/// What a file held: its meshes, and what was left out, by kind, for the command bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contents {
    /// The meshes, in file order.
    pub meshes: Vec<Named>,
    /// Things left out, by kind, with how many.
    pub skipped: Vec<(String, usize)>,
}

impl Contents {
    fn skip(&mut self, kind: &str, n: usize) {
        if n == 0 {
            return;
        }
        match self.skipped.iter_mut().find(|(k, _)| k == kind) {
            Some((_, count)) => *count += n,
            None => self.skipped.push((kind.to_owned(), n)),
        }
    }
}

/// Why a file could not be read. The text is for logs; the command bar says it in its own words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    /// The line it went wrong on, for text files.
    pub line: Option<usize>,
    /// What went wrong.
    pub problem: String,
}

impl ReadError {
    fn new(problem: impl Into<String>) -> Self {
        Self {
            line: None,
            problem: problem.into(),
        }
    }

    fn at(line: usize, problem: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            problem: problem.into(),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.problem),
            None => f.write_str(&self.problem),
        }
    }
}

impl std::error::Error for ReadError {}
