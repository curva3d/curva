//! `Line`: a straight line between two points.

use super::{ask_point, geometry_failed, guide, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// The running command.
#[derive(Default)]
pub struct Line {
    start: Option<Point>,
}

impl Command for Line {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match (self.start, answer) {
            (None, None) => ask_point("prompt.line.start", Vec::new()),
            (None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                ask_point("prompt.line.end", vec![start])
            }
            (Some(start), Some(Answer::Point(end))) => match Curve::line(start, end) {
                Ok(curve) => {
                    doc.add(curve);
                    Step::Done(Some(Message::new("done.line")))
                }
                Err(error) => geometry_failed(&error),
            },
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
    use super::super::run;
    use curva_document::Document;

    #[test]
    fn makes_a_line_of_the_right_length() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0,0", "3,4,0"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - 5.0).abs() < 1e-9);
    }

    #[test]
    fn identical_points_make_nothing() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["l", "1,1", "1,1"]);
        assert!(doc.state().objects.is_empty());
        assert!(!processor.is_running());
    }
}
