//! `Offset`, as in Rhino: a copy of curves at a distance, on the side clicked. A number typed at
//! the side prompt sets `Distance`; `ThroughPoint` makes the copy pass through the point clicked;
//! `BothSides` makes one on each side, and `Cap` closes the two copies of an open curve into one, with
//! flat or round ends. Each copy follows its curve when that changes (`History`); a capped one does
//! not. Rhino's `Loose`, `Corner`, `Trim`, `Tolerance`, `InCPlane` and `OutputLayer` are still to
//! come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, KernelError, Point, cross, dot, gap, sub};

const DISTANCE: &str = "Distance";
const THROUGH_POINT: &str = "ThroughPoint";
const BOTH_SIDES: &str = "BothSides";
const CAP: &str = "Cap";
/// Rhino's ways of closing the two copies of an open curve.
const CAPS: [&str; 3] = ["None", "Flat", "Round"];

/// The recipe.
#[derive(Default)]
pub struct Offset;

impl Recipe for Offset {
    const NAME: &'static str = "offset";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::distance(DISTANCE, 1.0, false),
        OptionDef::mode(THROUGH_POINT),
        OptionDef::toggle(BOTH_SIDES, false),
        OptionDef::name(CAP, "None"),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == CAP).then(|| CAPS.map(str::to_owned).to_vec())
    }

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.offset.objects").offering(&[]),
            StepDef::point("prompt.offset.side").number_sets(DISTANCE),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(toward)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::point))
        else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let normal = cross(x, y);
        let failed =
            |e: curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
        let mut derived = Vec::new();
        for (source, curve) in curves_of(cx.doc, ids) {
            let distance = if cx.mode == Some(THROUGH_POINT) {
                gap(curve.closest_point(toward).map_err(failed)?, toward)
            } else {
                cx.number(DISTANCE)
            };
            let side = curve.side_of_point(normal, toward).map_err(failed)?;
            let sides: &[f64] = if cx.toggle(BOTH_SIDES) {
                &[1.0, -1.0]
            } else {
                &[side]
            };
            let cap = cx.name(CAP);
            if sides.len() == 2 && cap != "None" && !curve.is_closed().map_err(failed)? {
                // Both copies of an open curve closed into one, with flat or round ends (`Cap`).
                let one = curve
                    .offset_on_side(normal, distance, 1.0)
                    .map_err(failed)?;
                let other = curve
                    .offset_on_side(normal, distance, -1.0)
                    .map_err(failed)?;
                let ends = (cap == "Round").then_some([x, y]);
                derived.push(Derived {
                    shape: Shape::Curve(capped(&curve, &one, &other, ends).map_err(failed)?),
                    from: source,
                    history: None,
                });
                continue;
            }
            for side in sides {
                let copy = curve
                    .offset_on_side(normal, distance, *side)
                    .map_err(failed)?;
                derived.push(Derived {
                    shape: Shape::Curve(copy),
                    from: source,
                    history: Some(curva_document::Recipe::Offset {
                        source,
                        normal,
                        distance,
                        side: *side,
                    }),
                });
            }
        }
        let count = derived.len();
        Ok(Made {
            derived,
            message: Some(Message::with("done.offset", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

/// The two copies of an open `curve` joined into one closed curve: straight ends, or half circles
/// about the curve's ends when `round` gives the plane drawn on.
fn capped(
    curve: &Curve,
    one: &Curve,
    other: &Curve,
    round: Option<[Point; 2]>,
) -> Result<Curve, KernelError> {
    let ends = |c: &Curve| -> Result<(Point, Point), KernelError> {
        let path = c.sample()?;
        Ok((path[0], path[path.len() - 1]))
    };
    let path = curve.sample()?;
    let n = path.len();
    // Each end of the curve, and the way out of the curve there.
    let at_start = (path[0], sub(path[0], path[1]));
    let at_end = (path[n - 1], sub(path[n - 1], path[n - 2]));
    let (one_start, one_end) = ends(one)?;
    let (other_start, other_end) = ends(other)?;
    let cap = |(centre, out): (Point, Point), p: Point, q: Point| match round {
        None => Curve::line(p, q),
        Some([x, y]) => {
            // Counterclockwise from p, the half circle bulges where p turned a quarter points;
            // started from whichever end makes that the way out of the curve.
            let turned = cross(cross(x, y), sub(p, centre));
            if dot(turned, out) > 0.0 {
                Curve::arc(centre, p, q, x, y)
            } else {
                Curve::arc(centre, q, p, x, y)
            }
        }
    };
    let parts = [
        one.clone(),
        cap(at_end, one_end, other_end)?,
        other.clone(),
        cap(at_start, one_start, other_start)?,
    ];
    curva_kernel::join_all(&parts)?
        .into_iter()
        .next()
        .ok_or_else(|| KernelError("the capped copies did not join".to_owned()))
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn start() -> (Document, Processor) {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offset");
        (doc, processor)
    }

    fn last_start(doc: &Document) -> [f64; 3] {
        doc.state()
            .objects
            .last()
            .unwrap()
            .curve()
            .unwrap()
            .sample()
            .unwrap()[0]
    }

    #[test]
    fn a_click_offsets_to_that_side_at_the_current_distance() {
        let (mut doc, mut processor) = start();
        processor.click(&mut doc, [5.0, -9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2);
        assert_eq!(last_start(&doc), [0.0, -1.0, 0.0]);
    }

    #[test]
    fn a_typed_distance_is_used_and_the_prompt_asks_again() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "4");
        assert!(processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(last_start(&doc), [0.0, 4.0, 0.0]);
    }

    #[test]
    fn the_distance_is_remembered_for_the_next_offset() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "4");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        processor.submit(&mut doc, "offset");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
        assert_eq!(last_start(&doc), [0.0, 4.0, 0.0]);
    }

    #[test]
    fn a_distance_too_large_makes_nothing() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offset");
        processor.submit(&mut doc, "5");
        processor.click(&mut doc, [0.5, 0.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }

    #[test]
    fn the_preview_follows_the_side_and_a_typed_distance() {
        let (doc, processor) = start();
        let shown = processor.preview(&doc, Some([5.0, 9.0, 0.0]), "3");
        assert_eq!(shown[0].sample().unwrap()[0], [0.0, 3.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn through_point_and_both_sides() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "throughpoint");
        processor.click(&mut doc, [5.0, 2.5, 0.0]);
        assert_eq!(last_start(&doc), [0.0, 2.5, 0.0]);
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "bothsides");
        processor.click(&mut doc, [5.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn an_arc_offsets_to_both_sides_at_a_typed_distance() {
        // The owner's case (#513): an arc, BothSides, 10 typed at the side prompt.
        let mut doc = Document::default();
        let arc = Curve::arc(
            [0.0; 3],
            [0.0, 50.0, 0.0],
            [0.0, -50.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let id = doc.add(arc);
        doc.select(&[id]);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "offset");
        processor.submit(&mut doc, "bothsides");
        let shown = processor.preview(&doc, Some([-30.0, 0.0, 0.0]), "10");
        assert_eq!(shown.len(), 2, "both sides show");
        processor.submit(&mut doc, "10");
        processor.click(&mut doc, [-30.0, 0.0, 0.0]);
        let radii: Vec<f64> = doc.state().objects[1..]
            .iter()
            .map(|o| {
                let p = o.curve().unwrap().sample().unwrap()[0];
                (p[0] * p[0] + p[1] * p[1]).sqrt()
            })
            .collect();
        assert_eq!(radii.len(), 2, "{radii:?}");
        assert!(radii.iter().any(|r| (r - 40.0).abs() < 1e-6), "{radii:?}");
        assert!(radii.iter().any(|r| (r - 60.0).abs() < 1e-6), "{radii:?}");
    }

    #[test]
    fn cap_closes_both_copies_with_flat_or_round_ends() {
        for (cap, length) in [
            ("flat", 10.0 + 10.0 + 2.0 + 2.0),
            ("round", 20.0 + 2.0 * std::f64::consts::PI),
        ] {
            let (mut doc, mut processor) = start();
            processor.submit(&mut doc, "bothsides");
            processor.submit(&mut doc, "cap");
            processor.submit(&mut doc, cap);
            processor.click(&mut doc, [5.0, 9.0, 0.0]);
            assert_eq!(doc.state().objects.len(), 2, "{cap}: one closed curve");
            let made = doc.state().objects[1].curve().unwrap();
            assert!(made.is_closed().unwrap(), "{cap}");
            let found = made.length().unwrap();
            assert!((found - length).abs() < 1e-3, "{cap}: {found}");
        }
    }

    #[test]
    fn a_distance_of_zero_is_refused() {
        let (mut doc, mut processor) = start();
        processor.submit(&mut doc, "0");
        assert!(!processor.is_running());
        assert_eq!(doc.state().objects.len(), 1);
    }
}
