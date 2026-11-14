//! `Area`: measures the area inside closed flat curves.

use crate::commands::{ask_objects, curves_of, geometry_failed};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Area;

impl Command for Area {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.area.objects");
        }
        let mut total = 0.0;
        for (_, curve) in curves_of(doc, &chosen) {
            match curve.area() {
                Ok(area) => total += area,
                Err(error) => return geometry_failed(&error),
            }
        }
        Step::Done(Some(Message::with(
            "done.area",
            vec![curva_document::units::format_area(total, doc.state().unit)],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_adds_up_closed_curves() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let b = doc.add(
            Curve::polyline(
                vec![[5.0, 0.0, 0.0], [7.0, 0.0, 0.0], [7.0, 2.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["area"]);
        assert!(
            processor.log()[0].contains("5.142"),
            "{:?}",
            processor.log()
        );
    }

    #[test]
    fn an_open_curve_is_reported() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let processor = run(&mut doc, &["area"]);
        assert!(processor.log()[0].contains("not closed"));
    }
}
