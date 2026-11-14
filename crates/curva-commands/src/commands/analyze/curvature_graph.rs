//! The `CurvatureGraph` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_kernel::{Curve, Point, dot, gap};

/// `CurvatureGraph`: shows curves' curvature combs, scaled by `Scale`, with `Density` teeth;
/// Enter ends it.
#[derive(Default)]
pub struct CurvatureGraph;

impl Recipe for CurvatureGraph {
    const NAME: &'static str = "curvaturegraph";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("Scale", 10.0),
        OptionDef::whole("Density", 60, 2, 2000),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.curvaturegraph.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.curvaturegraph.done").optional());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let scale = cx.number("Scale");
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let teeth = cx.number("Density").round().max(2.0) as usize;
        let mut guides = Vec::new();
        let mut most: f64 = 0.0;
        for (_, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let Ok(comb) = curve.curvature_comb(teeth) else {
                continue;
            };
            // Teeth stand out of the curve, away from its centre of curvature, as in Rhino.
            let tips: Vec<Point> = comb
                .iter()
                .map(|(at, k)| {
                    most = most.max(dot(*k, *k).sqrt());
                    std::array::from_fn(|i| at[i] - k[i] * scale)
                })
                .collect();
            for ((at, _), tip) in comb.iter().zip(&tips) {
                if gap(*at, *tip) > 1e-9
                    && let Ok(tooth) = Curve::line(*at, *tip)
                {
                    guides.push(tooth);
                }
            }
            if tips.len() >= 2
                && let Ok(outline) = Curve::polyline(tips, false)
            {
                guides.push(outline);
            }
        }
        Ok(Made {
            guides,
            message: Some(Message::with(
                "done.curvaturegraph",
                vec![format!("{most:.5}")],
            )),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {

    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn curvature_graph_shows_teeth_and_reports() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 5.0).unwrap());
        doc.select(&[id]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "curvaturegraph");
        assert!(
            !processor
                .preview(&doc, Some([0.0, 0.0, 0.0]), "")
                .is_empty()
        );
        processor.submit(&mut doc, "");
        assert!(
            processor.log().iter().any(|l| l.contains("0.2")),
            "{:?}",
            processor.log()
        );
    }
}
