//! `RemoveKnot`, as in Rhino: takes away the knot nearest where a curve is clicked, the control
//! points near it moving to keep the shape as close as they can (#736).

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Spline};

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

/// The curve under `at` without the inner knot nearest `at`.
fn removed_at(doc: &Document, at: Point) -> Option<Result<(u64, Curve), String>> {
    let (id, curve) = doc.editable_curve_at(at, ON_CURVE)?;
    let spline = match Spline::of(&curve).and_then(Spline::clamped) {
        Ok(spline) => spline,
        Err(error) => return Some(Err(error.to_string())),
    };
    let picked = spline.closest(at);
    let (start, end) = spline.domain();
    let nearest = spline
        .knots
        .iter()
        .copied()
        .filter(|k| *k > start && *k < end)
        .min_by(|a, b| (a - picked).abs().total_cmp(&(b - picked).abs()))?;
    // Taken out whatever it changes, the rest refitted, as Rhino removes it.
    Some(
        curve
            .without_knot(nearest)
            .map(|made| (id, made))
            .map_err(|e| e.to_string()),
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
    fn the_knot_goes_and_the_curve_is_refitted_as_in_rhino() {
        let mut doc = Document::default();
        let points = vec![
            [0.0, 0.0, 0.0],
            [2.0, 4.0, 0.0],
            [4.0, 0.0, 0.0],
            [6.0, 4.0, 0.0],
            [8.0, 0.0, 0.0],
            [10.0, 4.0, 0.0],
        ];
        doc.add(Curve::control_point(points, 3, false).unwrap());
        let mut processor = run(&mut doc, &["removeknot"]);
        processor.click(&mut doc, [23.0 / 6.0, 5.0 / 3.0, 0.0]);
        processor.submit(&mut doc, "");
        let curve = doc.state().objects[0].curve().unwrap();
        let control = curve.control_points().unwrap();
        // Rhino 8: five control points, length 12.4849.
        assert_eq!(control.len(), 5, "{control:?}");
        assert!(
            (curve.length().unwrap() - 12.4849).abs() < 1e-3,
            "{} {control:?}",
            curve.length().unwrap()
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
