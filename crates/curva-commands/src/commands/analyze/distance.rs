//! `Distance`: measures between two points.

use crate::commands::{ask_point, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, gap};

/// The running command.
#[derive(Default)]
pub struct Distance {
    start: Option<Point>,
}

impl Command for Distance {
    fn step(&mut self, _doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.start, answer) {
            (None, None) => ask_point("prompt.distance.start", Vec::new()),
            (None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.distance.end", vec![start])
            }
            (Some(start), Some(Answer::Point(end))) => Step::Done(Some(Message::with(
                "done.distance",
                vec![format!("{:.3}", gap(start, end))],
            ))),
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
        assert!(doc.state().objects.is_empty());
    }
}
