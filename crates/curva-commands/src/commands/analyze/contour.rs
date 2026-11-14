//! The `Contour` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::commands::support::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Point, dot, sub, unit};

/// `Contour`: sections every `distance` along a direction: a base point, a point setting the
/// direction, then the distance between the planes.
#[derive(Default)]
pub struct Contour;

impl Recipe for Contour {
    const NAME: &'static str = "contour";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.contour.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.contour.base"));
        }
        if let Some(a) = point(cx, 1) {
            steps.push(StepDef::point("prompt.contour.direction").from(a));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::number("prompt.contour.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(step)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if step <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let normal = unit(sub(b, a)).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let (low, high) = body.bounds().map_err(|e| failed(&e))?;
            // The body's extent along the direction, from its box's corners.
            let corners = (0..8).map(|i| {
                [
                    if i & 1 == 0 { low[0] } else { high[0] },
                    if i & 2 == 0 { low[1] } else { high[1] },
                    if i & 4 == 0 { low[2] } else { high[2] },
                ]
            });
            let along: Vec<f64> = corners.map(|c| dot(sub(c, a), normal)).collect();
            let (from, to) = along
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(*v), hi.max(*v))
                });
            let mut k = (from / step).ceil();
            let mut planes = 0;
            while k * step <= to && planes < 2000 {
                let at: Point = std::array::from_fn(|i| a[i] + normal[i] * k * step);
                made.derived.extend(added_curves(
                    body.section(at, normal).map_err(|e| failed(&e))?,
                    id,
                ));
                k += 1.0;
                planes += 1;
            }
        }
        made.message = Some(Message::with(
            "done.contour",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
