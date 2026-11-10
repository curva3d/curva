//! `PointsOn`: shows the control points of curves so they can be picked and moved.

use super::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct PointsOn;

impl Command for PointsOn {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.pointson.objects");
        }
        let shown = doc.points_on(&chosen);
        if shown == 0 {
            return Step::Failed(Message::new("error.no_points"));
        }
        // As in Rhino, the curves are let go so that clicks pick their points.
        doc.select(&[]);
        Step::Done(Some(Message::with(
            "done.pointson",
            vec![shown.to_string()],
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn points_show_and_points_off_hides_them() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["pointson"]);
        assert_eq!(doc.showing_points(), [id]);
        assert!(doc.selection().is_empty());
        run(&mut doc, &["pointsoff"]);
        assert!(doc.showing_points().is_empty());
    }

    #[test]
    fn a_curve_without_points_is_reported() {
        let mut doc = Document::default();
        let id = doc.add(Curve::Point { at: [0.0; 3] });
        doc.select(&[id]);
        let processor = run(&mut doc, &["pointson"]);
        assert!(doc.showing_points().is_empty());
        assert_eq!(processor.log().len(), 1);
    }
}
