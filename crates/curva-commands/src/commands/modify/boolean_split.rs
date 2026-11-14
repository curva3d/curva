//! The `BooleanSplit` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Combine;

/// `BooleanSplit`: splits solids with others into the part inside and the part outside.
#[derive(Default)]
pub struct BooleanSplit;

impl Recipe for BooleanSplit {
    const NAME: &'static str = "booleansplit";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("DeleteInput", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.booleansplit.split").bodies()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::objects("prompt.booleansplit.with")
                    .bodies()
                    .apart(),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(split)), Some(Value::Objects(with))) =
            (cx.values.first(), cx.values.get(1))
        else {
            return Ok(Made::default());
        };
        let cutters = bodies_of(cx.doc, with);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, split) {
            for (_, cutter) in &cutters {
                for how in [Combine::Difference, Combine::Intersection] {
                    if let Some(part) = body.combined(cutter, how).map_err(|e| failed(&e))? {
                        made.derived.push(Derived {
                            shape: Shape::Body(part),
                            from: id,
                            history: None,
                        });
                    }
                }
            }
            if cx.toggle("DeleteInput") {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.booleansplit",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
