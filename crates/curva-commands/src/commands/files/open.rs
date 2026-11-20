//! `Open`: replaces the model with one from a file, Curva's own or another program's.

use super::meshfiles::{MeshFormat, open_mesh_file};
use crate::commands::unexpected;
use crate::{Answer, Command, Message, Request, Step};
use curva_document::Document;
use std::path::Path;

/// The running command.
#[derive(Default)]
pub struct Open;

/// A model read from a file, or why it could not be, as the command bar shows it.
fn opened(doc: &mut Document, read: Result<(Document, Message), String>) -> Step {
    match read {
        Ok((model, message)) => {
            doc.replace_with(model);
            Step::Done(Some(message))
        }
        Err(error) => Step::Failed(Message::with("error.file", vec![error])),
    }
}

/// A file read by a format on the model's list of formats, if one reads it.
fn through_the_list(doc: &mut Document, text: &str) -> Option<Step> {
    let path = Path::new(text.trim().trim_matches('"'));
    let open = doc.formats().of(path).and_then(|f| f.open)?;
    let read = open(path, text).map(|(model, done)| (model, Message::with(done.key, done.args)));
    Some(opened(doc, read))
}

impl Command for Open {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(Answer::Path(text)) = answer else {
            return match answer {
                None => Step::Ask(Request::file_to_open(
                    "prompt.open.path",
                    crate::commands::import_export::READ_FORMATS,
                )),
                Some(_) => unexpected(),
            };
        };
        if let Some(step) = through_the_list(doc, &text) {
            return step;
        }
        let path = Path::new(text.trim().trim_matches('"'));
        let summarised =
            |summary: String| Message::with("done.open_3dm", vec![text.clone(), summary]);
        if crate::commands::import_export::is_3dm(path) {
            let read = crate::commands::import_export::open_3dm(path);
            return opened(doc, read.map(|(m, r)| (m, summarised(r.summary()))));
        }
        if crate::commands::import_export::is_skp(path) {
            let read = crate::commands::import_export::open_skp(path);
            return opened(doc, read.map(|(m, r)| (m, summarised(r.summary()))));
        }
        if let Some(format) = curva_dxf::Format::of(path) {
            let read = crate::commands::import_export::open_cad(path, format);
            return opened(doc, read.map(|(m, r)| (m, summarised(r.summary()))));
        }
        if let Some(format) = MeshFormat::of(path) {
            let read = open_mesh_file(path, format);
            return opened(doc, read.map(|(m, s)| (m, summarised(s))));
        }
        let read = Document::open(Path::new(&text)).map_err(|e| e.to_string());
        opened(
            doc,
            read.map(|m| (m, Message::with("done.open", vec![text.clone()]))),
        )
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
