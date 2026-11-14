//! The `Align` command.

use crate::Message;
use crate::commands::Change;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Point, dot};

/// The lowest and highest corners around an object.
fn bounds_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape {
        // An instance's extent needs its definition; commands that use this leave it out.
        Shape::Instance(_) => None,
        Shape::Body(body) => body.bounds().ok(),
        Shape::Curve(curve) => {
            let points = curve.sample().ok()?;
            let first = *points.first()?;
            Some(points.iter().fold((first, first), |(lo, hi), p| {
                (
                    std::array::from_fn(|i| lo[i].min(p[i])),
                    std::array::from_fn(|i| hi[i].max(p[i])),
                )
            }))
        }
    }
}

/// Which value of an extent along an axis to line up: its start, its end, or its middle.
type Extent = fn((f64, f64)) -> f64;

const ALIGNS: [&str; 6] = [
    "Bottom",
    "Top",
    "Left",
    "Right",
    "HorizontalCenter",
    "VerticalCenter",
];

/// `Align`: lines objects up on the plane being drawn on: their bottoms, tops, left or right
/// sides, or centres, on those of all of them.
#[derive(Default)]
pub struct Align;

impl Recipe for Align {
    const NAME: &'static str = "align";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::finish("Bottom"),
        OptionDef::finish("Top"),
        OptionDef::finish("Left"),
        OptionDef::finish("Right"),
        OptionDef::finish("HorizontalCenter"),
        OptionDef::finish("VerticalCenter"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.align.objects")
                .at_least(2)
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.align.how").offering(&ALIGNS));
        }
        steps
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
            .filter_map(|o| Some((o.id, bounds_of(&o.shape)?)))
            .collect();
        // Each object's extent along the plane's X or Y, and the target on all of them.
        let along = |axis: Point, (lo, hi): (Point, Point)| {
            let (a, b) = (dot(lo, axis), dot(hi, axis));
            (a.min(b), a.max(b))
        };
        let (axis, pick): (Point, Extent) = match how {
            "Bottom" => (y, |(lo, _)| lo),
            "Top" => (y, |(_, hi)| hi),
            "Left" => (x, |(lo, _)| lo),
            "Right" => (x, |(_, hi)| hi),
            "HorizontalCenter" => (x, |(lo, hi)| (lo + hi) / 2.0),
            _ => (y, |(lo, hi)| (lo + hi) / 2.0),
        };
        let values: Vec<f64> = objects.iter().map(|(_, b)| pick(along(axis, *b))).collect();
        let target = match how {
            "Top" | "Right" => values.iter().copied().fold(f64::MIN, f64::max),
            "Bottom" | "Left" => values.iter().copied().fold(f64::MAX, f64::min),
            _ => values.iter().sum::<f64>() / values.len().max(1) as f64,
        };
        let mut made = Made::default();
        for ((id, _), value) in objects.iter().zip(values) {
            let by = axis.map(|v| v * (target - value));
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

    #[test]
    fn align_lines_up_bottoms() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([3.0, 5.0, 0.0], [4.0, 6.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["align", "bottom"]);
        let lowest = doc.state().objects[1].curve().unwrap().sample().unwrap()[0][1];
        assert!(lowest.abs() < 1e-9, "{lowest}");
    }
}
