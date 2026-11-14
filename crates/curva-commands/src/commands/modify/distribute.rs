//! The `Distribute` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Point;

/// The lowest and highest corners of a shape.
fn bounds_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape {
        // An instance's extent needs its definition; commands that use this leave it out.
        Shape::Instance(_) => None,
        Shape::Body(body) => body.bounds().ok(),
        Shape::Curve(curve) => {
            let points = curve.sample().ok()?;
            let first = *points.first()?;
            Some(points.iter().fold((first, first), |(mut lo, mut hi), p| {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
                (lo, hi)
            }))
        }
    }
}

/// `Distribute`: spaces objects evenly along `X` (the default), `Y` or `Z`, the first and last
/// staying put: their centres evenly apart, or with `Gap`, the gaps between them.
#[derive(Default)]
pub struct Distribute;

impl Recipe for Distribute {
    const NAME: &'static str = "distribute";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("X"),
        OptionDef::mode("Y"),
        OptionDef::mode("Z"),
        OptionDef::toggle("Gap", false),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.distribute.objects")
                .at_least(3)
                .offering(&["X", "Y", "Z", "Gap"]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let axis = match cx.mode {
            Some("Y") => 1,
            Some("Z") => 2,
            _ => 0,
        };
        let mut items: Vec<(u64, Shape, f64, f64)> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| objects(cx, 0).contains(&o.id))
            .filter_map(|o| {
                let (lo, hi) = bounds_of(&o.shape)?;
                Some((o.id, o.shape.clone(), lo[axis], hi[axis]))
            })
            .collect();
        if items.len() < 3 {
            return Ok(Made::default());
        }
        items.sort_by(|a, b| (a.2 + a.3).total_cmp(&(b.2 + b.3)));
        let count = items.len();
        let mut made = Made::default();
        if cx.toggle("Gap") {
            let span = items[count - 1].3 - items[0].2;
            let widths: f64 = items.iter().map(|i| i.3 - i.2).sum();
            let gap_size = (span - widths) / (count - 1) as f64;
            let mut at = items[0].3 + gap_size;
            for (id, shape, lo, hi) in &items[1..count - 1] {
                let mut by = [0.0; 3];
                by[axis] = at - lo;
                made.replace.push((*id, shape.translated(by)));
                at += hi - lo + gap_size;
            }
        } else {
            let centre = |i: &(u64, Shape, f64, f64)| (i.2 + i.3) / 2.0;
            let (first, last) = (centre(&items[0]), centre(&items[count - 1]));
            for (index, item) in items.iter().enumerate().take(count - 1).skip(1) {
                let wanted = first + (last - first) * index as f64 / (count - 1) as f64;
                let mut by = [0.0; 3];
                by[axis] = wanted - centre(item);
                made.replace.push((item.0, item.1.translated(by)));
            }
        }
        made.message = Some(Message::with("done.distribute", vec![count.to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn distribute_spaces_centres_and_gaps() {
        let mut doc = Document::default();
        let ids: Vec<u64> = [0.0, 1.0, 9.0, 10.0]
            .iter()
            .map(|x| doc.add(Curve::line([*x, 0.0, 0.0], [x + 1.0, 0.0, 0.0]).unwrap()))
            .collect();
        doc.select(&ids);
        run(&mut doc, &["distribute"]);
        let starts: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .map(|o| o.curve().unwrap().sample().unwrap()[0][0])
            .collect();
        for (got, want) in starts.iter().zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0]) {
            assert!((got - want).abs() < 1e-9, "{starts:?}");
        }
    }
}
