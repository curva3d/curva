//! `PointsOn`: shows the control points of curves, meshes, SubDs and surfaces so they can be
//! picked and moved.

use crate::commands::{ask_objects, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct PointsOn;

impl Command for PointsOn {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.pointson.objects");
        };
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
    use crate::commands::run;
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

    #[test]
    fn a_planes_four_points_show_and_moving_one_reshapes_it_as_in_rhino() {
        let mut doc = Document::default();
        let plane = curva_kernel::Body::ruled(
            &Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]).unwrap(),
            false,
        )
        .unwrap();
        let id = doc.add_body(plane);
        doc.select(&[id]);
        run(&mut doc, &["pointson"]);
        // Rhino 8: a 10 × 10 plane shows 4 control points (#768).
        let grips = doc.shown_grips();
        assert_eq!(grips.len(), 4);
        let (grip, _) = grips[0];
        let reshaped = doc
            .grips_reshaped(&[grip], |p| [p[0], p[1], p[2] + 3.0])
            .unwrap();
        let (_, shape) = &reshaped[0];
        let (_, high) = shape.body().unwrap().bounds().unwrap();
        assert!((high[2] - 3.0).abs() < 1e-9, "{high:?}");
    }
}
