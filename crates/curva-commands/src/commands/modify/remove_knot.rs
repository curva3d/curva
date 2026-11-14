//! `RemoveKnot`: takes away the control point nearest where a curve is clicked.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, gap};

/// How far from a curve a picked point may be, in millimetres, to mean that curve.
const ON_CURVE: f64 = 1e-3;

/// The running command.
#[derive(Default)]
pub struct RemoveKnot {
    removed: usize,
}

fn ask_place(key: &'static str) -> Step {
    Step::Ask(Request {
        allow_empty: true,
        ..Request::new(key, Kind::PickPart)
    })
}

/// The curve under `at` without the control point nearest `at`.
fn removed_at(doc: &Document, at: Point) -> Option<Result<(u64, Curve), String>> {
    let (id, curve) = doc.editable_curve_at(at, ON_CURVE)?;
    let points = match curve.control_points() {
        Ok(points) => points,
        Err(error) => return Some(Err(error.to_string())),
    };
    let nearest =
        (0..points.len()).min_by(|a, b| gap(points[*a], at).total_cmp(&gap(points[*b], at)))?;
    Some(
        curve
            .without_control_point(nearest)
            .map(|made| (id, made))
            .map_err(|error| error.to_string()),
    )
}

impl Command for RemoveKnot {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None | Some(Answer::Number(_)) => ask_place("prompt.removeknot.place"),
            Some(Answer::Empty) => Step::Done(Some(Message::with(
                "done.removeknot",
                vec![self.removed.to_string()],
            ))),
            Some(Answer::Point(at)) => match removed_at(doc, at) {
                None => ask_place("prompt.removeknot.missed"),
                Some(Err(why)) => Step::Ask(Request {
                    prompt: Message::with("prompt.removeknot.refused", vec![why]),
                    allow_empty: true,
                    ..Request::new("prompt.removeknot.refused", Kind::PickPart)
                }),
                Some(Ok((id, made))) => {
                    doc.replace(id, made);
                    doc.select_grips(&[]);
                    doc.points_on(&[id]);
                    self.removed += 1;
                    ask_place("prompt.removeknot.place")
                }
            },
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(at) = live.cursor else {
            return Vec::new();
        };
        match removed_at(doc, at) {
            Some(Ok((_, made))) => vec![made],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn the_point_nearest_the_click_goes() {
        let mut doc = Document::default();
        doc.add(
            Curve::polyline(
                vec![[0.0; 3], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0], [6.0, 0.0, 0.0]],
                false,
            )
            .unwrap(),
        );
        let mut processor = run(&mut doc, &["removeknot"]);
        assert_eq!(processor.preview(&doc, Some([3.9, 0.0, 0.0]), "").len(), 1);
        processor.click(&mut doc, [3.9, 0.0, 0.0]);
        processor.submit(&mut doc, "");
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap(),
            [[0.0; 3], [2.0, 0.0, 0.0], [6.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn a_line_keeps_its_two_points() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap());
        let mut processor = run(&mut doc, &["removeknot"]);
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        assert!(processor.is_running());
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            2
        );
    }
}
