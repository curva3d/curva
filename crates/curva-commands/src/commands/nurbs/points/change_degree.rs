//! The `ChangeDegree` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::Curve;

/// `ChangeDegree`: gives curves a new degree. Raising it keeps the shape exactly (Rhino's
/// `Deformable=No`, adding control points) and lowering it keeps it within 0.01, both through
/// the engine; `Deformable=Yes` keeps the control
/// points and lets the shape change. Closed curves stay closed.
#[derive(Default)]
pub struct ChangeDegree;

impl Recipe for ChangeDegree {
    const NAME: &'static str = "changedegree";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("Degree", 3, 1, 11),
        OptionDef::toggle("Deformable", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.changedegree.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::number("prompt.changedegree.degree")
                    .remembered("Degree", 3.0)
                    .offering(&["Deformable"]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(degree)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::number))
        else {
            return Ok(Made::default());
        };
        if !(1.0..=11.0).contains(&degree) || degree.fract() != 0.0 {
            return Err(Message::with(
                "error.option_whole",
                vec!["Degree".to_owned(), "1".to_owned(), "11".to_owned()],
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let degree = degree as usize;
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, ids) {
            let spline = curva_kernel::Spline::of(&curve).map_err(|e| failed(&e))?;
            if spline.degree == degree {
                continue;
            }
            // The engine: raised exactly, or lowered keeping the shape within 0.01; a curve that
            // cannot be lowered that closely keeps its control points instead (`Deformable`).
            let exact = if cx.toggle("Deformable") {
                None
            } else {
                curve.with_degree(degree, 0.01).ok().map(|(c, _)| c)
            };
            let changed = if let Some(exact) = exact {
                exact
            } else {
                let mut points = spline.control_points();
                let closed = curve.is_closed().unwrap_or(false);
                if closed
                    && points.len() > 2
                    && curva_kernel::gap(points[0], points[points.len() - 1]) < 1e-9
                {
                    points.pop();
                }
                Curve::control_point(points, degree, closed).map_err(|e| failed(&e))?
            };
            made.replace.push((id, Shape::Curve(changed)));
        }
        made.message = Some(Message::with(
            "done.changedegree",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn control_points_and_degree() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(
                vec![
                    [0.0; 3],
                    [5.0, 5.0, 0.0],
                    [10.0, 0.0, 0.0],
                    [15.0, 5.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["changedegree", "2"]);
        assert!(matches!(
            doc.state().objects[0].curve(),
            Some(Curve::Nurbs { degree: 2, .. })
        ));
    }

    #[test]
    fn raising_the_degree_keeps_the_shape_and_closed_curves_stay_closed() {
        let wavy = Curve::control_point(
            vec![
                [0.0; 3],
                [5.0, 5.0, 0.0],
                [10.0, 0.0, 0.0],
                [15.0, 5.0, 0.0],
                [20.0, 0.0, 0.0],
            ],
            3,
            false,
        )
        .unwrap();
        let mut doc = Document::default();
        let id = doc.add(wavy.clone());
        doc.select(&[id]);
        run(&mut doc, &["changedegree", "5"]);
        let raised = doc.state().objects[0].curve().unwrap().clone();
        assert!(matches!(raised, Curve::Nurbs { degree: 5, .. }));
        let (before, after) = (
            curva_kernel::Spline::of(&wavy).unwrap(),
            curva_kernel::Spline::of(&raised).unwrap(),
        );
        for i in 0..=20 {
            let u = f64::from(i) / 20.0;
            assert!(curva_kernel::gap(before.point(u), after.point(u)) < 1e-9);
        }
        // Closed curves stay closed, raised or lowered.
        for degree in ["4", "2"] {
            let mut doc = Document::default();
            let id = doc.add(Curve::circle([0.0; 3], 5.0).unwrap());
            doc.select(&[id]);
            run(&mut doc, &["changedegree", degree]);
            let changed = doc.state().objects[0].curve().unwrap();
            assert!(changed.is_closed().unwrap(), "degree {degree}");
        }
    }
}
