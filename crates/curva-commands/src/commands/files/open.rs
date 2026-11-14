//! `Open`: replaces the model with one from a file.

use crate::commands::unexpected;
use crate::{Answer, Command, Message, Request, Step};
use curva_document::Document;
use std::path::Path;

/// The running command.
#[derive(Default)]
pub struct Open;

impl Command for Open {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::file_to_open(
                "prompt.open.path",
                crate::commands::import_export::READ_FORMATS,
            )),
            Some(Answer::Path(text))
                if crate::commands::import_export::is_3dm(Path::new(
                    text.trim().trim_matches('"'),
                )) =>
            {
                match crate::commands::import_export::open_3dm(Path::new(
                    text.trim().trim_matches('"'),
                )) {
                    Ok((opened, report)) => {
                        *doc = opened;
                        Step::Done(Some(Message::with(
                            "done.open_3dm",
                            vec![text, report.summary()],
                        )))
                    }
                    Err(error) => Step::Failed(Message::with("error.file", vec![error])),
                }
            }
            Some(Answer::Path(text)) => match Document::open(Path::new(&text)) {
                Ok(opened) => {
                    *doc = opened;
                    Step::Done(Some(Message::with("done.open", vec![text])))
                }
                Err(error) => Step::Failed(Message::with("error.file", vec![error.to_string()])),
            },
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn a_saved_model_comes_back_the_same() {
        let folder = std::env::temp_dir().join(format!("curva-open-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let typed = folder.join("model.curva").display().to_string();
        let mut doc = Document::default();
        run(
            &mut doc,
            &["interpcrv", "0,0", "5,5", "9,0", "", "save", &typed],
        );
        let saved = doc.state().clone();
        let mut other = Document::default();
        run(&mut other, &["open", &typed]);
        assert_eq!(other.state(), &saved);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_missing_file_leaves_the_model_alone() {
        let mut doc = Document::default();
        let processor = run(
            &mut doc,
            &["line", "0,0", "1,0", "open", "no-such-file.curva"],
        );
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }
}
