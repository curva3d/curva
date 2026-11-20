//! The `Helix` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_kernel::Curve;

/// `Helix`: the start and end of its axis, then its radius; `Turns` sets how many times round.
#[derive(Default)]
pub struct Helix;

impl Recipe for Helix {
    const NAME: &'static str = "helix";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::number(TURNS, 10.0)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.helix.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.helix.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.helix.radius").from(b));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(r)) = (point(cx, 0), point(cx, 1), number(cx, 2)) else {
            return Ok(Made::default());
        };
        let [x, _] = cx.plane();
        let turns = cx.number(TURNS).max(0.1);
        let points = turning(a, b, x, r, r, turns, false)
            .ok_or_else(|| Message::new("error.reference_point"))?;
        added(Curve::interpolated(points, false), "done.helix")
    }
}
