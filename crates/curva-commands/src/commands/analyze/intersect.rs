//! The `Intersect` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::commands::{bodies_of, curves_of};
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Curve;

/// `Intersect`: curves where surfaces and solids meet each other, and points where curves cross.
#[derive(Default)]
pub struct Intersect;

impl Recipe for Intersect {
    const NAME: &'static str = "intersect";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.intersect.objects").at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        let bodies = bodies_of(cx.doc, ids);
        let curves = curves_of(cx.doc, ids);
        let mut made = Made::default();
        for (i, (id, a)) in bodies.iter().enumerate() {
            for (_, b) in &bodies[i + 1..] {
                made.derived.extend(added_curves(
                    a.intersections(b).map_err(|e| failed(&e))?,
                    *id,
                ));
            }
        }
        for (i, (_, a)) in curves.iter().enumerate() {
            for (_, b) in &curves[i + 1..] {
                for at in curva_kernel::crossings(a, b).unwrap_or_default() {
                    made.add.push(Shape::Curve(Curve::Point { at }));
                }
            }
        }
        made.message = Some(Message::with(
            "done.intersect",
            vec![(made.derived.len() + made.add.len()).to_string()],
        ));
        Ok(made)
    }
}
