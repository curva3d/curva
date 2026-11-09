//! `Polyline`: straight segments through points, ended by Enter or `Close`.

use super::{PointRun, RunState, geometry_failed};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct Polyline {
    run: PointRun,
}

impl Command for Polyline {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match self
            .run
            .feed(answer, "prompt.polyline.start", "prompt.polyline.next")
        {
            RunState::Asking(step) => step,
            RunState::Finished { closed } => {
                let count = self.run.points.len();
                match Curve::polyline(self.run.points.clone(), closed) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::with(
                            "done.polyline",
                            vec![count.to_string()],
                        )))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        Curve::polyline(self.run.through(live.cursor), false)
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn enter_finishes_an_open_polyline() {
        let mut doc = Document::default();
        run(&mut doc, &["polyline", "0,0", "10,0", "10,10", ""]);
        let length = doc.state().objects[0].curve.length().unwrap();
        assert!((length - 20.0).abs() < 1e-9);
    }

    #[test]
    fn close_adds_the_last_segment() {
        let mut doc = Document::default();
        run(&mut doc, &["pl", "0,0", "10,0", "10,10", "0,10", "close"]);
        let length = doc.state().objects[0].curve.length().unwrap();
        assert!((length - 40.0).abs() < 1e-9);
    }

    #[test]
    fn enter_needs_two_points_first() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["pl", "0,0", ""]);
        assert!(processor.is_running());
        assert!(doc.state().objects.is_empty());
    }
}
