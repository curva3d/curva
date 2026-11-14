//! The `CageEdit` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value, run_points};
use curva_document::Shape;
use curva_kernel::{Curve, Point, gap};

/// `CageEdit`: moves the corners of a box around objects, the objects following inside it:
/// pick a corner (the nearest is taken) and where it goes, as often as wanted; Enter applies it.
#[derive(Default)]
pub struct CageEdit;

impl Recipe for CageEdit {
    const NAME: &'static str = "cageedit";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.cageedit.objects")];
        if cx.values.is_empty() {
            return steps;
        }
        let clicks = &cx.values[1..];
        let prompt = |i: usize| {
            if i.is_multiple_of(2) {
                "prompt.cageedit.corner"
            } else {
                "prompt.cageedit.to"
            }
        };
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point(prompt(i));
            steps.push(if i.is_multiple_of(2) && i >= 2 {
                step.optional()
            } else {
                step
            });
        }
        if clicks.last() != Some(&Value::Empty) {
            let i = clicks.len();
            let mut step = StepDef::point(prompt(i));
            if let Some(Value::Point(last)) = clicks.last() {
                step = step.from(*last);
            }
            steps.push(if i.is_multiple_of(2) && i >= 2 {
                step.optional()
            } else {
                step
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        // The box around the objects.
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
        {
            let points: Vec<Point> = match &object.shape {
                Shape::Curve(c) => c.sample().unwrap_or_default(),
                Shape::Body(b) => b.bounds().map(|(l, h)| vec![l, h]).unwrap_or_default(),
                Shape::Instance(_) => Vec::new(),
                Shape::Annotation(_) => object
                    .shape
                    .bounds()
                    .map(|(l, h)| vec![l, h])
                    .unwrap_or_default(),
            };
            for p in points {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        if low[0] > high[0] {
            return Ok(Made::default());
        }
        let size: Point = std::array::from_fn(|k| (high[k] - low[k]).max(1e-9));
        let corner_at = |c: usize| -> Point {
            std::array::from_fn(|k| if c >> k & 1 == 1 { high[k] } else { low[k] })
        };
        let mut corners: Vec<Point> = (0..8).map(corner_at).collect();
        let clicks = run_points(cx);
        for pair in clicks.chunks(2).filter(|p| p.len() == 2) {
            let nearest = (0..8)
                .min_by(|a, b| gap(corner_at(*a), pair[0]).total_cmp(&gap(corner_at(*b), pair[0])))
                .unwrap_or(0);
            corners[nearest] = pair[1];
        }
        let map = move |p: Point| {
            let t: Point = std::array::from_fn(|k| (p[k] - low[k]) / size[k]);
            let mut out = [0.0; 3];
            for (c, corner) in corners.iter().enumerate() {
                let w: f64 = (0..3)
                    .map(|k| if c >> k & 1 == 1 { t[k] } else { 1.0 - t[k] })
                    .product();
                for k in 0..3 {
                    out[k] += w * corner[k];
                }
            }
            out
        };
        if cx.values.last() != Some(&Value::Empty) && cx.previewing {
            let guides = (0..8)
                .flat_map(|c| {
                    (0..3)
                        .filter(move |k| c >> k & 1 == 0)
                        .map(move |k| (c, c | 1 << k))
                })
                .filter_map(|(a, b)| Curve::line(map(corner_at(a)), map(corner_at(b))).ok())
                .collect();
            return Ok(Made {
                guides,
                ..Made::default()
            });
        }
        deform_all(cx, ids, &map, "done.cageedit")
    }
}
