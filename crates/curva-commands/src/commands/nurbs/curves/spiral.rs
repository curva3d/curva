//! The `Spiral` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_kernel::Curve;

/// `Spiral`: the start and end of its axis, its first radius, then its second; `Turns` sets how
/// many times round, and `Flat` lays it on the plane at the axis's start.
#[derive(Default)]
pub struct Spiral;

impl Recipe for Spiral {
    const NAME: &'static str = "spiral";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number(TURNS, 10.0),
        OptionDef::toggle("Flat", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.helix.start")];
        if let Some(a) = point(cx, 0) {
            steps.push(StepDef::point("prompt.helix.end").from(a));
            if let Some(b) = point(cx, 1) {
                steps.push(StepDef::number("prompt.spiral.first").from(a));
                if cx.values.len() >= 3 {
                    steps.push(StepDef::number("prompt.spiral.second").from(b));
                }
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(r1), Some(r2)) =
            (point(cx, 0), point(cx, 1), number(cx, 2), number(cx, 3))
        else {
            return Ok(Made::default());
        };
        let [x, _] = cx.plane();
        let turns = cx.number(TURNS).max(0.1);
        let points = turning(a, b, x, r1, r2, turns, cx.toggle("Flat"))
            .ok_or_else(|| Message::new("error.reference_point"))?;
        added(Curve::interpolated(points, false), "done.spiral")
    }
}
