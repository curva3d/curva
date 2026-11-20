//! `SaveAs`, as in Rhino: writes the model to a new file, which it then saves to.

use crate::commands::unexpected;
use crate::{Answer, Command, Message, Request, Step};
use curva_document::Document;
use std::path::PathBuf;

/// The running command.
#[derive(Default)]
pub struct SaveAs;

impl Command for SaveAs {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::file_to_save(
                "prompt.save.path",
                doc.formats().saving(),
                crate::commands::files::import_export::model_name(doc),
            )),
            Some(Answer::Path(text)) => {
                let path = PathBuf::from(text.trim().trim_matches('"'));
                if let Some(save) = doc.formats().of(&path).and_then(|f| f.save) {
                    return match save(doc, &path, &path.display().to_string()) {
                        Ok(done) => Step::Done(Some(Message::with(done.key, done.args))),
                        Err(error) => Step::Failed(Message::with("error.file", vec![error])),
                    };
                }
                match doc.save(&path) {
                    Ok(()) => Step::Done(Some(Message::with(
                        "done.save",
                        vec![path.display().to_string()],
                    ))),
                    Err(error) => {
                        Step::Failed(Message::with("error.file", vec![error.to_string()]))
                    }
                }
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn asks_for_a_name_even_when_the_model_has_a_file() {
        let folder = std::env::temp_dir().join(format!("curva-saveas-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let (first, second) = (folder.join("a.curva"), folder.join("b.curva"));
        let mut doc = Document::default();
        doc.save(&first).unwrap();
        let processor = run(&mut doc, &["saveas", second.to_str().unwrap()]);
        assert!(!processor.is_running());
        assert!(second.exists());
        assert_eq!(doc.path(), Some(second.as_path()));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
