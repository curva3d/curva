//! The `Polygon` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_kernel::{Curve, dot, sub};
use std::f64::consts::TAU;

const SIDES: &str = "NumSides";

const CIRCUMSCRIBED: &str = "Circumscribed";

/// `Polygon`: a regular polygon from its centre and a corner (or, with `Circumscribed`, the
/// middle of a side); `NumSides` sets how many.
#[derive(Default)]
pub struct Polygon;

impl Recipe for Polygon {
    const NAME: &'static str = "polygon";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SIDES, 5, 3, 100),
        OptionDef::toggle(CIRCUMSCRIBED, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.polygon.centre")];
        steps.extend(point(cx, 0).map(|c| StepDef::point("prompt.polygon.corner").from(c)));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(corner)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sides = cx.number(SIDES) as usize;
        let to = sub(corner, c);
        let (u, v) = (dot(to, x), dot(to, y));
        let mut reach = u.hypot(v);
        let mut start = v.atan2(u);
        if cx.toggle(CIRCUMSCRIBED) {
            // The point picked is the middle of a side.
            reach /= (std::f64::consts::PI / sides as f64).cos();
            start += std::f64::consts::PI / sides as f64;
        }
        if reach <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        let points = (0..sides)
            .map(|i| {
                let angle = start + TAU * i as f64 / sides as f64;
                plus(plus(c, x, reach * angle.cos()), y, reach * angle.sin())
            })
            .collect();
        added(Curve::polyline(points, true), "done.polygon")
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn polygon_helix_and_spiral() {
        let mut doc = Document::default();
        run(&mut doc, &["polygon", "numsides", "6", "0,0", "10,0"]);
        let Some(Curve::Polyline {
            points,
            closed: true,
        }) = doc.state().objects[0].curve()
        else {
            panic!("no polygon");
        };
        assert_eq!(points.len(), 6);
        let mut doc = Document::default();
        run(&mut doc, &["helix", "0,0,0", "0,0,10", "3"]);
        let sample = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(sample.iter().all(|p| (p[0].hypot(p[1]) - 3.0).abs() < 0.2));
        let mut doc = Document::default();
        run(&mut doc, &["spiral", "flat", "0,0,0", "0,0,10", "1", "5"]);
        assert!(
            doc.state().objects[0]
                .curve()
                .unwrap()
                .sample()
                .unwrap()
                .iter()
                .all(|p| p[2].abs() < 1e-6)
        );
    }
}
