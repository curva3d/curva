//! Helpers several commands in this tab share.

use crate::Message;
use crate::commands::bodies_of;
use crate::commands::support::*;
use crate::recipe::Context;
use crate::recipe::Made;
use crate::recipe::Recipe;
use crate::recipe::StepDef;
use crate::recipe::Value;
use curva_document::Shape;
use curva_kernel::Curve;

/// `AreaCentroid` (or, with `VOLUME`, `VolumeCentroid`): places a point at the centre of each
/// surface's area, or solid's volume, and reports where.
#[derive(Default)]
pub struct Centroid<const VOLUME: bool>;

impl<const VOLUME: bool> Recipe for Centroid<VOLUME> {
    const NAME: &'static str = if VOLUME {
        "volumecentroid"
    } else {
        "areacentroid"
    };

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.centroid.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        let mut said = Vec::new();
        for (_, body) in bodies_of(cx.doc, ids) {
            // Exact from the engine: a solid's volume, or a surface's area. A solid's area
            // centre comes from its mesh, which the engine does not give.
            let found = match (VOLUME, body.is_solid()) {
                (true, true) | (false, false) => body.centroid().ok().or_else(|| {
                    if VOLUME {
                        volume_centroid(&body)
                    } else {
                        area_centroid(&body)
                    }
                }),
                (true, false) => None,
                (false, true) => area_centroid(&body),
            };
            if let Some(at) = found {
                said.push(format!("{:.4}, {:.4}, {:.4}", at[0], at[1], at[2]));
                made.add.push(Shape::Curve(Curve::Point { at }));
            }
        }
        if said.is_empty() {
            return Err(Message::new("error.centroid"));
        }
        made.message = Some(Message::with("done.centroid", vec![said.join("; ")]));
        Ok(made)
    }
}
