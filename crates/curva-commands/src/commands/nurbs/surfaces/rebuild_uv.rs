//! The `RebuildUv` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

/// `RebuildUV`: refits untrimmed surfaces with a new degree and number of control points each
/// way.
#[derive(Default)]
pub struct RebuildUv;

impl Recipe for RebuildUv {
    const NAME: &'static str = "rebuilduv";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("PointCountU", 6, 2, 100),
        OptionDef::whole("PointCountV", 6, 2, 100),
        OptionDef::whole("DegreeU", 3, 1, 11),
        OptionDef::whole("DegreeV", 3, 1, 11),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.rebuilduv.objects")
                .bodies()
                .offering(&["PointCountU", "PointCountV", "DegreeU", "DegreeV"]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = |name: &str| cx.number(name).round().max(1.0) as usize;
        let degrees = [whole("DegreeU"), whole("DegreeV")];
        let counts = [whole("PointCountU"), whole("PointCountV")];
        let spans = [
            counts[0].saturating_sub(degrees[0]).max(1),
            counts[1].saturating_sub(degrees[1]).max(1),
        ];
        let mut made = Made::default();
        let mut worst: f64 = 0.0;
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let (rebuilt, off) = body.rebuilt_uv(degrees, spans).map_err(|e| failed(&e))?;
            worst = worst.max(off);
            made.replace.push((id, Shape::Body(rebuilt)));
        }
        made.message = Some(Message::with(
            "done.rebuilduv",
            vec![made.replace.len().to_string(), format!("{worst:.4}")],
        ));
        Ok(made)
    }
}
