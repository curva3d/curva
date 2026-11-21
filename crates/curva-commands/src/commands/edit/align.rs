//! `Align`, as in Rhino 8 (measured, #527): objects lined up on the plane being drawn on. After the
//! objects, choose how: `Bottom`, `Top`, `Left`, `Right`, `HorizCenter` (centres on one horizontal
//! line), `VertCenter` (centres on one vertical line) or `Concentric` (both). Then the alignment
//! point; Enter aligns to the selection itself: its lowest bottom, highest top, leftmost left,
//! rightmost right, or the middle of all of them together.

use crate::Message;
use crate::commands::Change;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::{Document, Object, Shape};
use curva_kernel::{Point, dot};

/// Rhino's ways of aligning, by the names Rhino takes.
const ALIGNS: [&str; 7] = [
    "Bottom",
    "Concentric",
    "HorizCenter",
    "Left",
    "Right",
    "Top",
    "VertCenter",
];

/// `Align`.
#[derive(Default)]
pub struct Align;

/// An object's extent, a block's from what it draws.
fn bounds_of(doc: &Document, object: &Object) -> Option<(Point, Point)> {
    let Shape::Instance(instance) = &object.shape else {
        return object.shape.bounds();
    };
    doc.instance_shapes(instance)
        .iter()
        .filter_map(|(shape, _)| shape.bounds())
        .reduce(|(lo, hi), (a, b)| {
            (
                std::array::from_fn(|i| lo[i].min(a[i])),
                std::array::from_fn(|i| hi[i].max(b[i])),
            )
        })
}

/// What a way of aligning lines up along the plane's X and along its Y: for each, which end of
/// the extent (-1 the low end, 0 the middle, 1 the high end), or `None` when it leaves that way
/// alone.
fn lines_up(how: &str) -> [Option<i8>; 2] {
    match how {
        "Left" => [Some(-1), None],
        "Right" => [Some(1), None],
        "Bottom" => [None, Some(-1)],
        "Top" => [None, Some(1)],
        "HorizCenter" => [None, Some(0)],
        "VertCenter" => [Some(0), None],
        _ => [Some(0), Some(0)],
    }
}

/// The value of an extent `(lo, hi)` at `end`.
fn at_end((lo, hi): (f64, f64), end: i8) -> f64 {
    match end {
        -1 => lo,
        1 => hi,
        _ => f64::midpoint(lo, hi),
    }
}

impl Recipe for Align {
    const NAME: &'static str = "align";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("Bottom"),
        OptionDef::mode("Concentric"),
        OptionDef::mode("HorizCenter"),
        OptionDef::mode("Left"),
        OptionDef::mode("Right"),
        OptionDef::mode("Top"),
        OptionDef::mode("VertCenter"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.align.objects")
                .at_least(2)
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(if cx.mode.is_some() {
                // "Alignment point. Press Enter to auto align".
                StepDef::point("prompt.align.point")
                    .optional()
                    .offering(&ALIGNS)
            } else {
                StepDef::point("prompt.align.how").offering(&ALIGNS)
            });
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.values.len() > 1 && cx.mode.is_none() {
            return Err(Message::new("error.align_choose"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(how)) = (cx.values.first(), cx.mode) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let objects: Vec<(u64, (Point, Point))> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .filter_map(|o| Some((o.id, bounds_of(cx.doc, o)?)))
            .collect();
        let along = |axis: Point, (lo, hi): (Point, Point)| {
            let (a, b) = (dot(lo, axis), dot(hi, axis));
            (a.min(b), a.max(b))
        };
        let point = cx.values.get(1).and_then(Value::point);
        let mut moves: Vec<Point> = vec![[0.0; 3]; objects.len()];
        for (axis, end) in [x, y].into_iter().zip(lines_up(how)) {
            let Some(end) = end else {
                continue;
            };
            let extents: Vec<(f64, f64)> = objects.iter().map(|(_, b)| along(axis, *b)).collect();
            // The alignment point's, or the selection's own: all extents together.
            let target = point.map_or_else(
                || {
                    let all = extents
                        .iter()
                        .fold((f64::MAX, f64::MIN), |(lo, hi), (a, b)| {
                            (lo.min(*a), hi.max(*b))
                        });
                    at_end(all, end)
                },
                |p| dot(p, axis),
            );
            for (moved, extent) in moves.iter_mut().zip(&extents) {
                let by = target - at_end(*extent, end);
                for (k, m) in moved.iter_mut().enumerate() {
                    *m += axis[k] * by;
                }
            }
        }
        let mut made = Made::default();
        for ((id, _), by) in objects.iter().zip(moves) {
            let part =
                crate::commands::changed(cx, &[*id], &[Change::Move(by)], false, |_, _| None);
            made.replace.extend(part.replace);
        }
        made.message = Some(Message::with(
            "done.align",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    /// Rhino 8's measured case: lines (0,0)–(4,0) and (1,5)–(3,5) and a circle of radius 2 at
    /// (10,3), aligned with Enter; each object's extent afterwards as (min x, min y, max x, max y).
    fn aligned(how: &str, point: &str) -> Vec<[f64; 4]> {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([1.0, 5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let c = doc.add(Curve::circle([10.0, 3.0, 0.0], 2.0).unwrap());
        doc.select(&[a, b, c]);
        run(&mut doc, &["align", how, point]);
        doc.state()
            .objects
            .iter()
            .map(|o| {
                let (lo, hi) = o.shape.bounds().unwrap();
                [lo[0], lo[1], hi[0], hi[1]].map(|v| (v * 1000.0).round() / 1000.0)
            })
            .collect()
    }

    #[test]
    fn each_way_lines_up_as_in_rhino() {
        // Measured in Rhino 8 with _-Align <option> _Enter.
        let rhino: [(&str, [[f64; 4]; 3]); 7] = [
            (
                "left",
                [
                    [0.0, 0.0, 4.0, 0.0],
                    [0.0, 5.0, 2.0, 5.0],
                    [0.0, 1.0, 4.0, 5.0],
                ],
            ),
            (
                "right",
                [
                    [8.0, 0.0, 12.0, 0.0],
                    [10.0, 5.0, 12.0, 5.0],
                    [8.0, 1.0, 12.0, 5.0],
                ],
            ),
            (
                "top",
                [
                    [0.0, 5.0, 4.0, 5.0],
                    [1.0, 5.0, 3.0, 5.0],
                    [8.0, 1.0, 12.0, 5.0],
                ],
            ),
            (
                "bottom",
                [
                    [0.0, 0.0, 4.0, 0.0],
                    [1.0, 0.0, 3.0, 0.0],
                    [8.0, 0.0, 12.0, 4.0],
                ],
            ),
            (
                "horizcenter",
                [
                    [0.0, 2.5, 4.0, 2.5],
                    [1.0, 2.5, 3.0, 2.5],
                    [8.0, 0.5, 12.0, 4.5],
                ],
            ),
            (
                "vertcenter",
                [
                    [4.0, 0.0, 8.0, 0.0],
                    [5.0, 5.0, 7.0, 5.0],
                    [4.0, 1.0, 8.0, 5.0],
                ],
            ),
            (
                "concentric",
                [
                    [4.0, 2.5, 8.0, 2.5],
                    [5.0, 2.5, 7.0, 2.5],
                    [4.0, 0.5, 8.0, 4.5],
                ],
            ),
        ];
        for (how, expected) in rhino {
            assert_eq!(aligned(how, ""), expected, "{how}");
        }
    }

    #[test]
    fn an_alignment_point_sets_the_line() {
        let found = aligned("left", "20,0");
        assert!(
            found.iter().all(|e| (e[0] - 20.0).abs() < 1e-9),
            "{found:?}"
        );
    }
}
