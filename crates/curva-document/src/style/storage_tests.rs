//! A model's own styles and the user's library of style files.

use super::*;
use crate::Document;

/// A copy of Shaded called `name`.
fn named(name: &str) -> ViewStyle {
    ViewStyle {
        name: name.to_owned(),
        ..ViewStyle::shaded()
    }
}

/// An empty folder of its own for test `test`.
fn folder(test: &str) -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("curva-styles-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

#[test]
fn built_in_styles_are_only_copied_and_names_ignore_case() {
    let mut doc = Document::default();
    assert_eq!(
        doc.save_style(named("shaded")),
        Err(StyleError::BuiltIn("Shaded".to_owned()))
    );
    assert_eq!(doc.save_style(named("  ")), Err(StyleError::NoName));
    doc.save_style(named(" Diagram ")).unwrap();
    let mut darker = named("DIAGRAM");
    darker.nurbs.faces.opacity = 0.5;
    doc.save_style(darker).unwrap();
    assert_eq!(doc.styles().len(), 1, "the same name replaces");
    let kept = doc.model_style("diagram").unwrap();
    assert_eq!(kept.name, "DIAGRAM");
    assert!((kept.nurbs.faces.opacity - 0.5).abs() < 1e-6);
    assert!(doc.delete_style("Diagram"));
    assert!(!doc.delete_style("Diagram"));
    assert!(doc.styles().is_empty());
}

#[test]
fn a_view_finds_the_model_s_style_or_else_the_built_in_one() {
    let mut doc = Document::default();
    doc.save_style(named("Diagram")).unwrap();
    assert_eq!(doc.style_named("diagram").unwrap().name, "Diagram");
    assert_eq!(doc.style_named("ghosted").unwrap().name, "Ghosted");
    assert!(doc.style_named("Nowhere").is_none());
}

#[test]
fn renaming_keeps_names_unique_and_built_ins_untouched() {
    let mut doc = Document::default();
    doc.save_style(named("Diagram")).unwrap();
    doc.save_style(named("Plan")).unwrap();
    assert_eq!(
        doc.rename_style("Diagram", "plan"),
        Err(StyleError::Taken("plan".to_owned()))
    );
    assert_eq!(
        doc.rename_style("Diagram", "Hidden"),
        Err(StyleError::BuiltIn("Hidden".to_owned()))
    );
    assert_eq!(
        doc.rename_style("Nowhere", "Other"),
        Err(StyleError::Missing("Nowhere".to_owned()))
    );
    doc.rename_style("Diagram", "DIAGRAM").unwrap();
    doc.rename_style("DIAGRAM", "Study").unwrap();
    let names: Vec<_> = doc.styles().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Study", "Plan"]);
}

#[test]
fn a_model_s_styles_are_saved_in_its_file_with_settings_it_does_not_know() {
    let folder = folder("model");
    std::fs::create_dir_all(&folder).unwrap();
    let mut doc = Document::default();
    let mut diagram = named("Diagram");
    diagram.engine = Engine::Vector;
    diagram
        .unknown
        .push(("future.glow".to_owned(), "yes".to_owned()));
    doc.save_style(diagram.clone()).unwrap();
    let path = folder.join("styles.curva");
    doc.save(&path).unwrap();
    let opened = Document::open(&path).unwrap();
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(opened.styles(), [diagram]);
}

#[test]
fn a_file_saved_before_styles_opens_with_none() {
    let folder = folder("old");
    std::fs::create_dir_all(&folder).unwrap();
    let doc = Document::default();
    let mut file = serde_json::to_value(crate::FileContents {
        format: crate::FORMAT.to_owned(),
        version: crate::migrate::CURRENT,
        state: doc.state().clone(),
        views: Vec::new(),
        cplanes: Vec::new(),
        styles: vec![named("Gone")],
    })
    .unwrap();
    file.as_object_mut().unwrap().remove("styles");
    let path = folder.join("old.curva");
    std::fs::write(&path, file.to_string()).unwrap();
    let opened = Document::open(&path).unwrap();
    let _ = std::fs::remove_dir_all(&folder);
    assert!(opened.styles().is_empty());
}

#[test]
fn the_library_saves_lists_replaces_and_deletes_style_files() {
    let folder = folder("library");
    let library = Library::at(&folder);
    assert!(library.list().is_empty(), "a missing folder is empty");
    let first = library.save(&named("Diagram")).unwrap();
    assert_eq!(first, folder.join("Diagram.curvastyle"));
    library.save(&named("Plan / 1:100")).unwrap();
    let mut changed = named("diagram");
    changed.engine = Engine::Occlusion;
    assert_eq!(library.save(&changed).unwrap(), first, "the same file");
    // Files that are not style files, and a style file Curva cannot read, are left out.
    std::fs::write(folder.join("notes.txt"), "not a style").unwrap();
    std::fs::write(folder.join("broken.curvastyle"), [0xFF, 0xFE, 0x00]).unwrap();
    let names: Vec<_> = library
        .list()
        .into_iter()
        .map(|held| held.style.name)
        .collect();
    assert_eq!(names, ["diagram", "Plan / 1:100"]);
    assert_eq!(
        library.load("DIAGRAM").unwrap().style.engine,
        Engine::Occlusion
    );
    assert!(library.delete("Diagram").unwrap());
    assert!(!library.delete("Diagram").unwrap());
    assert!(library.load("Diagram").is_none());
    assert!(matches!(
        library.save(&named("Wireframe")),
        Err(LibraryError::Name(StyleError::BuiltIn(_)))
    ));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn styles_whose_file_names_clash_get_files_of_their_own() {
    let folder = folder("clash");
    let library = Library::at(&folder);
    let a = library.save(&named("A/B")).unwrap();
    let b = library.save(&named("A:B")).unwrap();
    assert_ne!(a, b);
    assert_eq!(library.list().len(), 2);
    let _ = std::fs::remove_dir_all(&folder);
}
