//! The `Pyramid` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point, cross, dot, sub, unit};

/// `Pyramid`: a pyramid on a regular polygon: its centre, a corner (or, with `Circumscribed`,
/// the middle of a side), then its height. `NumSides` sets how many sides.
#[derive(Default)]
pub struct Pyramid;

impl Recipe for Pyramid {
    const NAME: &'static str = "pyramid";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("NumSides", 5, 3, 100),
        OptionDef::toggle("Circumscribed", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.pyramid.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.pyramid.corner").from(c));
            if point(cx, 1).is_some() {
                steps.push(StepDef::along("prompt.pyramid.height", c, up(cx)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(corner)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let normal = up(cx);
        let toward = sub(corner, c);
        let flat = sub(toward, normal.map(|v| v * dot(toward, normal)));
        let x = unit(flat).ok_or_else(|| Message::new("error.reference_point"))?;
        let y = cross(normal, x);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sides = cx.number("NumSides").round().max(3.0) as usize;
        let step = std::f64::consts::TAU / sides as f64;
        let mut radius = dot(flat, flat).sqrt();
        let mut start = 0.0;
        if cx.toggle("Circumscribed") {
            radius /= (step / 2.0).cos();
            start = step / 2.0;
        }
        let corners: Vec<Point> = (0..sides)
            .map(|i| {
                let angle = start + step * i as f64;
                plus(plus(c, x, radius * angle.cos()), y, radius * angle.sin())
            })
            .collect();
        let base = Curve::polyline(corners, true).map_err(|e| failed(&e))?;
        let Some(height) = number(cx, 2).filter(|h| h.abs() > 1e-9) else {
            return Ok(Made {
                guides: vec![base],
                ..Made::default()
            });
        };
        let body = Body::pyramid(&base, plus(c, normal, height)).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.pyramid")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;

    fn last_volume(doc: &Document) -> f64 {
        doc.state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn pyramids_on_regular_polygons() {
        let mut doc = Document::default();
        run(&mut doc, &["pyramid", "NumSides", "4", "0,0", "1,0", "3"]);
        // A square of diagonal 2 (area 2), 3 high: volume 2.
        assert!(
            (last_volume(&doc) - 2.0).abs() < 1e-6,
            "{}",
            last_volume(&doc)
        );
        let mut doc = Document::default();
        run(
            &mut doc,
            &[
                "pyramid",
                "NumSides",
                "4",
                "Circumscribed",
                "0,0",
                "1,0",
                "3",
            ],
        );
        // Sides touching radius 1: a 2 × 2 square, volume 4.
        assert!(
            (last_volume(&doc) - 4.0).abs() < 1e-6,
            "{}",
            last_volume(&doc)
        );
    }
}
