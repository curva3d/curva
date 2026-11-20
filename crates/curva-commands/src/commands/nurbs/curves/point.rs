//! `Point`: places a point object where picked, as in Rhino.

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Curve;

/// The recipe.
#[derive(Default)]
pub struct PointObject;

impl Recipe for PointObject {
    const NAME: &'static str = "point";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point("prompt.point.at")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(at) = cx.values.first().and_then(Value::point) else {
            return Ok(Made::default());
        };
        Ok(Made {
            add: vec![Shape::Curve(Curve::Point { at })],
            message: Some(Message::new("done.point")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn places_a_point_where_asked() {
        let mut doc = Document::default();
        run(&mut doc, &["point", "1,2,3"]);
        assert_eq!(
            *doc.state().objects[0].curve().unwrap(),
            Curve::Point {
                at: [1.0, 2.0, 3.0]
            }
        );
    }
}
