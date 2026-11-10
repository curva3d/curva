//! `ArrayPolar`: copies objects around a centre, on the plane being drawn on.

use super::{ask_objects, ask_point, ghosts, plane_normal, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point, Similarity};

/// Most items in a polar array.
const MOST_ITEMS: usize = 10_000;
/// Most items previewed, so that a large array does not stall the window.
const MOST_PREVIEWED: usize = 200;
const FULL_TURN: f64 = 360.0;

/// The running command.
#[derive(Default)]
pub struct ArrayPolar {
    ids: Option<Vec<u64>>,
    centre: Option<Point>,
    items: Option<usize>,
}

fn valid_items(typed: f64) -> Option<usize> {
    (typed >= 2.0 && typed.fract() == 0.0 && typed <= MOST_ITEMS as f64).then_some(typed as usize)
}

/// The turn from one item to the next, in degrees. A full circle does not repeat its first item.
fn step_of(items: usize, fill: f64) -> f64 {
    if fill.abs() >= FULL_TURN {
        fill / items as f64
    } else {
        fill / (items - 1) as f64
    }
}

fn turns(centre: Point, axis: Point, items: usize, fill: f64) -> impl Iterator<Item = Similarity> {
    let step = step_of(items, fill);
    (1..items)
        .map(move |item| Similarity::rotation(centre, axis, (step * item as f64).to_radians()))
}

impl Command for ArrayPolar {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.arraypolar.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.arraypolar.centre", Vec::new());
        };
        match (self.centre, self.items, answer) {
            (None, None, Some(Answer::Point(centre))) => {
                self.centre = Some(centre);
                Step::Ask(Request::new("prompt.arraypolar.items", Kind::Number))
            }
            (Some(_), None, Some(Answer::Number(typed))) => {
                let Some(items) = valid_items(typed) else {
                    return Step::Failed(Message::new("error.bad_items"));
                };
                self.items = Some(items);
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.arraypolar.fill", Kind::Number)
                })
            }
            (Some(centre), Some(items), Some(answer @ (Answer::Number(_) | Answer::Empty))) => {
                let fill = if let Answer::Number(fill) = answer {
                    fill
                } else {
                    FULL_TURN
                };
                if fill == 0.0 || !fill.is_finite() || fill.abs() > FULL_TURN {
                    return Step::Failed(Message::new("error.bad_fill"));
                }
                let mut made = 0;
                for turn in turns(centre, plane_normal(doc), items, fill) {
                    made += doc.copy_transformed(&ids, &turn).len();
                }
                Step::Done(Some(Message::with("done.array", vec![made.to_string()])))
            }
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(ids), Some(centre)) = (&self.ids, self.centre) else {
            return Vec::new();
        };
        let (items, fill) = match self.items {
            Some(items) => (Some(items), live.number.unwrap_or(FULL_TURN)),
            None => (live.number.and_then(valid_items), FULL_TURN),
        };
        let Some(items) = items.filter(|_| fill != 0.0 && fill.abs() <= FULL_TURN) else {
            return Vec::new();
        };
        turns(centre, plane_normal(doc), items.min(MOST_PREVIEWED), fill)
            .flat_map(|turn| ghosts(doc, ids, |curve| curve.mapped(&turn)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn starts(doc: &Document) -> Vec<Point> {
        doc.state()
            .objects
            .iter()
            .map(|object| object.curve().unwrap().sample().unwrap()[0])
            .collect()
    }

    fn has(points: &[Point], wanted: Point) -> bool {
        points
            .iter()
            .any(|p| (0..3).all(|axis| (p[axis] - wanted[axis]).abs() < 1e-9))
    }

    #[test]
    fn enter_fills_a_full_circle_without_repeating_the_first() {
        let mut doc = selected_line();
        run(&mut doc, &["arraypolar", "0,0", "4", ""]);
        let found = starts(&doc);
        assert_eq!(found.len(), 4);
        assert!(has(&found, [0.0, 5.0, 0.0]));
        assert!(has(&found, [-5.0, 0.0, 0.0]));
        assert!(has(&found, [0.0, -5.0, 0.0]));
    }

    #[test]
    fn a_partial_fill_reaches_its_last_angle() {
        let mut doc = selected_line();
        run(&mut doc, &["arraypolar", "0,0", "3", "90"]);
        let found = starts(&doc);
        assert_eq!(found.len(), 3);
        assert!(has(&found, [0.0, 5.0, 0.0]));
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_line();
        run(&mut doc, &["arraypolar", "0,0", "4", "", "undo"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn bad_counts_and_fills_make_nothing() {
        for steps in [
            ["arraypolar", "0,0", "1", ""],
            ["arraypolar", "0,0", "2.5", ""],
            ["arraypolar", "0,0", "3", "0"],
            ["arraypolar", "0,0", "3", "400"],
        ] {
            let mut doc = selected_line();
            run(&mut doc, &steps);
            assert_eq!(doc.state().objects.len(), 1, "{steps:?}");
        }
    }

    #[test]
    fn the_preview_follows_a_typed_number_of_items() {
        let mut doc = selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["arraypolar", "0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert_eq!(processor.preview(&doc, None, "4").len(), 3);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
