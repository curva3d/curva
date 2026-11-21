//! `Open`: replaces the model with one from a file, Curva's own or another program's.

use crate::commands::unexpected;
use crate::{Answer, Command, Finish, Message, Request, Step, Working};
use curva_document::Document;
use curva_document::progress::Job;
use std::path::Path;

/// The running command.
#[derive(Default)]
pub struct Open {
    /// A file to read once its format's options are given (issue #432).
    waiting: Option<String>,
}

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
/// The file is read on its own thread, so the interface keeps answering and the command bar shows
/// how far along it is (#480); the model is replaced once it has been read.
fn through_the_list(doc: &mut Document, text: &str) -> Option<Step> {
    let path = Path::new(text.trim().trim_matches('"')).to_path_buf();
    let open = doc.formats().of(&path).and_then(|f| f.open)?;
    let from = doc.settings_copy();
    let shown = text.to_owned();
    let name = path
        .file_name()
        .map_or_else(|| shown.clone(), |n| n.to_string_lossy().into_owned());
    let job = Job::start("job.open", vec![name]);
    Some(Step::Working(Working::spawn(job, move || {
        let read = open(&path, &shown, &from)
            .map(|(model, done)| (model, Message::with(done.key, done.args)));
        let finish: Finish = Box::new(move |doc: &mut Document| opened(doc, read));
        finish
    })))
}

/// The panel of options the format of the file typed asks before it is read, if any.
pub(crate) fn options_asked(doc: &Document, text: &str) -> Option<Step> {
    let path = Path::new(text.trim().trim_matches('"'));
    let panel = doc.formats().of(path).and_then(|f| f.asks)?;
    Some(Step::Ask(Request::panel_step(
        "prompt.open.options",
        crate::Panel::Format(panel),
    )))
}

impl Command for Open {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let text = match answer {
            None => {
                return Step::Ask(Request::file_to_open(
                    "prompt.open.path",
                    doc.formats().opening(),
                ));
            }
            Some(Answer::Path(text)) => {
                if let Some(ask) = options_asked(doc, &text) {
                    self.waiting = Some(text);
                    return ask;
                }
                text
            }
            Some(Answer::Empty) => match self.waiting.take() {
                Some(text) => text,
                None => return unexpected(),
            },
            Some(_) => return unexpected(),
        };
        if let Some(step) = through_the_list(doc, &text) {
            return step;
        }
        // Anything else is read as Curva's own file, as it always was.
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
    fn an_opened_model_is_shown_whole_once() {
        let folder = std::env::temp_dir().join(format!("curva-open-fit-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let typed = folder.join("model.curva").display().to_string();
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["line", "0,0", "5,5", "save", &typed]);
        while processor.take_view_request().is_some() {}
        let mut other = Document::default();
        let mut processor = run(&mut other, &["open", &typed]);
        assert_eq!(
            processor.take_view_request(),
            Some(crate::ViewRequest::ZoomExtents)
        );
        assert_eq!(processor.take_view_request(), None);
        assert!(!other.take_just_opened(), "fitted once only");
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
