//! `Point`: places a point object.

use super::{ask_point, unexpected};
use crate::{Answer, Command, Live, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct PointObject;

impl Command for PointObject {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => ask_point("prompt.point.at", Vec::new()),
            Some(Answer::Point(at)) => {
                doc.add(Curve::Point { at });
                Step::Done(Some(Message::new("done.point")))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, _doc: &Document, live: &Live) -> Vec<Curve> {
        live.cursor
            .map(|at| Curve::Point { at })
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn places_a_point_where_asked() {
        let mut doc = Document::default();
        run(&mut doc, &["point", "1,2,3"]);
        assert_eq!(
            *doc.state().objects[0].curve().unwrap(),
            Curve::Point {
                at: [1.0, 2.0, 3.0]
            }
        );
    }
}
