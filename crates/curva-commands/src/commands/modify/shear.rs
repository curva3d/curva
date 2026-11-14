//! The `Shear` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::{Point, cross, dot, sub, unit};

/// `Shear`: slants objects: an origin, a reference point setting the slant's direction, then
/// the angle. Points move along the reference direction in proportion to how far they are from
/// the line through the origin along it, in the construction plane.
#[derive(Default)]
pub struct Shear;

impl Shear {
    fn map(
        origin: Point,
        reference: Point,
        angle_degrees: f64,
        normal: Point,
    ) -> Option<[[f64; 4]; 4]> {
        let r = unit(sub(reference, origin))?;
        let p = unit(cross(normal, r))?;
        let k = angle_degrees.to_radians().tan();
        // x' = x + k ((x - o) · p) r: each axis's image, then the move.
        let image = |axis: usize| -> [f64; 4] {
            let mut row = [0.0; 4];
            row[axis] = 1.0;
            for j in 0..3 {
                row[j] += k * p[axis] * r[j];
            }
            row
        };
        let shift = -k * dot(origin, p);
        Some([
            image(0),
            image(1),
            image(2),
            [shift * r[0], shift * r[1], shift * r[2], 1.0],
        ])
    }
}

impl Recipe for Shear {
    const NAME: &'static str = "shear";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.shear.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.shear.origin"));
        }
        if let Some(o) = point(cx, 1) {
            steps.push(StepDef::point("prompt.shear.reference").from(o));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::number("prompt.shear.angle").remembered("Angle", 15.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(o), Some(r), Some(angle)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if angle.abs() >= 89.0 {
            return Err(Message::new("error.shear_angle"));
        }
        let map =
            Self::map(o, r, angle, up(cx)).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| objects(cx, 0).contains(&o.id))
        {
            let shape = curva_document::blocks::shape_placed(&object.shape, &map)
                .ok_or_else(|| Message::new("error.shear_failed"))?;
            made.replace.push((object.id, shape));
        }
        made.message = Some(Message::with(
            "done.shear",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}
