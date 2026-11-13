//! `SaveAs`, as in Rhino: writes the model to a new file, which it then saves to.

use super::unexpected;
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use std::path::PathBuf;

/// The running command.
#[derive(Default)]
pub struct SaveAs;

impl Command for SaveAs {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.save.path", Kind::Path)),
            Some(Answer::Path(text)) => {
                let path = PathBuf::from(text.trim().trim_matches('"'));
                // A Rhino file is written beside the model, which keeps saving as Curva.
                if super::files::is_3dm(&path) {
                    return match super::files::write_3dm(doc, None, &path) {
                        Ok(report) => {
                            let mut notes: Vec<String> = report
                                .skipped
                                .iter()
                                .map(|(k, n)| format!("{n} {k}"))
                                .collect();
                            notes.extend(report.notes);
                            let notes = if notes.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", notes.join("; "))
                            };
                            Step::Done(Some(Message::with(
                                "done.save_3dm",
                                vec![path.display().to_string(), notes],
                            )))
                        }
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
    use super::super::run;
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
