//! Rhino `.3dm` files at the command line (spec `docs/specs/3dm-files.md` §3): `Import`, `Export`,
//! `ExportSelected`, and the `.3dm` side of `Open` and `SaveAs`.

use super::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_3dm::import::Report;
use curva_document::Document;
use std::path::{Path, PathBuf};

/// Whether a typed path names a `.3dm` file.
pub fn is_3dm(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("3dm"))
}

/// A path as typed, without the quotes a pasted one may have.
fn typed(text: &str) -> PathBuf {
    PathBuf::from(text.trim().trim_matches('"'))
}

fn failed(error: impl std::fmt::Display) -> Step {
    Step::Failed(Message::with("error.file", vec![error.to_string()]))
}

/// A `.3dm` file as a new model, unsaved, named after the file: it saves as a `.curva` beside
/// it unless one is there already, and never over the `.3dm`.
pub fn open_3dm(path: &Path) -> Result<(Document, Report), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let model = curva_3dm::read::read(&data).map_err(|e| e.to_string())?;
    let (mut doc, report) = curva_3dm::import::open(&model);
    let beside = path.with_extension("curva");
    doc.set_path((!beside.exists()).then_some(beside));
    Ok((doc, report))
}

/// The model, or the objects in `only`, written to a `.3dm` file.
pub fn write_3dm(doc: &Document, only: Option<&[u64]>, path: &Path) -> Result<Report, String> {
    let (model, report) = curva_3dm::export::export_document(doc, only);
    let bytes = curva_3dm::write::write(&model);
    // A half-written file never replaces a whole one, as with `.curva` files.
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    Ok(report)
}

/// What was written, for the command bar: the count of objects and anything left out.
fn written(path: &Path, report: &Report) -> Message {
    let objects: usize = report.brought.iter().map(|(_, n)| n).sum();
    let mut notes: Vec<String> = report
        .skipped
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
    notes.extend(report.notes.iter().cloned());
    Message::with(
        "done.export",
        vec![
            objects.to_string(),
            path.display().to_string(),
            if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join("; "))
            },
        ],
    )
}

/// `Import`: a `.3dm` or `.curva` file's contents added to the model, its layers merged by name.
#[derive(Default)]
pub struct Import;

impl Command for Import {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.import.path", Kind::Path)),
            Some(Answer::Path(text)) => {
                let path = typed(&text);
                let model = if is_3dm(&path) {
                    let data = match std::fs::read(&path) {
                        Ok(data) => data,
                        Err(e) => return failed(e),
                    };
                    match curva_3dm::read::read(&data) {
                        Ok(model) => model,
                        Err(e) => return failed(e),
                    }
                } else {
                    // A Curva file comes in the same way, as plain geometry.
                    match Document::open(&path) {
                        Ok(other) => curva_3dm::export::export_document(&other, None).0,
                        Err(e) => return failed(e),
                    }
                };
                let report = curva_3dm::import::import(&model, doc);
                Step::Done(Some(Message::with(
                    "done.import",
                    vec![path.display().to_string(), report.summary()],
                )))
            }
            Some(_) => unexpected(),
        }
    }
}

/// `Export` and `ExportSelected`: the model, or the chosen objects, written as `.3dm`.
#[derive(Default)]
pub struct Export {
    selected: bool,
    chosen: Option<Vec<u64>>,
}

/// `ExportSelected`.
#[derive(Default)]
pub struct ExportSelected(Export);

impl Command for ExportSelected {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        self.0.selected = true;
        self.0.step(doc, answer)
    }
}

impl Command for Export {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Objects(list)) if self.selected && self.chosen.is_none() => {
                if list.is_empty() {
                    return ask_objects("prompt.export.objects");
                }
                self.chosen = Some(list);
                Step::Ask(Request::new("prompt.export.path", Kind::Path))
            }
            None if self.selected => {
                let chosen = doc.selection();
                if chosen.is_empty() {
                    return ask_objects("prompt.export.objects");
                }
                self.chosen = Some(chosen);
                Step::Ask(Request::new("prompt.export.path", Kind::Path))
            }
            None => Step::Ask(Request::new("prompt.export.path", Kind::Path)),
            Some(Answer::Path(text)) => {
                let mut path = typed(&text);
                if path.extension().is_none() {
                    path.set_extension("3dm");
                }
                if !is_3dm(&path) {
                    return Step::Failed(Message::with(
                        "error.export_kind",
                        vec![path.display().to_string()],
                    ));
                }
                match write_3dm(doc, self.chosen.as_deref(), &path) {
                    Ok(report) => Step::Done(Some(written(&path, &report))),
                    Err(e) => failed(e),
                }
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    fn folder(name: &str) -> std::path::PathBuf {
        let f = std::env::temp_dir().join(format!("curva-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&f).unwrap();
        f
    }

    #[test]
    fn export_then_open_gives_the_model_back() {
        let dir = folder("export3dm");
        let file = dir.join("drawing.3dm");
        let typed = file.display().to_string();
        let mut doc = Document::default();
        run(
            &mut doc,
            &[
                "line", "0,0", "5,0", "circle", "0,0", "2", "box", "0,0", "3,3", "2", "export",
                &typed,
            ],
        );
        assert!(file.exists());
        let mut other = Document::default();
        run(&mut other, &["open", &typed]);
        assert_eq!(other.state().objects.len(), doc.state().objects.len());
        // Unsaved, named after the file, and never saving over the .3dm.
        assert_eq!(other.path(), Some(dir.join("drawing.curva").as_path()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_adds_to_the_model_and_undo_takes_it_away() {
        let dir = folder("import3dm");
        let typed = dir.join("parts.3dm").display().to_string();
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "5,0", "export", &typed]);
        let before = doc.state().clone();
        let mut processor = run(&mut doc, &["import", &typed]);
        assert_eq!(doc.state().objects.len(), 2);
        // The file's layer is merged with the model's of the same name.
        assert_eq!(doc.state().layers.len(), before.layers.len());
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state(), &before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_selected_writes_only_the_selection() {
        let dir = folder("exportsel");
        let file = dir.join("one.3dm");
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "5,0", "line", "0,1", "5,1"]);
        let first = doc.state().objects[0].id;
        doc.select(&[first]);
        run(&mut doc, &["exportselected", file.to_str().unwrap()]);
        let model = curva_3dm::read::read(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(model.objects.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saveas_3dm_writes_rhino_and_keeps_saving_curva() {
        let dir = folder("saveas3dm");
        let curva = dir.join("model.curva");
        let rhino = dir.join("model.3dm");
        let mut doc = Document::default();
        doc.save(&curva).unwrap();
        run(
            &mut doc,
            &["line", "0,0", "5,0", "saveas", rhino.to_str().unwrap()],
        );
        assert!(curva_3dm::read::read(&std::fs::read(&rhino).unwrap()).is_ok());
        assert_eq!(doc.path(), Some(curva.as_path()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_3dm_is_refused_and_the_model_kept() {
        let dir = folder("bad3dm");
        let file = dir.join("bad.3dm");
        std::fs::write(&file, b"3D Geometry File Format       70 not really").unwrap();
        let mut doc = Document::default();
        run(
            &mut doc,
            &["line", "0,0", "1,0", "open", file.to_str().unwrap()],
        );
        assert_eq!(doc.state().objects.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
