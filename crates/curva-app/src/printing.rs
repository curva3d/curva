//! Carrying out `Print` (spec `drawing-output.md` §1): layouts made into pages by the viewport,
//! written by the model's printing format (a PDF, from `curva-pdf`), then saved, or handed to the
//! system's print command. Printing goes through a PDF so no `unsafe` code is needed (decision
//! 0020, point 7). SVG drawings from `Export` are written by the model's drawing format.

use crate::*;
use curva_commands::{PrintJob, PrintTo};

impl Curva {
    /// Makes the job's pages and sends them where it asks, reporting on the command line.
    pub(crate) fn print(&mut self, job: &PrintJob) {
        let fields = curva_document::fields::FieldContext::now(self.doc.path());
        let colour = if job.black {
            curva_viewport::print::PrintColour::Black
        } else {
            curva_viewport::print::PrintColour::Display
        };
        let pages: Vec<_> = job
            .layouts
            .iter()
            .filter_map(|id| curva_viewport::print::layout_page(&self.doc, *id, &fields, colour))
            .collect();
        let formats = std::sync::Arc::clone(self.doc.formats());
        let printer = formats.all().iter().find_map(|f| f.print);
        let failed = |error: String| Message::with("error.print_failed", vec![error]);
        let report = match &job.to {
            PrintTo::Svg(path) => match formats.all().iter().find_map(|f| f.drawing) {
                Some(write) => written(write(&pages, std::path::Path::new(path), path), failed),
                None => failed("no drawing format".to_owned()),
            },
            PrintTo::Pdf(path) => match printer {
                Some(write) => written(write(&pages, std::path::Path::new(path), path), failed),
                None => failed("no printing format".to_owned()),
            },
            PrintTo::Printer => match printer {
                Some(write) => {
                    let file = std::env::temp_dir()
                        .join(format!("curva-print-{}.pdf", std::process::id()));
                    let shown = file.display().to_string();
                    match write(&pages, &file, &shown).and_then(|_| send_to_printer(&shown)) {
                        Ok(()) => {
                            Message::with("done.print_printer", vec![pages.len().to_string()])
                        }
                        Err(error) => failed(error),
                    }
                }
                None => failed("no printing format".to_owned()),
            },
        };
        let line = self.text.render(&report);
        self.processor.note(line);
    }
}

/// What a format reports, for the command line.
fn written(
    result: Result<curva_document::formats::Done, String>,
    failed: impl Fn(String) -> Message,
) -> Message {
    match result {
        Ok(done) => Message::with(done.key, done.args),
        Err(error) => failed(error),
    }
}

/// Asks the system to print the PDF written at `shown`: Windows' print verb for PDFs, `lp`
/// elsewhere. The printer and paper are chosen in the system's own dialog or defaults.
fn send_to_printer(shown: &str) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        // No console window flashing up (spec `windows-no-console.md`).
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = std::process::Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Start-Process -FilePath '{}' -Verb Print",
                    shown.replace('\'', "''")
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("lp");
        command.arg(shown);
        command
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}
