//! `Distance`, as in Rhino: measures between two points, with the angle in the construction plane,
//! the elevation above it and the steps along its axes (#694).

use crate::commands::{ask_point, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, cross, dot, gap, sub};

/// The running command.
#[derive(Default)]
pub struct Distance {
    start: Option<Point>,
}

impl Command for Distance {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.start, answer) {
            (None, None) => ask_point("prompt.distance.start", Vec::new()),
            (None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.distance.end", vec![start])
            }
            (Some(start), Some(Answer::Point(end))) => {
                let [x, y] = doc.work_plane();
                let up = cross(x, y);
                let d = sub(end, start);
                let (dx, dy, dz) = (dot(d, x), dot(d, y), dot(d, up));
                let xy = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                let elevation = dz.atan2(dx.hypot(dy)).to_degrees();
                Step::Done(Some(Message::with(
                    "done.distance",
                    vec![
                        doc.length_text(gap(start, end)),
                        format!("{xy:.3}"),
                        format!("{elevation:.3}"),
                        format!("{dx:.3}"),
                        format!("{dy:.3}"),
                        format!("{dz:.3}"),
                    ],
                )))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        match (self.start, live.cursor) {
            (Some(start), Some(cursor)) => guide(start, cursor),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn it_reports_the_distance_and_changes_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["distance", "0,0", "3,4"]);
        assert!(processor.log()[0].contains("5.000"));
        // The angle in the plane, the elevation and the steps, as Rhino reports them.
        for part in ["53.130", "dx 3.000", "dy 4.000", "dz 0.000"] {
            assert!(processor.log()[0].contains(part), "{}", processor.log()[0]);
        }
        assert!(doc.state().objects.is_empty());
    }
}
