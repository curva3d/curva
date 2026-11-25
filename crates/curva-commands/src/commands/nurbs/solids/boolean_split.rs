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
                // An open surface cuts the solid into closed pieces, each capped where it was
                // cut, as in Rhino (#711); a solid cutter gives what is outside it and inside it.
                if !cutter.is_solid() {
                    if let Some((behind, front)) = body.split_by(cutter).map_err(|e| failed(&e))? {
                        made.derived.extend([behind, front].map(|part| Derived {
                            shape: Shape::Body(part),
                            from: id,
                            history: None,
                        }));
                    }
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;

    #[test]
    fn a_plane_cuts_a_box_into_two_closed_halves_as_in_rhino() {
        let mut doc = Document::default();
        let mut processor = Processor::default();
        for step in [
            "box", "0,0,0", "10,10,0", "10", "plane", "-1,-1,5", "11,11,5",
        ] {
            processor.submit(&mut doc, step);
        }
        let (cube, plane) = (doc.state().objects[0].id, doc.state().objects[1].id);
        processor.submit(&mut doc, "booleansplit");
        doc.select(&[cube]);
        processor.submit(&mut doc, "");
        doc.select(&[plane]);
        processor.submit(&mut doc, "");
        let halves: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| o.id != plane)
            .filter_map(|o| o.shape.body())
            .map(|b| {
                assert!(b.is_solid());
                b.volume().unwrap()
            })
            .collect();
        assert_eq!(halves.len(), 2, "{halves:?}");
        assert!(
            halves.iter().all(|v| (v - 500.0).abs() < 1e-6),
            "{halves:?}"
        );
    }
}
