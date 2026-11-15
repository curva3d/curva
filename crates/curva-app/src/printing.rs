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
        if let PrintTo::Svg(path) = &job.to {
            let report = match write_svgs(&pages, std::path::Path::new(path)) {
                Ok(count) => {
                    Message::with("done.export_svg", vec![count.to_string(), path.clone()])
                }
                Err(error) => Message::with("error.print_failed", vec![error]),
            };
            let line = self.text.render(&report);
            self.processor.note(line);
            return;
        }
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
                // Handled above, before any PDF is made.
                PrintTo::Svg(_) => return,
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

/// Writes each page as an SVG file: one page at `path`; several beside it, each named after its
/// layout (`plan-Ground floor.svg`). Returns how many were written.
fn write_svgs(
    pages: &[curva_document::drawing::Page],
    path: &std::path::Path,
) -> Result<usize, String> {
    if pages.is_empty() {
        return Err("nothing to draw".to_owned());
    }
    let files: Vec<std::path::PathBuf> = if pages.len() == 1 {
        vec![path.to_path_buf()]
    } else {
        let stem = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let name: String = if page.name.is_empty() {
                    (index + 1).to_string()
                } else {
                    page.name.clone()
                };
                // Characters file names cannot hold become dashes.
                let name: String = name
                    .chars()
                    .map(|c| if "<>:\"/\\|?*".contains(c) { '-' } else { c })
                    .collect();
                path.with_file_name(format!("{stem}-{name}.svg"))
            })
            .collect()
    };
    for (page, file) in pages.iter().zip(&files) {
        std::fs::write(file, curva_pdf::write_svg(page)).map_err(|e| e.to_string())?;
    }
    Ok(files.len())
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

#[cfg(test)]
mod tests {
    use curva_document::drawing::{Mark, Page};

    fn page(name: &str) -> Page {
        Page {
            name: name.to_owned(),
            size: [420.0, 297.0],
            marks: vec![Mark::Line {
                points: vec![[0.0, 0.0], [10.0, 0.0]],
                closed: false,
                colour: [0.0; 3],
                width: 0.25,
                dashes: Vec::new(),
            }],
        }
    }

    #[test]
    fn one_page_is_one_svg_and_more_are_named_after_their_layouts() {
        let dir = std::env::temp_dir().join(format!("curva-svgs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let one = dir.join("plan.svg");
        assert_eq!(super::write_svgs(&[page("Plan")], &one), Ok(1));
        assert!(std::fs::read_to_string(&one).unwrap().contains("<svg"));
        let set = dir.join("set.svg");
        assert_eq!(
            super::write_svgs(&[page("Ground/first"), page("")], &set),
            Ok(2)
        );
        assert!(dir.join("set-Ground-first.svg").exists());
        assert!(dir.join("set-2.svg").exists());
        assert!(super::write_svgs(&[], &set).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
