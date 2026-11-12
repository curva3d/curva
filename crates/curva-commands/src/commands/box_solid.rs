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

/// The base: its first corner and its width and depth along the plane, if it is not flat.
fn base(cx: &Context<'_>) -> Option<(Point, [f64; 2])> {
    let first = cx.values.first().and_then(Value::point)?;
    let other = cx.values.get(1).and_then(Value::point)?;
    let corner = if cx.mode == Some(CENTER) {
        std::array::from_fn(|i| 2.0 * first[i] - other[i])
    } else {
        first
    };
    let [x, y] = cx.plane();
    let across = sub(other, corner);
    let sides = [dot(across, x), dot(across, y)];
    sides
        .iter()
        .all(|side| side.abs() >= SMALLEST_SIDE)
        .then_some((corner, sides))
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
            steps.push(StepDef::point("prompt.box.other").from(first).offering(&[]));
            if cx.values.len() >= 2 {
                steps.push(
                    StepDef::along("prompt.box.height", first, up(cx))
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
        if let Some(Value::Number(high)) = cx.values.get(2)
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
        let high = match cx.values.get(2) {
            Some(Value::Number(high)) => *high,
            // Enter at the height uses the base's width, as in Rhino.
            Some(Value::Empty) => wide.abs(),
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
    use super::super::run;
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
        // Enter: the height is the width, 4, grown both ways.
        assert!(
            close(low[2], -4.0) && close(high[2], 4.0),
            "{low:?} {high:?}"
        );
    }
}
