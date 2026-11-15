//! Carrying out `Print` (spec `drawing-output.md` §1): layouts made into pages by the viewport,
//! written as a PDF by `curva-pdf`, then saved, or handed to the system's print command. Printing
//! goes through a PDF so no `unsafe` code is needed (decision 0020, point 7).

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
        let report = match curva_pdf::write_pdf(&pages) {
            Err(error) => Message::with("error.print_failed", vec![error]),
            Ok(pdf) => match &job.to {
                PrintTo::Pdf(path) => match std::fs::write(path, pdf) {
                    Ok(()) => Message::with(
                        "done.print_pdf",
                        vec![pages.len().to_string(), path.clone()],
                    ),
                    Err(error) => Message::with("error.print_failed", vec![error.to_string()]),
                },
                PrintTo::Printer => match send_to_printer(&pdf) {
                    Ok(()) => Message::with("done.print_printer", vec![pages.len().to_string()]),
                    Err(error) => Message::with("error.print_failed", vec![error]),
                },
            },
        };
        let line = self.text.render(&report);
        self.processor.note(line);
    }
}

/// Writes the PDF to a temporary file and asks the system to print it: Windows' print verb for
/// PDFs, `lp` elsewhere. The printer and paper are chosen in the system's own dialog or defaults.
fn send_to_printer(pdf: &[u8]) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("curva-print-{}.pdf", std::process::id()));
    std::fs::write(&path, pdf).map_err(|e| e.to_string())?;
    let shown = path.display().to_string();
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
        command.arg(&shown);
        command
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}
