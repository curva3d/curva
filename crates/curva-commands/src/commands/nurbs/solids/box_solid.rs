//! `Box`, as in Rhino: two corners of its base on the plane being drawn on, then a height that
//! follows the mouse, snaps included. Option `Center` takes the base's centre first. At the height,
//! `BothSides` grows the box both ways and Enter uses the base's width. Rhino's `Diagonal`,
//! `3Point` and `Vertical` ways are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point, cross, dot, sub};

/// Shortest side treated as real, in millimetres.
const SMALLEST_SIDE: f64 = 1e-9;
const CENTER: &str = "Center";
const BOTH_SIDES: &str = "BothSides";

/// The recipe.
#[derive(Default)]
pub struct BoxSolid;

fn plus(p: Point, direction: Point, amount: f64) -> Point {
    std::array::from_fn(|k| p[k] + direction[k] * amount)
}

/// The base: its first corner and its sides along the plane's X and Y, if it is not flat. The
/// second answer is the other corner, or a typed length followed by the width (Enter: the length),
/// as in Rhino.
pub(crate) fn base(cx: &Context<'_>) -> Option<(Point, [f64; 2])> {
    let first = cx.values.first().and_then(Value::point)?;
    let [x, y] = cx.plane();
    let (corner, sides) = match cx.values.get(1)? {
        Value::Number(length) => {
            let width = match cx.values.get(2) {
                Some(Value::Number(width)) => *width,
                _ => *length,
            };
            let corner = if cx.mode == Some(CENTER) {
                plus(plus(first, x, -length / 2.0), y, -width / 2.0)
            } else {
                first
            };
            (corner, [*length, width])
        }
        other => {
            let other = other.point()?;
            let corner = if cx.mode == Some(CENTER) {
                std::array::from_fn(|i| 2.0 * first[i] - other[i])
            } else {
                first
            };
            let across = sub(other, corner);
            (corner, [dot(across, x), dot(across, y)])
        }
    };
    sides
        .iter()
        .all(|side| side.abs() >= SMALLEST_SIDE)
        .then_some((corner, sides))
}

/// Where the height is among the answers: after the other corner, or after the length and width.
pub(crate) fn height_at(cx: &Context<'_>) -> usize {
    if matches!(cx.values.get(1), Some(Value::Number(_))) {
        3
    } else {
        2
    }
}

fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    cross(x, y)
}

impl Recipe for BoxSolid {
    const NAME: &'static str = "box";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(CENTER),
        OptionDef::toggle(BOTH_SIDES, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.box.first").offering(&[CENTER])];
        if let Some(first) = cx.values.first().and_then(Value::point) {
            // The other corner, or a typed length, as in Rhino ("Other corner of base or length").
            steps.push(
                StepDef::point_or_number("prompt.box.other")
                    .from(first)
                    .offering(&[])
                    .corner(),
            );
            if let Some(Value::Number(_)) = cx.values.get(1) {
                // "Width. Press Enter to use length".
                let [_, y] = cx.plane();
                steps.push(StepDef::along("prompt.box.width", first, y).optional());
                if cx.values.len() > 2 {
                    steps.push(
                        StepDef::along("prompt.box.height", first, up(cx))
                            .optional()
                            .offering(&[BOTH_SIDES]),
                    );
                }
            } else if let Some(other) = cx.values.get(1).and_then(Value::point) {
                // The height is measured up from the corner just clicked, where the mouse is,
                // as in Rhino (#484).
                steps.push(
                    StepDef::along("prompt.box.height", other, up(cx))
                        .optional()
                        .offering(&[BOTH_SIDES]),
                );
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.values.len() >= 2 && base(cx).is_none() {
            return Err(Message::new("error.flat_rectangle"));
        }
        if let Some(Value::Number(high)) = cx.values.get(height_at(cx))
            && high.abs() < SMALLEST_SIDE
        {
            return Err(Message::new("error.flat_box"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((corner, [wide, deep])) = base(cx) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let high = match cx.values.get(height_at(cx)) {
            Some(Value::Number(high)) => *high,
            // Enter at the height uses the base's width (its side along Y), as in Rhino.
            Some(Value::Empty) => deep.abs(),
            _ => 0.0,
        };
        if high.abs() < SMALLEST_SIDE {
            // Still choosing the base, or no height yet: show the base flat.
            let ring = [
                corner,
                plus(corner, x, wide),
                plus(plus(corner, x, wide), y, deep),
                plus(corner, y, deep),
            ];
            let guides = (0..4)
                .filter_map(|i| Curve::line(ring[i], ring[(i + 1) % 4]).ok())
                .collect();
            return Ok(Made {
                guides,
                ..Made::default()
            });
        }
        let (start, size) = if cx.toggle(BOTH_SIDES) {
            (plus(corner, up(cx), -high), [wide, deep, 2.0 * high])
        } else {
            (corner, [wide, deep, high])
        };
        let body = Body::cuboid_on(start, x, y, size)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.box")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn bounds(doc: &Document) -> ([f64; 3], [f64; 3]) {
        doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap()
    }

    #[test]
    fn two_corners_and_a_height_make_a_solid_box() {
        let mut doc = Document::default();
        let mut processor = run(&mut doc, &["box", "0,0", "10,20"]);
        assert_eq!(processor.preview(&doc, None, "5").len(), 12);
        processor.submit(&mut doc, "5");
        let body = doc.state().objects[0].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), 1000.0));
        let (low, high) = body.bounds().unwrap();
        assert!(close(low[2], 0.0) && close(high[2], 5.0));
        processor.submit(&mut doc, "undo");
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn the_height_is_measured_up_from_the_corner_just_clicked() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["box", "0,0", "10,20"]);
        let request = processor.current_request().unwrap();
        let (origin, up) = request.along.unwrap();
        assert!(close(origin[0], 10.0) && close(origin[1], 20.0) && close(origin[2], 0.0));
        assert!(close(up[2], 1.0));
    }

    #[test]
    fn a_negative_height_goes_below_the_plane() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "0,0", "2,2", "-3"]);
        let (low, high) = bounds(&doc);
        assert!(close(low[2], -3.0) && close(high[2], 0.0));
    }

    #[test]
    fn a_flat_base_is_refused() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["box", "0,0", "5,0"]);
        assert!(!processor.is_running());
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn a_box_can_be_moved_rotated_and_copied() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "0,0", "2,3", "4"]);
        let id = doc.state().objects[0].id;
        doc.select(&[id]);
        run(&mut doc, &["rotate", "0,0", "30"]);
        run(&mut doc, &["move", "0,0", "10,0"]);
        run(&mut doc, &["copy", "0,0", "0,10", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        for object in &doc.state().objects {
            assert!(close(object.shape.body().unwrap().volume().unwrap(), 24.0));
        }
    }

    #[test]
    fn center_takes_the_middle_of_the_base_first() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "center", "0,0", "2,3", "1"]);
        let (low, high) = bounds(&doc);
        assert!(
            close(low[0], -2.0)
                && close(low[1], -3.0)
                && close(high[0], 2.0)
                && close(high[1], 3.0)
        );
    }

    #[test]
    fn both_sides_and_enter_for_the_width() {
        let mut doc = Document::default();
        run(&mut doc, &["box", "0,0", "4,2", "bothsides", ""]);
        let (low, high) = bounds(&doc);
        // Enter: the height is the width, the side along Y (2), grown both ways, as Rhino 8 gives
        // (measured: corners 0,0 and 4,2 then Enter make 4 x 2 x 2).
        assert!(
            close(low[2], -2.0) && close(high[2], 2.0),
            "{low:?} {high:?}"
        );
    }

    #[test]
    fn a_typed_length_then_width_then_height_as_in_rhino() {
        // Measured in Rhino 8 (#526): 10, Enter, Enter gives 10 x 10 x 10; 10, 4, Enter gives
        // 10 x 4 x 4; 10, 4, 7 gives 10 x 4 x 7.
        for (typed, size) in [
            (&["10", "", ""][..], [10.0, 10.0, 10.0]),
            (&["10", "4", ""][..], [10.0, 4.0, 4.0]),
            (&["10", "4", "7"][..], [10.0, 4.0, 7.0]),
        ] {
            let mut doc = Document::default();
            let mut steps = vec!["box", "0,0"];
            steps.extend_from_slice(typed);
            run(&mut doc, &steps);
            let (low, high) = bounds(&doc);
            let found: Vec<f64> = (0..3).map(|i| high[i] - low[i]).collect();
            assert!(
                (0..3).all(|i| close(found[i], size[i])),
                "{typed:?}: {found:?}"
            );
        }
    }
}
