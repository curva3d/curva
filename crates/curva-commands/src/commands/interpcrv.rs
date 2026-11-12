//! `InterpCrv`: a smooth curve through points, ended by Enter or `Close`.

use super::{PointRun, RunState, geometry_failed};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct InterpCrv {
    run: PointRun,
}

impl Command for InterpCrv {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match self
            .run
            .feed(answer, "prompt.interpcrv.start", "prompt.interpcrv.next")
        {
            RunState::Asking(step) => *step,
            RunState::Finished { closed } => {
                let count = self.run.points.len();
                match Curve::interpolated(self.run.points.clone(), closed) {
                    Ok(curve) => {
                        doc.add(curve);
                        Step::Done(Some(Message::with(
                            "done.interpcrv",
                            vec![count.to_string()],
                        )))
                    }
                    Err(error) => geometry_failed(&error),
                }
            }
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        Curve::interpolated(self.run.through(live.cursor), false)
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn makes_a_curve_from_its_first_to_its_last_point() {
        let mut doc = Document::default();
        run(&mut doc, &["interpcrv", "0,0", "10,5", "20,-3", "30,4", ""]);
        let drawn = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(drawn[0][0].abs() < 1e-6);
        assert!((drawn[drawn.len() - 1][0] - 30.0).abs() < 1e-6);
    }

    #[test]
    fn close_joins_it_back_to_its_start() {
        let mut doc = Document::default();
        run(&mut doc, &["interpcrv", "0,0", "10,0", "10,10", "close"]);
        let drawn = doc.state().objects[0].curve().unwrap().sample().unwrap();
        let last = drawn[drawn.len() - 1];
        assert!(last[0].abs() < 1e-6 && last[1].abs() < 1e-6);
    }
}
