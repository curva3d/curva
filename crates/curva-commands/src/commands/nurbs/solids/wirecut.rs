//! The `Wirecut` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::{bodies_of, curves_of};
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, gap};

/// `Wirecut`: cuts solids with a curve pulled straight through them, out of the plane being drawn
/// on. Only the part on the right of the curve's direction is kept, as Rhino keeps it (#729);
/// `KeepAll` keeps both.
#[derive(Default)]
pub struct Wirecut;

impl Recipe for Wirecut {
    const NAME: &'static str = "wirecut";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("KeepAll", false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.wirecut.curve").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.wirecut.objects").bodies());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let cutters = curves_of(cx.doc, objects(cx, 0));
        let Some((_, cutter)) = cutters.first() else {
            return Ok(Made::default());
        };
        let normal = up(cx);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 1)) {
            let (low, high) = body.bounds().map_err(|e| failed(&e))?;
            let reach = gap(low, high).max(1.0) * 2.0;
            // The curve pulled through from well behind to well in front.
            let sheet = Body::extruded(
                &[cutter.translated(normal.map(|v| -v * reach))],
                normal,
                2.0 * reach,
                false,
            )
            .map_err(|e| failed(&e))?;
            let Some((behind, front)) = body.split_by(&sheet).map_err(|e| failed(&e))? else {
                continue;
            };
            if !cx.toggle("KeepAll") {
                made.replace.push((id, Shape::Body(front)));
                continue;
            }
            made.replace.push((id, Shape::Body(behind)));
            made.derived.push(Derived {
                shape: Shape::Body(front),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.wirecut",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {

    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn by_default_only_the_part_right_of_the_curve_is_kept_as_in_rhino() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        let cutter = doc.add(Curve::line([-1.0, 3.0, 5.0], [11.0, 7.0, 5.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[cutter]);
        processor.submit(&mut doc, "wirecut");
        doc.select(&[cube]);
        processor.submit(&mut doc, "");
        let kept: Vec<&Body> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .collect();
        assert_eq!(kept.len(), 1);
        // Rhino 8 keeps the low-y half: volume 500, up to y = 6.667.
        assert!((kept[0].volume().unwrap() - 500.0).abs() < 1e-6);
        let (_, high) = kept[0].bounds().unwrap();
        assert!((high[1] - 20.0 / 3.0).abs() < 1e-3, "{high:?}");
    }

    #[test]
    fn wirecut_cuts_a_box_in_two() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let cutter = doc.add(Curve::line([-1.0, 1.0, 0.0], [3.0, 1.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[cutter]);
        processor.submit(&mut doc, "wirecut");
        processor.submit(&mut doc, "keepall");
        doc.select(&[cube]);
        processor.submit(&mut doc, "");
        let volumes: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .map(|b| b.volume().unwrap())
            .collect();
        assert_eq!(volumes.len(), 2, "{volumes:?}");
        assert!(
            volumes.iter().all(|v| (v - 4.0).abs() < 1e-6),
            "{volumes:?}"
        );
    }
}
