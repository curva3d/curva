//! The `Taper` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::Point;

/// `Taper`: narrows or widens objects along an axis: the axis, then how much bigger they are at
/// its end (1 keeps them).
#[derive(Default)]
pub struct Taper;

impl Recipe for Taper {
    const NAME: &'static str = "taper";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        axis_steps(
            cx,
            "prompt.taper.objects",
            Some(StepDef::number("prompt.taper.factor").remembered("Factor", 0.5)),
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(factor)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if factor <= 0.0 {
            return Err(Message::new("error.taper_factor"));
        }
        let map = move |p: Point| {
            let (t, foot, r) = along_axis(p, a, b);
            plus(foot, r, 1.0 + (factor - 1.0) * t)
        };
        deform_all(cx, objects(cx, 0), &map, "done.taper")
    }
}
