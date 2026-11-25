//! The `Ellipsoid` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, cross, dot, gap, sub, unit};
use std::f64::consts::TAU;

/// `Ellipsoid`: its centre, the end of its first axis, the end of its second, then the end of the
/// third (its distance from the centre) or its length, straight out of the plane, as in Rhino.
#[derive(Default)]
pub struct Ellipsoid;

impl Recipe for Ellipsoid {
    const NAME: &'static str = "ellipsoid";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.ellipse.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.ellipse.first").from(c));
            if cx.values.len() >= 2 {
                steps.push(StepDef::point("prompt.ellipse.second").from(c));
            }
            if cx.values.len() >= 3 {
                steps.push(StepDef::point_or_number("prompt.ellipsoid.third").from(c));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(first)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let up = cross(x, y);
        let a = unit(sub(first, c)).ok_or_else(|| Message::new("error.reference_point"))?;
        let b = cross(up, a);
        let ra = gap(first, c);
        let third = match cx.values.get(3) {
            Some(Value::Number(length)) => Some(*length),
            Some(Value::Point(end)) => Some(gap(*end, c)),
            _ => None,
        };
        let (Some(second), Some(rc)) = (point(cx, 2), third) else {
            return Ok(Made {
                guides: Curve::line(c, first).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let rb = dot(sub(second, c), b).abs();
        if ra <= 1e-9 || rb <= 1e-9 || rc <= 1e-9 {
            return Err(Message::new("error.bad_radius"));
        }
        // A sphere of the first radius, stretched along the other two axes.
        let bottom = plus(c, up, -ra);
        let top = plus(c, up, ra);
        let arc = Curve::arc(c, bottom, top, a, up).map_err(|e| failed(&e))?;
        let axis = Curve::line(top, bottom).map_err(|e| failed(&e))?;
        let half = Curve::compound(vec![arc, axis]).map_err(|e| failed(&e))?;
        let body = Body::revolved(&[half], c, up, TAU, true)
            .and_then(|s| s.stretched(c, b, rb / ra))
            .and_then(|s| s.stretched(c, up, rc / ra))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.ellipsoid")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use std::f64::consts::PI;

    #[test]
    fn the_third_axis_takes_a_point_as_in_rhino() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipsoid", "0,0,0", "5,0,0", "0,3,0", "0,0,2"]);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 125.6637).abs() < 1e-3, "{volume}");
    }

    #[test]
    fn ellipsoid_and_boolean_split() {
        let mut doc = Document::default();
        run(&mut doc, &["ellipsoid", "0,0,0", "5,0,0", "0,3,0", "2"]);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 4.0 / 3.0 * PI * 30.0).abs() < 1e-2, "{volume}");
        let mut doc = Document::default();
        let a = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let b = doc.add_body(curva_kernel::Body::cuboid([2.0, 0.0, 0.0], [4.0, 4.0, 4.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "booleansplit");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        assert_eq!(doc.state().objects.len(), 3);
    }
}
