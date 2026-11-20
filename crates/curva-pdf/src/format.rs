//! PDF and SVG on Curva's list of formats (spec `docs/specs/file-formats.md` §3): both write the
//! pages the viewport draws from layouts. PDF is what `Print` and `ExportPDF` write; SVG is what
//! `Export` writes, one file per layout. Moved unchanged from the interface.

use curva_document::drawing::Page;
use curva_document::formats::{Done, Format};
use std::path::{Path, PathBuf};

/// PDF drawings, as the list holds them.
#[must_use]
pub fn pdf() -> Format {
    Format {
        print: Some(write_pdf_file),
        ..Format::new("files.pdf", &["pdf"])
    }
}

/// SVG drawings, one file per layout, as the list holds them.
#[must_use]
pub fn svg() -> Format {
    Format {
        drawing: Some(svg_drawings),
        ..Format::new("files.svg", &["svg"])
    }
}

/// Every page in one PDF file.
fn write_pdf_file(pages: &[Page], path: &Path, shown: &str) -> Result<Done, String> {
    let pdf = crate::write_pdf(pages)?;
    std::fs::write(path, pdf).map_err(|e| e.to_string())?;
    Ok(Done {
        key: "done.print_pdf",
        args: vec![pages.len().to_string(), shown.to_owned()],
    })
}

/// The layouts' pages as SVG files, for `Export`.
fn svg_drawings(pages: &[Page], path: &Path, shown: &str) -> Result<Done, String> {
    let count = write_svgs(pages, path)?;
    Ok(Done {
        key: "done.export_svg",
        args: vec![count.to_string(), shown.to_owned()],
    })
}

/// Writes each page as an SVG file: one page at `path`; several beside it, each named after its
/// layout (`plan-Ground floor.svg`). Returns how many were written.
fn write_svgs(pages: &[Page], path: &Path) -> Result<usize, String> {
    if pages.is_empty() {
        return Err("nothing to draw".to_owned());
    }
    let files: Vec<PathBuf> = if pages.len() == 1 {
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
        std::fs::write(file, crate::write_svg(page)).map_err(|e| e.to_string())?;
    }
    Ok(files.len())
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
                source: None,
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
