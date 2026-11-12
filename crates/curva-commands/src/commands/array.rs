//! `Array`, as in Rhino: copies objects into rows, columns and levels: the number of places in X,
//! Y and Z, then the spacing for each direction with more than one. Rhino's unit cell picked with
//! the mouse, and its `Mode`, `Number`, `Spacing` and `Preview` options at the end, are still to
//! come (`docs/commands/rhino-reference.md`).

use super::{Change, changed};
use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_kernel::Point;

/// Most copies an array may make, so that a slip of the keyboard cannot freeze Curva.
const MOST_COPIES: usize = 10_000;
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

/// The recipe.
#[derive(Default)]
pub struct Array;

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

/// A typed number of places, if it is one: zero counts as one, as nothing is arrayed that way.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn valid_count(typed: f64) -> Option<usize> {
    (typed >= 0.0 && typed.fract() == 0.0 && typed <= MOST_COPIES as f64)
        .then_some((typed as usize).max(1))
}

/// The numbers of places given so far, one for each direction not yet given.
fn counts(cx: &Context<'_>) -> [usize; 3] {
    std::array::from_fn(|axis| {
        cx.values
            .get(1 + axis)
            .and_then(Value::number)
            .and_then(valid_count)
            .unwrap_or(1)
    })
}

/// The directions that need a spacing: those with more than one place.
fn spaced(counts: [usize; 3]) -> Vec<usize> {
    (0..3).filter(|axis| counts[*axis] > 1).collect()
}

impl Recipe for Array {
    const NAME: &'static str = "array";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.array.objects")];
        if cx.values.is_empty() {
            return steps;
        }
        steps.extend(COUNT_KEYS.iter().map(|key| StepDef::number(key)));
        if cx.values.len() >= 4 {
            steps.extend(
                spaced(counts(cx))
                    .into_iter()
                    .map(|axis| StepDef::number(SPACING_KEYS[axis])),
            );
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        for value in cx.values.iter().skip(1).take(3) {
            if value.number().and_then(valid_count).is_none() {
                return Err(Message::new("error.bad_count"));
            }
        }
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(());
        };
        let places: usize = counts(cx).iter().product();
        if places.saturating_mul(ids.len()) > MOST_COPIES {
            return Err(Message::new("error.too_many_copies"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let counts = counts(cx);
        let mut spacing = [0.0; 3];
        for (index, axis) in spaced(counts).into_iter().enumerate() {
            spacing[axis] = cx
                .values
                .get(4 + index)
                .and_then(Value::number)
                .unwrap_or(0.0);
        }
        let most = if cx.previewing {
            MOST_PREVIEWED
        } else {
            usize::MAX
        };
        let changes: Vec<Change> = offsets(counts, spacing)
            .take(most)
            .map(Change::Move)
            .collect();
        let mut made = changed(cx, ids, &changes, true, |_, _| None);
        made.message = Some(Message::with(
            "done.array",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
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
