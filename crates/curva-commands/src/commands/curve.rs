//! `Curve`: a control-point curve, as in Rhino. Click points, Enter to finish, or `Close` once
//! there are three. `Degree` is remembered; `Undo` takes back the last point. Rhino's `AutoClose`,
//! `PersistentClose` and `Sharp` are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, point_run, run_points};
use curva_document::Shape;
use curva_kernel::Curve;

const DEGREE: &str = "Degree";
const CLOSE: &str = "Close";

/// The recipe.
#[derive(Default)]
pub struct CurveCommand;

impl Recipe for CurveCommand {
    const NAME: &'static str = "curve";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(DEGREE, 3, 1, 11),
        OptionDef::finish(CLOSE),
        OptionDef::undo(),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = point_run(cx, "prompt.curve.start", "prompt.curve.next", 2);
        let offered: &[&'static str] = match run_points(cx).len() {
            0 => &[DEGREE],
            1 | 2 => &[DEGREE, "Undo"],
            _ => &[DEGREE, CLOSE, "Undo"],
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
        // The degree is whole and from 1 to 11, so this cast is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let degree = cx.number(DEGREE) as usize;
        let curve = Curve::control_point(points, degree, cx.mode == Some(CLOSE))
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::with("done.curve", vec![count.to_string()])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects[0].curve().unwrap().clone()
    }

    #[test]
    fn has_one_control_point_per_click_and_runs_from_first_to_last() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "0,0", "5,8", "10,-3", "15,2", "20,0", ""],
        );
        let curve = only_curve(&doc);
        let Curve::Nurbs { points, degree, .. } = &curve else {
            panic!("not a control-point curve");
        };
        assert_eq!((points.len(), *degree), (5, 3));
        let path = curve.sample().unwrap();
        assert!(gap(path[0], [0.0, 0.0, 0.0]) < 1e-6);
        assert!(gap(path[path.len() - 1], [20.0, 0.0, 0.0]) < 1e-6);
    }

    #[test]
    fn close_joins_it_round() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "0,0", "10,0", "10,10", "0,10", "close"],
        );
        let path = only_curve(&doc).sample().unwrap();
        assert!(gap(path[0], path[path.len() - 1]) < 1e-6);
    }

    #[test]
    fn the_degree_is_an_option_and_is_remembered() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "degree", "2", "0,0", "5,5", "10,0", "15,5", ""],
        );
        run(&mut doc, &["curve", "0,0", "5,5", "10,0", "15,5", ""]);
        for object in &doc.state().objects {
            assert!(matches!(
                object.curve(),
                Some(Curve::Nurbs { degree: 2, .. })
            ));
        }
    }

    #[test]
    fn a_degree_out_of_range_is_refused() {
        let mut doc = Document::default();
        run(&mut doc, &["curve", "degree", "12"]);
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn undo_takes_back_a_point() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["curve", "0,0", "5,5", "99,99", "undo", "10,0", ""],
        );
        let Some(Curve::Nurbs { points, degree, .. }) = doc.state().objects[0].curve() else {
            panic!("not a control-point curve");
        };
        // Three points allow degree 2 at most.
        assert_eq!((points.len(), *degree), (3, 2));
    }
}
