//! `Print` (Serpentine's PrintSheets and Plot) and `ExportPDF` (spec `drawing-output.md` §1):
//! layouts to a PDF or a printer. Options first: which layouts, colour or black, and where; then
//! Enter asks for the file, or sends the pages to the printer. The interface makes the pages.

use crate::{
    Answer, Command, Kind, Message, PDF_FILES, PrintJob, PrintTo, Request, Step, ViewRequest,
};
use curva_document::Document;

const LAYOUTS: &str = "Layouts";
const COLOUR: &str = "Colour";
const OUTPUT: &str = "Output";

/// The running command.
#[derive(Default)]
pub struct Print {
    all: bool,
    black: bool,
    printer: bool,
    asking_file: bool,
}

impl Print {
    /// The layouts chosen: all of them in the Layouts panel's order, or the one being worked on
    /// (all when working in the model).
    fn layouts(&self, doc: &Document) -> Vec<u64> {
        match doc.space() {
            Some(current) if !self.all => vec![current],
            _ => doc.layouts().iter().map(|l| l.id).collect(),
        }
    }

    fn ask(&self, doc: &Document) -> Step {
        let options = vec![LAYOUTS, COLOUR, OUTPUT];
        let labels = vec![
            format!("{LAYOUTS}={}", if self.all { "All" } else { "Current" }),
            format!("{COLOUR}={}", if self.black { "Black" } else { "Display" }),
            format!("{OUTPUT}={}", if self.printer { "Printer" } else { "PDF" }),
        ];
        let request = if self.asking_file {
            let name = self
                .layouts(doc)
                .first()
                .and_then(|id| doc.layout(*id))
                .map(|l| l.name.clone());
            Request::file_to_save("prompt.print.file", &[PDF_FILES], name)
        } else {
            Request {
                allow_empty: true,
                ..Request::new("prompt.print.ready", Kind::Choice)
            }
        };
        Step::Ask(Request {
            options,
            option_labels: labels,
            ..request
        })
    }

    fn job(&self, doc: &Document, to: PrintTo) -> Step {
        let layouts = self.layouts(doc);
        if layouts.is_empty() {
            return Step::Failed(Message::new("error.print_no_layouts"));
        }
        Step::View(ViewRequest::Print(PrintJob {
            layouts,
            black: self.black,
            to,
        }))
    }
}

impl Command for Print {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None if doc.layouts().is_empty() => {
                Step::Failed(Message::new("error.print_no_layouts"))
            }
            None => self.ask(doc),
            Some(Answer::Option(LAYOUTS)) => {
                self.all = !self.all;
                self.ask(doc)
            }
            Some(Answer::Option(COLOUR)) => {
                self.black = !self.black;
                self.ask(doc)
            }
            Some(Answer::Option(OUTPUT)) => {
                self.printer = !self.printer;
                self.asking_file = false;
                self.ask(doc)
            }
            Some(Answer::Empty) if !self.asking_file => {
                if self.printer {
                    self.job(doc, PrintTo::Printer)
                } else {
                    self.asking_file = true;
                    self.ask(doc)
                }
            }
            Some(Answer::Path(path)) if self.asking_file => {
                let path = path.trim();
                if path.is_empty() {
                    return self.ask(doc);
                }
                let path = if std::path::Path::new(path)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                {
                    path.to_owned()
                } else {
                    format!("{path}.pdf")
                };
                self.job(doc, PrintTo::Pdf(path))
            }
            Some(_) => crate::commands::unexpected_step(),
        }
    }
}

/// `ExportPDF`: `Print` with a PDF as its output.
#[derive(Default)]
pub struct ExportPdf(Print);

impl Command for ExportPdf {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        self.0.step(doc, answer)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{PrintJob, PrintTo, ViewRequest};
    use curva_document::Document;

    fn sheets(doc: &mut Document) -> (u64, u64) {
        let paper = curva_document::layouts::paper_size("A3").unwrap();
        let a = doc.add_layout("Plans".to_owned(), paper, true);
        let b = doc.add_layout("Sections".to_owned(), paper, true);
        (a, b)
    }

    #[test]
    fn print_asks_for_a_pdf_and_hands_the_layouts_over() {
        let mut doc = Document::default();
        let (a, b) = sheets(&mut doc);
        doc.set_space(Some(b));
        let mut processor = run(&mut doc, &["Print", "", "drawings/set"]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Print(PrintJob {
                layouts: vec![b],
                black: false,
                to: PrintTo::Pdf("drawings/set.pdf".to_owned()),
            }))
        );
        let mut processor = run(&mut doc, &["Print", "Layouts", "Colour", "Output", ""]);
        assert_eq!(
            processor.take_view_request(),
            Some(ViewRequest::Print(PrintJob {
                layouts: vec![a, b],
                black: true,
                to: PrintTo::Printer,
            }))
        );
    }

    #[test]
    fn with_no_layouts_there_is_nothing_to_print() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["Print"]);
        assert_eq!(processor.take_view_request(), None);
        assert!(!processor.is_running());
    }
}
