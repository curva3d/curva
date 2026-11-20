//! The `Reparameterize` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Spline;

/// `Reparameterize`: gives curves a new parameter range, start and end.
#[derive(Default)]
pub struct Reparameterize;

impl Recipe for Reparameterize {
    const NAME: &'static str = "reparameterize";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.reparameterize.objects").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.reparameterize.start").remembered("Start", 0.0));
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::number("prompt.reparameterize.end").remembered("End", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(start), Some(end)) = (number(cx, 1), number(cx, 2)) else {
            return Ok(Made::default());
        };
        if end <= start {
            return Err(Message::new("error.reparameterize_order"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
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
