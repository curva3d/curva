//! `Revolve`, as in Rhino: spins curves about an axis into surfaces. Enter at the angle, or
//! `FullCircle`, makes a whole turn; `DeleteInput` takes the curves away. A closed curve turned
//! part of the way is closed at its ends. Rhino's `AskForStartAngle`, `Deformable`, `PointCount`
//! and `SplitAtTangents`, and Enter for the plane's Z axis, are still to come
//! (`docs/commands/rhino-reference.md`).

use super::curves_of;
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Similarity, gap, sub, unit};

/// A whole turn, in degrees.
const FULL_TURN: f64 = 360.0;
/// Shortest axis treated as real, in millimetres.
const SHORTEST_AXIS: f64 = 1e-9;
/// Copies of the curves shown turned about the axis in the preview.
const PREVIEW_STEPS: u32 = 8;
const FULL_CIRCLE: &str = "FullCircle";
const DELETE_INPUT: &str = "DeleteInput";

/// The recipe.
#[derive(Default)]
pub struct Revolve;

impl Recipe for Revolve {
    const NAME: &'static str = "revolve";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::finish(FULL_CIRCLE),
        OptionDef::toggle(DELETE_INPUT, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.revolve.objects")
                .curves()
                .offering(&[DELETE_INPUT]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.revolve.axis_start").offering(&[DELETE_INPUT]));
        }
        if let Some(start) = cx.values.get(1).and_then(Value::point) {
            steps.push(
                StepDef::point("prompt.revolve.axis_end")
                    .from(start)
                    .offering(&[DELETE_INPUT]),
            );
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::number("prompt.revolve.angle").optional());
        }
        steps
    }

    /// An axis with no length, or an angle outside a whole turn, is asked again.
    fn accepts(&self, cx: &Context<'_>) -> bool {
        let axis_ok = match (
            cx.values.get(1).and_then(Value::point),
            cx.values.get(2).and_then(Value::point),
        ) {
            (Some(start), Some(end)) => gap(start, end) >= SHORTEST_AXIS,
            _ => true,
        };
        let angle_ok = cx
            .values
            .get(3)
            .and_then(Value::number)
            .is_none_or(|degrees| degrees > 0.0 && degrees <= FULL_TURN);
        axis_ok && angle_ok
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(start)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::point))
        else {
            return Ok(Made::default());
        };
        let Some(end) = cx.values.get(2).and_then(Value::point) else {
            return Ok(Made::default());
        };
        let axis_line: Vec<Curve> = Curve::line(start, end).ok().into_iter().collect();
        // While the axis is being placed, only the axis shows, as turning is heavy.
        if cx.answered < 3 || gap(start, end) < SHORTEST_AXIS {
            return Ok(Made {
                guides: axis_line,
                ..Made::default()
            });
        }
        // Until an angle is given, and with Enter or FullCircle, a whole turn.
        let degrees = match (cx.mode, cx.values.get(3).and_then(Value::number)) {
            (Some(FULL_CIRCLE), _) | (_, None) => FULL_TURN,
            (_, Some(degrees)) => degrees.clamp(f64::EPSILON, FULL_TURN),
        };
        let mut made = Made {
            guides: axis_line,
            ..Made::default()
        };
        // The preview also shows the curves turned step by step, to show the way round.
        if cx.previewing
            && let Some(axis) = unit(sub(end, start))
        {
            for step in 1..=PREVIEW_STEPS {
                let angle = degrees * f64::from(step) / f64::from(PREVIEW_STEPS);
                let turn = Similarity::rotation(start, axis, angle.to_radians());
                made.guides.extend(
                    curves_of(cx.doc, ids)
                        .iter()
                        .map(|(_, curve)| curve.mapped(&turn)),
                );
            }
        }
        for (id, curve) in curves_of(cx.doc, ids) {
            if matches!(curve, Curve::Point { .. }) {
                continue;
            }
            let cap = degrees < FULL_TURN && curve.is_closed().unwrap_or(false);
            let body = Body::revolved(&[curve], start, sub(end, start), degrees.to_radians(), cap)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
            if cx.toggle(DELETE_INPUT) {
                made.remove.push(id);
            }
        }
        made.message = Some(Message::with(
            "done.revolve",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn selected_profile(doc: &mut Document) {
        let id = doc.add(
            Curve::polyline(
                vec![
                    [3.0, 0.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [4.0, 0.0, 2.0],
                    [3.0, 0.0, 2.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[id]);
    }

    #[test]
    fn a_full_turn_about_an_axis_makes_a_ring() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        let mut processor = run(&mut doc, &["revolve", "0,0,0", "0,0,1"]);
        assert!(processor.preview(&doc, None, "").len() > 8);
        processor.submit(&mut doc, "");
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), PI * 7.0 * 2.0));
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn part_of_a_turn_closes_the_ends() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        run(&mut doc, &["revolve", "0,0,0", "0,0,1", "90"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), PI * 7.0 * 2.0 / 4.0));
    }

    #[test]
    fn an_angle_over_a_turn_is_asked_again() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        let processor = run(&mut doc, &["revolve", "0,0,0", "0,0,1", "400"]);
        assert!(processor.is_running());
    }

    #[test]
    fn full_circle_makes_a_whole_turn_at_once() {
        let mut doc = Document::default();
        selected_profile(&mut doc);
        run(&mut doc, &["revolve", "0,0,0", "0,0,1", "fullcircle"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), PI * 7.0 * 2.0));
    }
}
