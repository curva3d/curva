//! `InterpCrv`: a smooth curve through points, as in Rhino. Enter ends it, or `Close` once there
//! are three; `Undo` takes back the last point. Rhino's `Degree`, `Knots`, tangents, `AutoClose`,
//! `PersistentClose` and `Sharp` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, point_run, run_points};
use curva_document::Shape;
use curva_kernel::Curve;

const CLOSE: &str = "Close";

/// The recipe.
#[derive(Default)]
pub struct InterpCrv;

impl Recipe for InterpCrv {
    const NAME: &'static str = "interpcrv";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::finish(CLOSE), OptionDef::undo()];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = point_run(cx, "prompt.interpcrv.start", "prompt.interpcrv.next", 2);
        let offered: &[&'static str] = match run_points(cx).len() {
            0 => &[],
            1 | 2 => &["Undo"],
            _ => &[CLOSE, "Undo"],
        };
        if let Some(last) = steps.last_mut() {
            *last = last.clone().offering(offered);
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let points = run_points(cx);
        if points.len() < 2 {
            return Ok(Made::default());
        }
        let count = points.len();
        let curve = Curve::interpolated(points, cx.mode == Some(CLOSE))
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::with("done.interpcrv", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;

    #[test]
    fn makes_a_curve_from_its_first_to_its_last_point() {
        let mut doc = Document::default();
        run(&mut doc, &["interpcrv", "0,0", "10,5", "20,-3", "30,4", ""]);
        let drawn = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(drawn[0][0].abs() < 1e-6);
        assert!((drawn[drawn.len() - 1][0] - 30.0).abs() < 1e-6);
    }

    #[test]
    fn close_joins_it_back_to_its_start() {
        let mut doc = Document::default();
        run(&mut doc, &["interpcrv", "0,0", "10,0", "10,10", "close"]);
        let drawn = doc.state().objects[0].curve().unwrap().sample().unwrap();
        let last = drawn[drawn.len() - 1];
        assert!(last[0].abs() < 1e-6 && last[1].abs() < 1e-6);
    }
}
