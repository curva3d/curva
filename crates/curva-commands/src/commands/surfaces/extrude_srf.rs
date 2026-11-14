//! The `ExtrudeSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Point, cross};

/// `ExtrudeSrf`, as in Rhino: pulls surfaces straight out of the plane being drawn on into
/// solids, the sides swept by their open edges; curved surfaces too. The distance follows the
/// mouse, or is typed; Enter uses the last one. `BothSides` goes as far each way; `DeleteInput`
/// takes the surfaces away.
#[derive(Default)]
pub struct ExtrudeSrf;

impl ExtrudeSrf {
    fn up(cx: &Context<'_>) -> Point {
        let [x, y] = cx.plane();
        curva_kernel::unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
    }
}

impl Recipe for ExtrudeSrf {
    const NAME: &'static str = "extrudesrf";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle("BothSides", false),
        OptionDef::toggle("DeleteInput", true),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudesrf.objects")
                .bodies()
                .offering(&[]),
        ];
        let base = match cx.values.first() {
            Some(Value::Objects(ids)) => bodies_of(cx.doc, ids)
                .first()
                .and_then(|(_, body)| body.bounds().ok())
                .map(|(low, high)| std::array::from_fn(|i| (low[i] + high[i]) / 2.0)),
            _ => None,
        };
        if let Some(base) = base {
            steps.push(
                StepDef::along("prompt.extrudecrv.distance", base, Self::up(cx))
                    .remembered("Distance", 10.0),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(distance)) = (cx.values.first(), number(cx, 1)) else {
            return Ok(Made::default());
        };
        if distance.abs() < 1e-9 {
            return Err(Message::new("error.bad_distance"));
        }
        let up = Self::up(cx);
        let (lift, length) = if cx.toggle("BothSides") {
            (-distance, 2.0 * distance)
        } else {
            (0.0, distance)
        };
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, ids) {
            if body.is_solid() {
                continue;
            }
            let start = body.translated(up.map(|v| v * lift));
            let solid = start
                .extruded_straight(up, length)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(solid),
                from: id,
                history: None,
            });
            if cx.toggle("DeleteInput") {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.extrudesrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}
