//! `Array`: copies objects into rows, columns and levels.

use super::{ask_objects, ghosts};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Curve, Point};

/// Most copies an array may make, so that a slip of the keyboard cannot freeze Curva.
const MOST_COPIES: usize = 10_000;
/// Zero copies in a direction means nothing is arrayed that way, the same as one.
/// Most places previewed, so that a large array does not stall the window.
const MOST_PREVIEWED: usize = 200;

const COUNT_KEYS: [&str; 3] = [
    "prompt.array.count_x",
    "prompt.array.count_y",
    "prompt.array.count_z",
];
const SPACING_KEYS: [&str; 3] = [
    "prompt.array.spacing_x",
    "prompt.array.spacing_y",
    "prompt.array.spacing_z",
];

/// The running command.
#[derive(Default)]
pub struct Array {
    ids: Option<Vec<u64>>,
    counts: Vec<usize>,
    spacings: Vec<f64>,
}

/// Every offset of the array except the first, which is where the originals already are.
fn offsets(counts: [usize; 3], spacing: [f64; 3]) -> impl Iterator<Item = Point> {
    let [nx, ny, nz] = counts;
    (0..nz)
        .flat_map(move |k| (0..ny).flat_map(move |j| (0..nx).map(move |i| (i, j, k))))
        .skip(1)
        .map(move |(i, j, k)| {
            [
                i as f64 * spacing[0],
                j as f64 * spacing[1],
                k as f64 * spacing[2],
            ]
        })
}

fn valid_count(typed: f64) -> Option<usize> {
    (typed >= 0.0 && typed.fract() == 0.0 && typed <= MOST_COPIES as f64)
        .then_some((typed as usize).max(1))
}

impl Array {
    /// The next axis whose spacing is needed, skipping axes with only one place.
    fn spacing_needed(&self) -> Option<usize> {
        (self.spacings.len()..3).find(|axis| self.counts[*axis] > 1)
    }

    fn ask(key: &'static str) -> Step {
        Step::Ask(Request::new(key, Kind::Number))
    }

    fn next(&mut self, doc: &mut Document, ids: &[u64]) -> Step {
        if self.counts.len() < 3 {
            return Self::ask(COUNT_KEYS[self.counts.len()]);
        }
        if let Some(axis) = self.spacing_needed() {
            self.spacings.resize(axis, 0.0);
            return Self::ask(SPACING_KEYS[axis]);
        }
        self.spacings.resize(3, 0.0);
        let counts = [self.counts[0], self.counts[1], self.counts[2]];
        let spacing = [self.spacings[0], self.spacings[1], self.spacings[2]];
        let mut made = 0;
        for by in offsets(counts, spacing) {
            made += doc.copy(ids, by).len();
        }
        Step::Done(Some(Message::with("done.array", vec![made.to_string()])))
    }
}

impl Command for Array {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.array.objects");
            }
            self.ids = Some(chosen.clone());
            return self.next(doc, &chosen);
        };
        let Some(Answer::Number(typed)) = answer else {
            return super::unexpected();
        };
        if self.counts.len() < 3 {
            let Some(count) = valid_count(typed) else {
                return Step::Failed(Message::new("error.bad_count"));
            };
            self.counts.push(count);
            let places: usize = self.counts.iter().product();
            if places.saturating_mul(ids.len()) > MOST_COPIES {
                return Step::Failed(Message::new("error.too_many_copies"));
            }
        } else {
            self.spacings.push(typed);
        }
        self.next(doc, &ids)
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let mut counts = [1; 3];
        counts[..self.counts.len()].copy_from_slice(&self.counts);
        let mut spacing = [0.0; 3];
        spacing[..self.spacings.len()].copy_from_slice(&self.spacings);
        match (self.counts.len(), live.number) {
            (asked @ 0..=2, Some(typed)) => {
                if let Some(count) = valid_count(typed) {
                    counts[asked] = count;
                }
            }
            (3, Some(typed)) => {
                if let Some(axis) = self.spacing_needed() {
                    spacing[axis] = typed;
                }
            }
            _ => {}
        }
        offsets(counts, spacing)
            .take(MOST_PREVIEWED)
            .flat_map(|by| ghosts(doc, ids, |curve| curve.translated(by)))
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
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
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

    #[test]
    fn makes_rows_and_columns_at_the_right_spacing() {
        let mut doc = selected_line();
        run(&mut doc, &["array", "2", "3", "1", "10", "5"]);
        let found = starts(&doc);
        assert_eq!(found.len(), 6);
        assert!(found.contains(&[10.0, 10.0, 0.0]));
        assert!(found.contains(&[0.0, 5.0, 0.0]));
    }

    #[test]
    fn makes_levels_too() {
        let mut doc = selected_line();
        run(&mut doc, &["array", "1", "1", "3", "4"]);
        assert!(starts(&doc).contains(&[0.0, 0.0, 8.0]));
        assert_eq!(starts(&doc).len(), 3);
    }

    #[test]
    fn one_place_on_every_axis_adds_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["array", "1", "1", "1"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }

    #[test]
    fn it_is_one_undo_step() {
        let mut doc = selected_line();
        run(&mut doc, &["array", "3", "1", "1", "2", "undo"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn zero_copies_in_a_direction_arrays_only_the_others() {
        let mut doc = selected_line();
        run(&mut doc, &["array", "0", "3", "0", "5"]);
        let found = starts(&doc);
        assert_eq!(found.len(), 3);
        assert!(found.contains(&[0.0, 10.0, 0.0]));
    }

    #[test]
    fn zero_copies_everywhere_adds_nothing() {
        let mut doc = selected_line();
        let processor = run(&mut doc, &["array", "0", "0", "0"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }
    #[test]
    fn a_bad_count_makes_nothing() {
        for count in ["2.5", "-1"] {
            let mut doc = selected_line();
            let processor = run(&mut doc, &["array", count]);
            assert_eq!(doc.state().objects.len(), 1);
            assert!(!processor.is_running());
        }
    }

    #[test]
    fn too_many_copies_are_refused() {
        let mut doc = selected_line();
        run(&mut doc, &["array", "1000", "1000", "1", "1", "1"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn the_preview_follows_a_typed_spacing_without_changing_the_model() {
        let mut doc = selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["array", "3", "1", "1"] {
            processor.submit(&mut doc, typed);
        }
        let shown = processor.preview(&doc, None, "10");
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[1].sample().unwrap()[0], [20.0, 0.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
