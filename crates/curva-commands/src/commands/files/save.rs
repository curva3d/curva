//! `Save`: writes the model to its file, asking for a location the first time.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use std::path::{Path, PathBuf};

/// The running command.
#[derive(Default)]
pub struct Save;

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
                None => Step::Ask(Request::new("prompt.save.path", Kind::Path)),
            },
            Some(Answer::Path(text)) => write(doc, &PathBuf::from(text)),
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

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
