//! The system's own file dialogs, shown when a command asks for a file (spec
//! `feedback-0.4.2.md` §1). A dialog runs on a worker thread, so Curva keeps drawing while it is
//! open; its answer goes to the command as if typed, and Cancel ends the command.

use crate::*;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

/// A dialog open for a command's question.
pub(crate) struct OpenDialog {
    /// The question it answers. If the command has moved on (a location was typed instead, or
    /// it was cancelled), the dialog's answer is left unused.
    request: curva_commands::Request,
    /// The file picked, or `None` for Cancel; sent once, when the dialog closes.
    picked: Receiver<Option<PathBuf>>,
}

/// File dialogs and where the last one was left.
#[derive(Default)]
pub(crate) struct FileDialogs {
    /// The dialog open now, if any.
    open: Option<OpenDialog>,
    /// The folder of the last file picked, where the next dialog starts.
    folder: Option<PathBuf>,
}

impl Curva {
    /// Shows a dialog when the running command asks for a file with one, and hands its answer
    /// back when it closes.
    pub(crate) fn serve_file_dialogs(&mut self, frame: &eframe::Frame) {
        if let Some(open) = &self.file_dialogs.open {
            let answer = match open.picked.try_recv() {
                Ok(answer) => answer,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => None,
            };
            let still_asking = self.processor.current_request() == Some(&open.request);
            self.file_dialogs.open = None;
            if still_asking {
                match answer {
                    Some(path) => {
                        self.file_dialogs.folder = path.parent().map(PathBuf::from);
                        self.processor
                            .submit(&mut self.doc, &path.display().to_string());
                    }
                    None => self.processor.cancel(&mut self.doc),
                }
            }
            return;
        }
        let Some(request) = self.processor.current_request() else {
            return;
        };
        let Some(wanted) = request.file.clone() else {
            return;
        };
        let request = request.clone();
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let mut dialog = rfd::AsyncFileDialog::new().set_parent(frame);
        if let Some(folder) = &self.file_dialogs.folder {
            dialog = dialog.set_directory(folder);
        }
        if !wanted.save && wanted.formats.len() > 1 {
            let all: Vec<&str> = wanted
                .formats
                .iter()
                .flat_map(|(_, extensions)| extensions.iter().copied())
                .collect();
            dialog = dialog.add_filter(say("files.all_supported"), &all);
        }
        for (key, extensions) in wanted.formats {
            dialog = dialog.add_filter(say(key), extensions);
        }
        if wanted.save {
            let name = wanted.name.unwrap_or_else(|| say("ui.untitled"));
            dialog = dialog.set_file_name(name);
        }
        let (send, picked) = std::sync::mpsc::channel();
        let save = wanted.save;
        std::thread::spawn(move || {
            let chosen = pollster::block_on(async {
                if save {
                    dialog.save_file().await
                } else {
                    dialog.pick_file().await
                }
            });
            let _ = send.send(chosen.map(|file| file.path().to_path_buf()));
        });
        self.file_dialogs.open = Some(OpenDialog { request, picked });
    }
}
