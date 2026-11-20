//! Rhino `.3dm` files at the command line (spec `docs/specs/3dm-files.md` §3), AutoCAD's `.dxf`
//! and `.dwg` (spec `drawing-output.md` §3), and SketchUp's `.skp` (read only): `Import`,
//! `Export`, `ExportSelected`, and the `.3dm`, `.dxf`, `.dwg` and `.skp` side of `Open` and
//! `SaveAs`.

use super::meshfiles::{MeshFormat, export_mesh_file, import_mesh_file};
use crate::commands::{ask_objects, unexpected};
use crate::{
    Answer, CURVA_FILES, Command, DWG_FILES, DXF_FILES, FileFormat, Message, OBJ_FILES, PLY_FILES,
    PrintJob, PrintTo, RHINO_FILES, Request, SKP_FILES, STEP_FILES, STL_FILES, SVG_FILES, Step,
    ViewRequest,
};
use curva_document::Document;
use std::path::{Path, PathBuf};

/// What `Open` reads, Curva's own files first.
pub const READ_FORMATS: &[FileFormat] = &[
    CURVA_FILES,
    RHINO_FILES,
    DXF_FILES,
    DWG_FILES,
    SKP_FILES,
    STL_FILES,
    OBJ_FILES,
    PLY_FILES,
];

/// What `Import` reads.
pub const IMPORT_FORMATS: &[FileFormat] = &[
    CURVA_FILES,
    RHINO_FILES,
    DXF_FILES,
    DWG_FILES,
    STL_FILES,
    OBJ_FILES,
    PLY_FILES,
];

/// What `Save` and `SaveAs` write, Curva's own files first.
pub const SAVE_FORMATS: &[FileFormat] = &[CURVA_FILES, RHINO_FILES];

/// What `Export` and `ExportSelected` write.
pub const EXPORT_FORMATS: &[FileFormat] = &[
    RHINO_FILES,
    DXF_FILES,
    DWG_FILES,
    STEP_FILES,
    STL_FILES,
    OBJ_FILES,
    PLY_FILES,
    SVG_FILES,
];

/// The model's file name without its extension, for a save dialog's name field.
pub fn model_name(doc: &Document) -> Option<String> {
    doc.path()
        .and_then(Path::file_stem)
        .map(|stem| stem.to_string_lossy().into_owned())
}

/// A path as typed, without the quotes a pasted one may have.
fn typed(text: &str) -> PathBuf {
    PathBuf::from(text.trim().trim_matches('"'))
}

fn failed(error: impl std::fmt::Display) -> Step {
    Step::Failed(Message::with("error.file", vec![error.to_string()]))
}

/// What a format on the model's list reports, for the command bar.
fn done(result: Result<curva_document::formats::Done, String>) -> Step {
    match result {
        Ok(done) => Step::Done(Some(Message::with(done.key, done.args))),
        Err(error) => failed(error),
    }
}

/// `Import`: a `.3dm`, `.curva`, `.dxf` or `.dwg` file's contents added to the model, its layers
/// merged by name.
#[derive(Default)]
pub struct Import;

impl Command for Import {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::file_to_open("prompt.import.path", IMPORT_FORMATS)),
            Some(Answer::Path(text)) => {
                let path = typed(&text);
                if let Some(import) = doc.formats().of(&path).and_then(|f| f.import) {
                    return done(import(doc, &path, &path.display().to_string()));
                }
                if let Some(format) = MeshFormat::of(&path) {
                    return match import_mesh_file(doc, &path, format) {
                        Ok(summary) => Step::Done(Some(Message::with(
                            "done.import",
                            vec![path.display().to_string(), summary],
                        ))),
                        Err(e) => failed(e),
                    };
                }
                // A Curva file comes in the same way as a Rhino file, as plain geometry.
                done(curva_3dm::format::import_curva(
                    doc,
                    &path,
                    &path.display().to_string(),
                ))
            }
            Some(_) => unexpected(),
        }
    }
}

/// `Export` and `ExportSelected`: the model, or the chosen objects, written as `.3dm`, `.dxf` or
/// `.dwg`.
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

impl Export {
    /// The model, or the chosen objects, written to the file typed.
    fn write(&self, doc: &mut Document, text: &str) -> Step {
        let mut path = typed(text);
        if path.extension().is_none() {
            path.set_extension("3dm");
        }
        if let Some(export) = doc.formats().of(&path).and_then(|f| f.export) {
            let chosen = self.chosen.as_deref();
            return done(export(doc, chosen, &path, &path.display().to_string()));
        }
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        {
            // A drawing: the open layout, as it prints, or every layout from the model.
            if self.selected {
                return Step::Failed(Message::new("error.export_svg_selected"));
            }
            let layouts = match doc.space() {
                Some(sheet) => vec![sheet],
                None => doc.layouts().iter().map(|l| l.id).collect(),
            };
            if layouts.is_empty() {
                return Step::Failed(Message::new("error.export_svg_no_layouts"));
            }
            return Step::View(ViewRequest::Print(PrintJob {
                layouts,
                black: false,
                to: PrintTo::Svg(path.display().to_string()),
            }));
        }
        if let Some(format) = MeshFormat::of(&path) {
            let chosen = self.chosen.as_deref();
            return match export_mesh_file(doc, chosen, &path, format) {
                Ok(summary) => Step::Done(Some(Message::with(
                    "done.export_cad",
                    vec![path.display().to_string(), summary],
                ))),
                Err(e) => failed(e),
            };
        }
        Step::Failed(Message::with(
            "error.export_kind",
            vec![path.display().to_string()],
        ))
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
                Step::Ask(Request::file_to_save(
                    "prompt.export.path",
                    EXPORT_FORMATS,
                    model_name(doc),
                ))
            }
            None if self.selected => {
                let chosen = doc.selection();
                if chosen.is_empty() {
                    return ask_objects("prompt.export.objects");
                }
                self.chosen = Some(chosen);
                Step::Ask(Request::file_to_save(
                    "prompt.export.path",
                    EXPORT_FORMATS,
                    model_name(doc),
                ))
            }
            None => Step::Ask(Request::file_to_save(
                "prompt.export.path",
                EXPORT_FORMATS,
                model_name(doc),
            )),
            Some(Answer::Path(text)) => self.write(doc, &text),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
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
    fn svg_exports_the_open_layout_or_every_layout_as_drawings() {
        use crate::{PrintJob, PrintTo, ViewRequest};
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["export", "plan.svg"]);
        assert_eq!(processor.take_view_request(), None, "no layouts to draw");
        run(&mut doc, &["layout", "", "layout", ""]);
        let mut processor = run(&mut doc, &["export", "plan.svg"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Print(PrintJob {
                layouts: vec![doc.layouts()[1].id],
                black: false,
                to: PrintTo::Svg("plan.svg".to_owned()),
            }))
        );
        doc.set_space(None);
        let mut processor = run(&mut doc, &["export", "plan.svg"]);
        let Some(ViewRequest::Print(job)) = processor.take_view_request() else {
            panic!("no print");
        };
        assert_eq!(job.layouts.len(), 2);
    }

    #[test]
    fn dxf_and_dwg_go_out_and_open_or_import() {
        let dir = folder("cadfiles");
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "5,0", "circle", "10,10", "3"]);
        for name in ["plan.dxf", "plan.dwg"] {
            let file = dir.join(name);
            run(&mut doc, &["export", file.to_str().unwrap()]);
            assert!(file.exists(), "{name} written");
            let mut opened = Document::default();
            run(&mut opened, &["open", file.to_str().unwrap()]);
            assert_eq!(opened.state().objects.len(), 2, "{name} opened");
            assert_eq!(opened.path(), Some(dir.join("plan.curva").as_path()));
            run(&mut opened, &["import", file.to_str().unwrap()]);
            assert_eq!(opened.state().objects.len(), 4, "{name} imported");
        }
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
