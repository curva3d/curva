//! `CloseCrv`: joins the ends of open polylines and smooth curves.

use crate::commands::{ask_objects, curves_of};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct CloseCrv;

fn closed(curve: &Curve) -> Option<Curve> {
    match curve {
        Curve::Polyline {
            points,
            closed: false,
        } if points.len() >= 3 => Curve::polyline(points.clone(), true).ok(),
        Curve::Interpolated {
            points,
            closed: false,
        } if points.len() >= 3 => Curve::interpolated(points.clone(), true).ok(),
        _ => None,
    }
}

impl Command for CloseCrv {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.closecrv.objects");
        }
        let mut done = 0;
        for (id, curve) in curves_of(doc, &chosen) {
            if let Some(whole) = closed(&curve) {
                doc.replace(id, whole);
                done += 1;
            }
        }
        if done == 0 {
            return Step::Failed(Message::new("error.nothing_to_close"));
        }
        Step::Done(Some(Message::with("done.closecrv", vec![done.to_string()])))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected(curve: Curve) -> Document {
        let mut doc = Document::default();
        let id = doc.add(curve);
        doc.select(&[id]);
        doc
    }

    #[test]
    fn a_polyline_gains_its_last_side() {
        let path = vec![[0.0; 3], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        let mut doc = selected(Curve::polyline(path, false).unwrap());
        run(&mut doc, &["closecrv"]);
        assert!((doc.state().objects[0].curve().unwrap().length().unwrap() - 12.0).abs() < 1e-9);
    }

    #[test]
    fn a_smooth_curve_closes() {
        let path = vec![[0.0; 3], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        let mut doc = selected(Curve::interpolated(path, false).unwrap());
        run(&mut doc, &["closecrv"]);
        assert!(matches!(
            doc.state().objects[0].curve().unwrap(),
            Curve::Interpolated { closed: true, .. }
        ));
    }

    #[test]
    fn a_line_cannot_close() {
        let mut doc = selected(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let processor = run(&mut doc, &["closecrv"]);
        assert!(!processor.is_running());
        assert!(matches!(
            doc.state().objects[0].curve().unwrap(),
            Curve::Line { .. }
        ));
    }
}
