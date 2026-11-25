//! `Save`: writes the model to its file, asking for a location the first time.

use crate::commands::unexpected;
use crate::{Answer, Command, Message, Request, Step};
use curva_document::Document;
use std::path::{Path, PathBuf};

/// The running command.
#[derive(Default)]
pub struct Save;

/// The path typed or chosen, given `.curva` when it has no extension: the Linux file dialog
/// does not add one, and a file without it is not known as a Curva model.
pub(super) fn curva_path(text: &str) -> PathBuf {
    let mut path = PathBuf::from(text.trim().trim_matches('"'));
    if path.extension().is_none() {
        path.set_extension(crate::CURVA_FILES.1[0]);
    }
    path
}

fn write(doc: &mut Document, path: &Path) -> Step {
    match doc.save(path) {
        Ok(()) => Step::Done(Some(Message::with(
            "done.save",
            vec![path.display().to_string()],
        ))),
        Err(error) => Step::Failed(Message::with("error.file", vec![error.to_string()])),
    }
}

impl Command for Save {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => match doc.path().map(Path::to_path_buf) {
                Some(path) => write(doc, &path),
                None => Step::Ask(Request::file_to_save(
                    "prompt.save.path",
                    doc.formats().saving(),
                    crate::commands::files::import_export::model_name(doc),
                )),
            },
            Some(Answer::Path(text)) => write(doc, &curva_path(&text)),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn a_name_without_an_extension_is_saved_as_a_curva_file() {
        assert_eq!(
            super::curva_path("/home/me/plane glitch"),
            std::path::PathBuf::from("/home/me/plane glitch.curva")
        );
        assert_eq!(
            super::curva_path("model.3dm"),
            std::path::PathBuf::from("model.3dm")
        );
    }

    #[test]
    fn asks_once_then_saves_to_the_same_place() {
        let folder = std::env::temp_dir().join(format!("curva-save-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("model.curva");
        let typed = path.display().to_string();
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["line", "0,0", "1,0", "save", &typed]);
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
        processor.submit(&mut doc, "save");
        assert!(!processor.is_running());
        assert!(path.exists());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
