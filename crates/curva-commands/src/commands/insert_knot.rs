//! `InsertKnot` (also `InsertControlPoint`): adds a control point where a curve is clicked,
//! keeping its shape, as Rhino's InsertKnot does.

use super::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;

/// The running command.
#[derive(Default)]
pub struct InsertKnot {
    added: usize,
}

fn ask_place(key: &'static str) -> Step {
    Step::Ask(Request {
        allow_empty: true,
        ..Request::new(key, Kind::PickPart)
    })
}

/// The curve under `at` with a point added there, and where the new point is.
fn added_at(doc: &Document, at: Point) -> Option<Result<(u64, Curve, Point), String>> {
    let (id, curve) = doc.editable_curve_at(at, ON_CURVE)?;
    Some(
        curve
            .with_point_added_at(at)
            .and_then(|made| Ok((id, made, curve.closest_point(at)?)))
            .map_err(|error| error.to_string()),
    )
}

impl Command for InsertKnot {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None | Some(Answer::Number(_)) => ask_place("prompt.insertknot.place"),
            Some(Answer::Empty) => Step::Done(Some(Message::with(
                "done.insertknot",
                vec![self.added.to_string()],
            ))),
            Some(Answer::Point(at)) => match added_at(doc, at) {
                None => ask_place("prompt.insertknot.missed"),
                Some(Err(why)) => {
                    // A pick at a curve's end is not worth stopping the command for.
                    Step::Ask(Request {
                        prompt: Message::with("prompt.insertknot.refused", vec![why]),
                        allow_empty: true,
                        ..Request::new("prompt.insertknot.refused", Kind::PickPart)
                    })
                }
                Some(Ok((id, made, _))) => {
                    doc.replace(id, made);
                    // The points after the new one have moved up a place, so no pick stays valid.
                    doc.select_grips(&[]);
                    doc.points_on(&[id]);
                    self.added += 1;
                    ask_place("prompt.insertknot.place")
                }
            },
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(at) = live.cursor else {
            return Vec::new();
        };
        match added_at(doc, at) {
            Some(Ok((_, made, on))) => vec![made, Curve::Point { at: on }],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn wave(doc: &mut Document) -> u64 {
        doc.add(
            Curve::nurbs(
                3,
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
                vec![
                    [0.0; 3],
                    [1.0, 2.0, 0.0],
                    [2.0, -1.0, 0.0],
                    [3.0, 2.0, 0.0],
                    [4.0, 0.0, 0.0],
                ],
                Vec::new(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn a_click_on_a_curve_adds_a_point_and_shows_them() {
        let mut doc = Document::default();
        let id = wave(&mut doc);
        let on = doc.state().objects[0].curve().unwrap().sample().unwrap()[9];
        let mut processor: Processor = run(&mut doc, &["insertknot"]);
        let preview = processor.preview(&doc, Some(on), "");
        assert_eq!(preview.len(), 2);
        processor.click(&mut doc, on);
        processor.submit(&mut doc, "");
        assert!(!processor.is_running());
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(doc.showing_points(), [id]);
        processor.submit(&mut doc, "undo");
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn insert_control_point_is_the_same_command() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap());
        let mut processor = run(&mut doc, &["insertcontrolpoint"]);
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.submit(&mut doc, "");
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn a_click_away_from_curves_asks_again() {
        let mut doc = Document::default();
        wave(&mut doc);
        let mut processor = run(&mut doc, &["insertknot"]);
        processor.click(&mut doc, [50.0, 50.0, 0.0]);
        assert!(processor.is_running());
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            5
        );
    }
}
