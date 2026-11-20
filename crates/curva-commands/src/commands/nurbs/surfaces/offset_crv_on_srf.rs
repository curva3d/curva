//! The `OffsetCrvOnSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `OffsetCrvOnSrf`: a curve on a surface moved along it by a distance, to the side of a point.
#[derive(Default)]
pub struct OffsetCrvOnSrf;

impl Recipe for OffsetCrvOnSrf {
    const NAME: &'static str = "offsetcrvonsrf";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.offsetcrvonsrf.curve")
                .curves()
                .one(),
        ];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.offsetcrvonsrf.surface")
                    .bodies()
                    .one(),
            );
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.offset.distance").remembered("Distance", 1.0));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::point("prompt.offset.side"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, curve)), Some((id, body)), Some(distance), Some(side)) = (
            first_curve(cx, 0),
            first_body(cx, 1),
            number(cx, 2),
            point(cx, 3),
        ) else {
            return Ok(Made::default());
        };
        let moved = body
            .offset_on_surface(&curve, distance, side)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Curve(moved),
                from: id,
                history: None,
            }],
            message: Some(Message::new("done.offsetcrvonsrf")),
            ..Made::default()
        })
    }
}
