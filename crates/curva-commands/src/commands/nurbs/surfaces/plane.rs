//! The `Plane` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve, dot, sub};

/// `Plane`: a flat rectangular surface from two opposite corners, on the plane being drawn on.
#[derive(Default)]
pub struct Plane;

impl Recipe for Plane {
    const NAME: &'static str = "plane";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.rectangle.first")];
        steps.extend(
            point(cx, 0).map(|a| StepDef::point("prompt.rectangle.other").from(a).corner()),
        );
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let across = sub(b, a);
        let (w, h) = (dot(across, x), dot(across, y));
        if w.abs() < 1e-9 || h.abs() < 1e-9 {
            return Err(Message::new("error.flat_rectangle"));
        }
        // As Rhino's Plane: an untrimmed surface, ruled between two opposite sides.
        let near = Curve::line(a, plus(a, x, w)).map_err(|e| failed(&e))?;
        let far = Curve::line(plus(a, y, h), plus(plus(a, x, w), y, h)).map_err(|e| failed(&e))?;
        // The surface faces up out of the plane being drawn on whichever way the corner was
        // dragged, as in Rhino: the edges taken the other way round where it would face down.
        let (first, second) = if w * h < 0.0 {
            (&far, &near)
        } else {
            (&near, &far)
        };
        let body = Body::ruled(first, second, false).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.planarsrf_one")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn a_plane_faces_up_whichever_way_its_corner_is_dragged() {
        for corner in ["4,5", "-4,5", "4,-5", "-4,-5"] {
            let mut doc = Document::default();
            run(&mut doc, &["plane", "0,0", corner]);
            let body = doc.state().objects[0].shape.body().unwrap();
            let normal = body.face_normal_near(0, [0.0; 3]).unwrap();
            assert!(normal[2] > 0.99, "{corner}: {normal:?}");
        }
    }

    #[test]
    fn plane_srfpt_and_extrudesrf() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,5"]);
        let body = doc.state().objects[0].shape.body().unwrap().clone();
        assert!((body.area().unwrap() - 20.0).abs() < 1e-6);
        doc.select(&[doc.state().objects[0].id]);
        // Its preview shows the solid before the distance is given.
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extrudesrf");
        assert!(!processor.preview_bodies(&doc, None, "2").is_empty());
        processor.cancel(&mut doc);
        run(&mut doc, &["extrudesrf", "2"]);
        let solid = doc.state().objects[0].shape.body().unwrap();
        assert!((solid.volume().unwrap() - 40.0).abs() < 1e-3);
        let mut doc = Document::default();
        run(&mut doc, &["srfpt", "0,0", "4,0", "4,4", "0,4"]);
        assert!((doc.state().objects[0].shape.body().unwrap().area().unwrap() - 16.0).abs() < 1e-6);
    }
}
