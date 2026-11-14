//! `Length`: measures curves.

use crate::commands::{ask_objects, curves_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Length;

impl Command for Length {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.length.objects");
        }
        let mut total = 0.0;
        for (_, curve) in curves_of(doc, &chosen) {
            match curve.length() {
                Ok(length) => total += length,
                Err(error) => return geometry_failed(&error),
            }
        }
        Step::Done(Some(Message::with(
            "done.length",
            vec![format!("{total:.3}")],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_adds_up_the_selected_curves() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [3.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0; 3], [0.0, 4.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["length"]);
        assert!(processor.log()[0].contains("7.000"));
    }
}
