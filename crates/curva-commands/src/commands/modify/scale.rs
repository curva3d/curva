//! `Scale`, as in Rhino: resizes objects evenly about a base point, by a factor typed or by two
//! reference points. `Copy` resizes copies instead. Rhino's `Rigid` and Enter for the bounding
//! box centre are still to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_kernel::{Curve, Similarity, gap};

/// Smallest size change treated as real.
const SMALLEST_FACTOR: f64 = 1e-9;
const COPY: &str = "Copy";

/// The recipe.
#[derive(Default)]
pub struct Scale;

/// The factor the answers give: typed, or how much further the second reference point is.
fn factor(cx: &Context<'_>) -> Option<f64> {
    let origin = cx.values.get(1).and_then(Value::point)?;
    match cx.values.get(2)? {
        Value::Number(factor) => Some(*factor),
        Value::Point(first) => {
            let second = cx.values.get(3).and_then(Value::point)?;
            Some(gap(second, origin) / gap(*first, origin))
        }
        _ => None,
    }
}

impl Recipe for Scale {
    const NAME: &'static str = "scale";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(COPY, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.scale.objects").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.scale.origin"));
        }
        if let Some(origin) = cx.values.get(1).and_then(Value::point) {
            steps.push(StepDef::point_or_number("prompt.scale.factor").from(origin));
            if cx.values.get(2).and_then(Value::point).is_some() {
                steps.push(StepDef::point("prompt.scale.second").from(origin));
            }
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(factor) = factor(cx)
            && !(factor.is_finite() && factor >= SMALLEST_FACTOR)
        {
            return Err(Message::new("error.bad_factor"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(origin)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::point))
        else {
            return Ok(Made::default());
        };
        let mut guides: Vec<Curve> = cx.values[2..]
            .iter()
            .filter_map(Value::point)
            .filter_map(|p| Curve::line(origin, p).ok())
            .collect();
        let Some(factor) = factor(cx).filter(|f| f.is_finite() && *f >= SMALLEST_FACTOR) else {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        };
        let change = Change::Map(Similarity::scaling(origin, factor));
        let mut made = changed(cx, ids, &[change], cx.toggle(COPY), |_, _| None);
        made.guides.append(&mut guides);
        made.message = Some(Message::with("done.scale", vec![ids.len().to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 0.0, 0.0], [3.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document) -> Vec<Point> {
        doc.state().objects[0].curve().unwrap().sample().unwrap()
    }

    #[test]
    fn a_typed_factor_scales_about_the_origin() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "1,0", "2"]);
        assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [5.0, 0.0, 0.0]]);
    }

    #[test]
    fn two_reference_points_give_the_ratio_of_their_distances() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "0,0", "2,0", "0,6"]);
        assert_eq!(ends(&doc), [[3.0, 0.0, 0.0], [9.0, 0.0, 0.0]]);
    }

    #[test]
    fn a_circle_stays_a_circle() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 2.0).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["scale", "0,0", "3"]);
        let length = doc.state().objects[0].curve().unwrap().length().unwrap();
        assert!((length - std::f64::consts::TAU * 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_or_negative_factor_changes_nothing() {
        for factor in ["0", "-2"] {
            let mut doc = selected_line();
            run(&mut doc, &["scale", "0,0", factor]);
            assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
        }
    }

    #[test]
    fn a_reference_point_on_the_origin_changes_nothing() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "0,0", "0,0", "2,0"]);
        assert_eq!(ends(&doc), [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
    }

    #[test]
    fn copy_resizes_a_copy_and_keeps_the_original() {
        let mut doc = selected_line();
        run(&mut doc, &["scale", "copy", "0,0", "2"]);
        assert_eq!(doc.state().objects.len(), 2);
    }
}
