//! The `SrfPt` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point};

/// `SrfPt`: a surface from three or four corner points; Enter after three makes a triangle.
#[derive(Default)]
pub struct SrfPt;

impl Recipe for SrfPt {
    const NAME: &'static str = "srfpt";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.srfpt.first")];
        for i in 0..3 {
            if let Some(p) = point(cx, i) {
                let step = StepDef::point("prompt.srfpt.next").from(p);
                steps.push(if i == 2 { step.optional() } else { step });
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points: Vec<Point> = cx.values.iter().filter_map(Value::point).collect();
        let finished = cx.values.len() >= 4;
        let body = match points.as_slice() {
            [a, b, c] if finished => {
                let outline = Curve::polyline(vec![*a, *b, *c], true).map_err(|e| failed(&e))?;
                Body::planar(&[outline])
            }
            [a, b, c, d] => {
                // A ruled surface between the first side and the opposite one.
                let first = Curve::line(*a, *b).map_err(|e| failed(&e))?;
                let other = Curve::line(*d, *c).map_err(|e| failed(&e))?;
                Body::lofted(&[first, other], false)
            }
            _ => {
                let guides = points
                    .windows(2)
                    .filter_map(|pair| Curve::line(pair[0], pair[1]).ok())
                    .collect();
                return Ok(Made {
                    guides,
                    ..Made::default()
                });
            }
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.srfpt")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn extrudesrf_goes_straight_up_as_rhino_does_not_along_the_surface() {
        // A tilted square, 4 by 4 on the ground, rising 3 along y: pulled straight up by 2, its
        // volume is its footprint times 2, and the top is 2 above the highest corner.
        let mut doc = Document::default();
        run(&mut doc, &["srfpt", "0,0,0", "4,0,0", "4,4,3", "0,4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["extrudesrf", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!(
            (solid.volume().unwrap() - 32.0).abs() < 1e-3,
            "{}",
            solid.volume().unwrap()
        );
        let (low, high) = solid.bounds().unwrap();
        assert!(
            (high[2] - 5.0).abs() < 1e-6 && low[2].abs() < 1e-6,
            "{low:?} {high:?}"
        );
        // BothSides goes as far down as up.
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,5"]);
        doc.select(&[doc.state().objects[0].id]);
        run(&mut doc, &["extrudesrf", "BothSides", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!((solid.volume().unwrap() - 80.0).abs() < 1e-3);
    }
}
