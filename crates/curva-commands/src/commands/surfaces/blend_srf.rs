//! The `BlendSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

const CONTINUITY: [OptionDef; 4] = [
    OptionDef::mode("Position"),
    OptionDef::mode("Tangency"),
    OptionDef::mode("Curvature"),
    OptionDef::number("Bulge", 1.0),
];

/// `BlendSrf`: a surface between two surface edges, meeting them in position, tangency or
/// curvature (the default); `Bulge` sets how far it leaves each edge.
#[derive(Default)]
pub struct BlendSrf;

impl Recipe for BlendSrf {
    const NAME: &'static str = "blendsrf";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.blendsrf.first").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::edges("prompt.blendsrf.second").one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((a_id, a_edge)), Some((b_id, b_edge))) = (edge(cx, 0), edge(cx, 1)) else {
            return Ok(Made::default());
        };
        let (Some(a), Some(b)) = (body_of(cx, a_id), body_of(cx, b_id)) else {
            return Ok(Made::default());
        };
        let blend = a
            .blend_to(a_edge, &b, b_edge, continuity(cx, 2), cx.number("Bulge"))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(blend),
                from: a_id,
                history: None,
            }],
            message: Some(Message::new("done.blendsrf")),
            ..Made::default()
        })
    }
}
