//! Every source file in Curva's crates under 800 lines and every function under 80, except those
//! already over on 19 November 2026, listed in `known.txt`, which may only shrink (issue #400).
//!
//! Test code is not counted: `#[cfg(test)]` and `#[test]` items, and test files (`tests.rs`,
//! `*_tests.rs`, anything under a `tests` folder). Patched copies of outside libraries (`vendor`)
//! are their authors'. Long tables of plain data are constants, not functions, so they are not
//! measured as functions; a file that is mostly data goes in `known.txt` with a note.
//!
//! To print today's list of files and functions over the limits:
//! `CURVA_SIZE_LIST=1 cargo test -p curva-size-check -- --nocapture`

use curva_size_check::{FILE_LIMIT, FUNCTION_LIMIT, measure};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The known exceptions: `file <path> <lines>` or `fn <path> <name> <lines>`.
fn known() -> BTreeMap<String, usize> {
    let text = include_str!("../known.txt");
    let mut known = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let (key, lines) = match parts.as_slice() {
            ["file", path, n] => (format!("file {path}"), n),
            ["fn", path, name, n] => (format!("fn {path} {name}"), n),
            _ => panic!("known.txt: a line that is neither a file nor a function: {line}"),
        };
        let lines = lines
            .parse()
            .unwrap_or_else(|_| panic!("known.txt: not a number of lines: {line}"));
        assert!(
            known.insert(key, lines).is_none(),
            "known.txt: listed twice: {line}"
        );
    }
    known
}

fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || path.components().any(|c| c.as_os_str() == "tests")
}

/// Every Rust source file under each crate's `src`, as a path from the repository's root.
fn sources(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "vendor") {
                    continue;
                }
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    let crates = root.join("crates");
    for entry in std::fs::read_dir(&crates)
        .expect("the crates folder")
        .flatten()
    {
        walk(&entry.path().join("src"), &mut out);
    }
    out.sort();
    out
}

#[test]
fn files_and_functions_stay_within_the_size_limits() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let known = known();
    let mut over: BTreeMap<String, usize> = BTreeMap::new();
    for path in sources(&root) {
        if is_test_file(&path) {
            continue;
        }
        let relative = path
            .strip_prefix(&root)
            .map_or_else(|_| path.clone(), Path::to_path_buf)
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path).expect("a readable source file");
        let sizes = measure(&source);
        if sizes.lines > FILE_LIMIT {
            over.insert(format!("file {relative}"), sizes.lines);
        }
        for (name, lines) in sizes.functions {
            if lines > FUNCTION_LIMIT {
                let key = format!("fn {relative} {name}");
                let longest = over.entry(key).or_default();
                *longest = (*longest).max(lines);
            }
        }
    }
    if std::env::var_os("CURVA_SIZE_LIST").is_some() {
        for (key, lines) in &over {
            println!("{key} {lines}");
        }
    }
    let mut problems = Vec::new();
    for (key, lines) in &over {
        match known.get(key) {
            None => problems.push(format!(
                "{key} is {lines} lines, over the limit ({FILE_LIMIT} for a file, \
                 {FUNCTION_LIMIT} for a function): split it (AGENTS.md, \"Keeping the code tidy\")"
            )),
            Some(&allowed) if *lines > allowed => problems.push(format!(
                "{key} grew from {allowed} to {lines} lines; it is already over the limit, so \
                 split it before adding to it"
            )),
            Some(_) => {}
        }
    }
    for (key, allowed) in &known {
        if !over.contains_key(key) {
            problems.push(format!(
                "{key} (listed at {allowed}) is now within the limit or gone: remove it from \
                 crates/curva-size-check/known.txt"
            ));
        }
    }
    assert!(problems.is_empty(), "\n{}\n", problems.join("\n"));
}
