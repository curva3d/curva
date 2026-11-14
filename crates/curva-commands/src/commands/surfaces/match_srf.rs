//! The `MatchSrf` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

/// `MatchSrf`: changes a surface along an edge to meet another surface's edge, in position or
/// tangency (the default).
#[derive(Default)]
pub struct MatchSrf;

impl Recipe for MatchSrf {
    const NAME: &'static str = "matchsrf";
    const OPTIONS: &'static [OptionDef] =
        &[OptionDef::mode("Position"), OptionDef::mode("Tangency")];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.matchsrf.change").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::edges("prompt.matchsrf.target").one());
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
        let matched = a
            .matched_to(a_edge, &b, b_edge, continuity(cx, 1).min(1))
            .map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(a_id, Shape::Body(matched))],
            message: Some(Message::new("done.matchsrf")),
            ..Made::default()
        })
    }
}
