//! Rhino `.3dm` files at the command line (spec `docs/specs/3dm-files.md` §3), AutoCAD's `.dxf`
//! and `.dwg` (spec `drawing-output.md` §3), and SketchUp's `.skp` (read only): `Import`,
//! `Export`, `ExportSelected`, and the `.3dm`, `.dxf`, `.dwg` and `.skp` side of `Open` and
//! `SaveAs`.

use crate::commands::{ask_objects, unexpected};
use crate::{
    Answer, Command, Finish, Message, PrintJob, PrintTo, Request, Step, ViewRequest, Working,
};
use curva_document::Document;
use curva_document::progress::Job;
use std::path::{Path, PathBuf};

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
pub struct Import {
    /// A file to read once its format's options are given (issue #432).
    waiting: Option<String>,
}

impl Command for Import {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::file_to_open(
                "prompt.import.path",
                doc.formats().importing(),
            )),
            Some(Answer::Path(text)) => {
                if let Some(ask) = super::open::options_asked(doc, &text) {
                    self.waiting = Some(text);
                    return ask;
                }
                Self::read(doc, &text)
            }
            Some(Answer::Empty) => match self.waiting.take() {
                Some(text) => Self::read(doc, &text),
                None => unexpected(),
            },
            Some(_) => unexpected(),
        }
    }
}

impl Import {
    /// The file typed, read into the model by its format, or as Curva's own file.
    fn read(doc: &mut Document, text: &str) -> Step {
        let path = typed(text);
        // Anything else is read as Curva's own file, the first on the list.
        let import = doc
            .formats()
            .of(&path)
            .and_then(|f| f.import)
            .or_else(|| doc.formats().all().first().and_then(|f| f.import));
        let Some(import) = import else {
            return failed("not a kind of file Curva can import");
        };
        // Read into a copy of the model on its own thread, so the interface keeps answering and
        // shows how far along it is (#480); the copy's model and views then become the model's.
        let mut copy = doc.settings_copy();
        copy.restore(doc.state().clone());
        let name = path
            .file_name()
            .map_or_else(|| text.to_owned(), |n| n.to_string_lossy().into_owned());
        let job = Job::start("job.import", vec![name]);
        Step::Working(Working::spawn(job, move || {
            let result = import(&mut copy, &path, &path.display().to_string());
            let finish: Finish = Box::new(move |doc: &mut Document| {
                if result.is_ok() {
                    doc.restore(copy.state().clone());
                    for view in copy.views() {
                        if doc.saved_view(&view.name).is_none() {
                            doc.save_view(view.clone());
                        }
                    }
                    for plane in copy.cplanes() {
                        if doc.saved_cplane(&plane.name).is_none() {
                            doc.save_cplane(plane.clone());
                        }
                    }
                }
                done(result)
            });
            finish
        }))
    }
}

/// `Export` and `ExportSelected`: the model, or the chosen objects, written as `.3dm`, `.dxf` or
/// `.dwg`.
#[derive(Default)]
pub struct Export {
    selected: bool,
    chosen: Option<Vec<u64>>,
    /// A file to write once the mesh settings are given, for a format that meshes (#387).
    meshing: Option<PathBuf>,
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
    fn write(&mut self, doc: &mut Document, text: &str) -> Step {
        let mut path = typed(text);
        // No extension: the first kind `Export` offers, as its dialog does.
        if path.extension().is_none()
            && let Some((_, extensions)) = doc.formats().exporting().first()
        {
            path.set_extension(extensions[0]);
        }
        // A format that meshes asks for the mesh settings first, in the interface's panel.
        if doc.formats().of(&path).is_some_and(|f| f.meshes) {
            self.meshing = Some(path);
            return Step::Ask(Request::panel_step(
                "prompt.export.mesh",
                crate::Panel::MeshSettings,
            ));
        }
        self.written(doc, &path)
    }

    /// The model, or the chosen objects, written to `path` by its format.
    fn written(&self, doc: &Document, path: &Path) -> Step {
        let path = path.to_path_buf();
        if let Some(export) = doc.formats().of(&path).and_then(|f| f.export) {
            let chosen = self.chosen.as_deref();
            return done(export(doc, chosen, &path, &path.display().to_string()));
        }
        if doc.formats().of(&path).is_some_and(|f| f.drawing.is_some()) {
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
                    doc.formats().exporting(),
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
                    doc.formats().exporting(),
                    model_name(doc),
                ))
            }
            None => Step::Ask(Request::file_to_save(
                "prompt.export.path",
                doc.formats().exporting(),
                model_name(doc),
            )),
            Some(Answer::Path(text)) => self.write(doc, &text),
            Some(Answer::Empty) if self.meshing.is_some() => match self.meshing.take() {
                Some(path) => self.written(doc, &path),
                None => unexpected(),
            },
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
