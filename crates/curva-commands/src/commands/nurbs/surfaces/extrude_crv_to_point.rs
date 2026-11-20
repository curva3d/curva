//! The `ExtrudeCrvToPoint` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Body;

/// `ExtrudeCrvToPoint`: lines from every point of curves to one point: cones and pyramids,
/// solids from closed curves with `Solid`.
#[derive(Default)]
pub struct ExtrudeCrvToPoint;

impl Recipe for ExtrudeCrvToPoint {
    const NAME: &'static str = "extrudecrvtopoint";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Solid", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvtopoint.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.extrudecrvtopoint.apex"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(apex) = point(cx, 1) else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let cap = cx.toggle("Solid") && curve.is_closed().unwrap_or(false);
            let body = Body::extruded_to_point(&curve, apex, cap).map_err(|e| failed(&e))?;
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

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;
    use curva_kernel::Curve;

    fn square(doc: &mut Document, size: f64) -> u64 {
        doc.add(
            Curve::polyline(
                vec![
                    [0.0; 3],
                    [size, 0.0, 0.0],
                    [size, size, 0.0],
                    [0.0, size, 0.0],
                ],
                true,
            )
            .unwrap(),
        )
    }

    fn last_volume(doc: &Document) -> f64 {
        doc.state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn extrude_along_a_curve_and_to_a_point() {
        let mut doc = Document::default();
        let id = square(&mut doc, 2.0);
        let path = doc.add(Curve::line([0.0; 3], [3.0, 0.0, 4.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[id]);
        processor.submit(&mut doc, "extrudecrvalongcrv");
        doc.select(&[path]);
        processor.submit(&mut doc, "");
        assert!((last_volume(&doc) - 16.0).abs() < 1e-3);
        let mut doc = Document::default();
        let id = square(&mut doc, 3.0);
        doc.select(&[id]);
        run(&mut doc, &["extrudecrvtopoint", "1.5,1.5,4"]);
        assert!((last_volume(&doc) - 12.0).abs() < 1e-6);
    }
}
