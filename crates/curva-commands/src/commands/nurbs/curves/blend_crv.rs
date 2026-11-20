//! The `BlendCrv` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Spline, gap};

/// `BlendCrv`: a curve joining the nearest ends of two curves, meeting them in position, along
/// their tangents, or in curvature (the default), as Rhino's BlendCrv.
#[derive(Default)]
pub struct BlendCrv;

impl Recipe for BlendCrv {
    const NAME: &'static str = "blendcrv";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.blendcrv.first").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.blendcrv.second").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (first, second) = (
            curves_of(cx.doc, objects(cx, 0)),
            curves_of(cx.doc, objects(cx, 1)),
        );
        let (Some((_, a)), Some((_, b))) = (first.first(), second.first()) else {
            return Ok(Made::default());
        };
        let (sa, sb) = (
            Spline::of(a).map_err(|e| failed(&e))?,
            Spline::of(b).map_err(|e| failed(&e))?,
        );
        let (a_end, b_end) = nearest_ends(&sa, &sb);
        let (p0, _, _) = leaving(&sa, a_end);
        let (p3, _, _) = leaving(&sb, b_end);
        if gap(p0, p3) <= 1e-9 {
            return Err(Message::new("error.blend_touching"));
        }
        // The engine's blend: the least-degree Bézier meeting each end as asked.
        let contact = match continuity(cx, Continuity::Curvature) {
            Continuity::Position => curva_kernel::Contact::Position,
            Continuity::Tangency => curva_kernel::Contact::Tangency,
            Continuity::Curvature => curva_kernel::Contact::Curvature,
        };
        let curve = a
            .blended_with(a_end, contact, b, b_end, contact, 1.0 / 3.0)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.blendcrv")),
            ..Made::default()
        })
    }
}
