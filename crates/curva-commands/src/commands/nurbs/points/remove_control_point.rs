//! `RemoveControlPoint`: takes away picked control points, or the one nearest each click.

use crate::commands::{geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Grip};
use curva_kernel::{Curve, Point, gap};

/// The running command.
#[derive(Default)]
pub struct RemoveControlPoint {
    removed: usize,
}

fn ask_point(key: &'static str) -> Step {
    Step::Ask(Request {
        allow_empty: true,
        ..Request::new(key, Kind::Point)
    })
}

/// The control point on show nearest `at`.
fn nearest_grip(doc: &Document, at: Point) -> Option<Grip> {
    doc.shown_grips()
        .into_iter()
        .min_by(|a, b| gap(a.1, at).total_cmp(&gap(b.1, at)))
        .map(|(grip, _)| grip)
}

/// Replaces the curves and lets go of the picked points, whose places have shifted.
fn apply(doc: &mut Document, made: Vec<(u64, Curve)>) {
    for (id, curve) in made {
        doc.replace(id, curve);
    }
    doc.select_grips(&[]);
}

impl Command for RemoveControlPoint {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => {
                let picked = doc.grip_selection();
                if !picked.is_empty() {
                    // As in Rhino, points picked beforehand go at once.
                    return match doc.grips_removed(&picked) {
                        Ok(made) => {
                            apply(doc, made);
                            Step::Done(Some(Message::with(
                                "done.removecontrolpoint",
                                vec![picked.len().to_string()],
                            )))
                        }
                        Err(error) => geometry_failed(&error),
                    };
                }
                if doc.shown_grips().is_empty() {
                    return Step::Failed(Message::new("error.no_points_shown"));
                }
                ask_point("prompt.removecontrolpoint.point")
            }
            Some(Answer::Empty) => Step::Done(Some(Message::with(
                "done.removecontrolpoint",
                vec![self.removed.to_string()],
            ))),
            Some(Answer::Point(at)) => {
                let Some(grip) = nearest_grip(doc, at) else {
                    return Step::Done(Some(Message::with(
                        "done.removecontrolpoint",
                        vec![self.removed.to_string()],
                    )));
                };
                match doc.grips_removed(&[grip]) {
                    Ok(made) => {
                        apply(doc, made);
                        self.removed += 1;
                        ask_point("prompt.removecontrolpoint.point")
                    }
                    Err(error) => Step::Ask(Request {
                        prompt: Message::with(
                            "prompt.removecontrolpoint.refused",
                            vec![error.to_string()],
                        ),
                        allow_empty: true,
                        ..Request::new("prompt.removecontrolpoint.refused", Kind::Point)
                    }),
                }
            }
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(grip) = live.cursor.and_then(|at| nearest_grip(doc, at)) else {
            return Vec::new();
        };
        doc.grips_removed(&[grip])
            .map(|made| made.into_iter().map(|(_, curve)| curve).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn zigzag(doc: &mut Document) -> u64 {
        doc.add(
            Curve::polyline(
                vec![[0.0; 3], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0], [3.0, 2.0, 0.0]],
                false,
            )
            .unwrap(),
        )
    }

    #[test]
    fn picked_points_go_at_once() {
        let mut doc = Document::default();
        let id = zigzag(&mut doc);
        doc.points_on(&[id]);
        doc.select_grips(&[(id, 1), (id, 2)]);
        let mut processor = run(&mut doc, &["removecontrolpoint"]);
        assert!(!processor.is_running());
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap(),
            [[0.0; 3], [3.0, 2.0, 0.0]]
        );
        processor.submit(&mut doc, "undo");
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn without_picked_points_each_click_takes_the_nearest() {
        let mut doc = Document::default();
        let id = zigzag(&mut doc);
        doc.points_on(&[id]);
        let mut processor = run(&mut doc, &["removecontrolpoint"]);
        assert_eq!(processor.preview(&doc, Some([1.1, 1.9, 0.0]), "").len(), 1);
        processor.submit(&mut doc, "1.1,1.9,0");
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
    fn points_must_be_on_show() {
        let mut doc = Document::default();
        zigzag(&mut doc);
        let processor = run(&mut doc, &["removecontrolpoint"]);
        assert!(!processor.is_running());
        assert_eq!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .control_points()
                .unwrap()
                .len(),
            4
        );
    }
}
