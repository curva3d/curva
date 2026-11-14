//! The `ExtrudeCrvAlongCrv` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Body;

/// `ExtrudeCrvAlongCrv`: moves curves along a path curve without turning them; closed flat
/// curves make solids with `Solid`.
#[derive(Default)]
pub struct ExtrudeCrvAlongCrv;

impl Recipe for ExtrudeCrvAlongCrv {
    const NAME: &'static str = "extrudecrvalongcrv";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Solid", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvalongcrv.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.extrudecrvalongcrv.path")
                    .curves()
                    .one(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let paths = curves_of(cx.doc, objects(cx, 1));
        let Some((_, path)) = paths.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let cap = cx.toggle("Solid") && curve.is_closed().unwrap_or(false);
            let body = Body::swept_translated(std::slice::from_ref(&curve), path, cap)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
