//! The `Reparameterize` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Spline;

/// `Reparameterize`, as in Rhino: gives curves a new parameter range, start and end, offering the
/// first curve's own range; `Automatic` gives each curve the range of its length (#741).
const AUTOMATIC: &str = "Automatic";

/// The first chosen curve's own parameter range, offered as the default.
fn current_range(cx: &Context<'_>) -> (f64, f64) {
    curves_of(cx.doc, objects(cx, 0))
        .first()
        .and_then(|(_, c)| Spline::of(c).ok())
        .map_or((0.0, 1.0), |s| s.domain())
}

#[derive(Default)]
pub struct Reparameterize;

impl Recipe for Reparameterize {
    const NAME: &'static str = "reparameterize";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::finish(AUTOMATIC)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.reparameterize.objects").curves()];
        if cx.values.is_empty() || cx.mode == Some(AUTOMATIC) {
            return steps;
        }
        let (start, end) = current_range(cx);
        steps.push(
            StepDef::number("prompt.reparameterize.start")
                .remembered("Start", start)
                .offering(&[AUTOMATIC]),
        );
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.reparameterize.end").remembered("End", end));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let automatic = cx.mode == Some(AUTOMATIC);
        let range = (number(cx, 1), number(cx, 2));
        if !automatic && (range.0.is_none() || range.1.is_none()) {
            return Ok(Made::default());
        }
        if let (Some(start), Some(end)) = range
            && end <= start
        {
            return Err(Message::new("error.reparameterize_order"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            // Automatic: each curve's range runs from 0 to its length, as in Rhino.
            let (start, end) = if automatic {
                (0.0, curve.length().map_err(|e| failed(&e))?)
            } else {
                (range.0.unwrap_or(0.0), range.1.unwrap_or(1.0))
            };
            let curve = spline
                .reparameterized(start, end)
                .to_curve()
                .map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.reparameterize",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Spline};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn automatic_gives_the_curve_the_range_of_its_length_and_enter_keeps_the_range() {
        let mut doc = Document::default();
        let points = vec![[0.0; 3], [1.0, 2.0, 0.0], [3.0, 2.0, 0.0], [4.0, 0.0, 0.0]];
        let id = doc.add(Curve::control_point(points, 3, false).unwrap());
        let before = Spline::of(&only_curve(&doc)).unwrap().domain();
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "", ""]);
        let kept = Spline::of(&only_curve(&doc)).unwrap().domain();
        assert!((kept.0 - before.0).abs() < 1e-12 && (kept.1 - before.1).abs() < 1e-12);
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "automatic"]);
        let curve = only_curve(&doc);
        let (start, end) = Spline::of(&curve).unwrap().domain();
        assert!(
            start.abs() < 1e-12 && (end - curve.length().unwrap()).abs() < 1e-9,
            "{start} {end}"
        );
    }

    #[test]
    fn reparameterize_and_make_uniform() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(
                vec![
                    [0.0; 3],
                    [1.0, 2.0, 0.0],
                    [3.0, 2.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [6.0, 1.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "10", "20"]);
        assert_eq!(
            Spline::of(&only_curve(&doc)).unwrap().domain(),
            (10.0, 20.0)
        );
        run(&mut doc, &["makeuniform"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        let inner = &spline.knots[4..spline.knots.len() - 4];
        assert!((inner[0] - 15.0).abs() < 1e-9, "{:?}", spline.knots);
    }
}
