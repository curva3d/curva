//! `ExtrudeCrv`, as in Rhino: pulls curves straight out of the plane being drawn on into
//! surfaces, or into solids from closed curves (`Solid`). The distance follows the mouse, or is
//! typed; Enter uses the last one. `BothSides` goes as far each way; `DeleteInput` takes the
//! curves away. Rhino's `Direction`, `ToBoundary`, `SplitAtTangents` and `SetBasePoint` are still
//! to come (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point, cross, nested_groups, unit};

/// Shortest extrusion treated as real, in millimetres.
const SHORTEST: f64 = 1e-9;
const SOLID: &str = "Solid";
const BOTH_SIDES: &str = "BothSides";
const DELETE_INPUT: &str = "DeleteInput";

/// The recipe.
#[derive(Default)]
pub struct ExtrudeCrv;

/// The curves chosen, leaving out points.
fn chosen(cx: &Context<'_>) -> Vec<(u64, Curve)> {
    let Some(Value::Objects(ids)) = cx.values.first() else {
        return Vec::new();
    };
    curves_of(cx.doc, ids)
        .into_iter()
        .filter(|(_, curve)| !matches!(curve, Curve::Point { .. }))
        .collect()
}

/// Straight out of the plane being drawn on.
fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}

impl Recipe for ExtrudeCrv {
    const NAME: &'static str = "extrudecrv";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(SOLID, true),
        OptionDef::toggle(BOTH_SIDES, false),
        OptionDef::toggle(DELETE_INPUT, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrv.objects")
                .curves()
                .offering(&[]),
        ];
        // Where the distance is measured from: the start of the first curve.
        let base = chosen(cx)
            .first()
            .and_then(|(_, curve)| curve.sample().ok())
            .and_then(|points| points.first().copied());
        if let Some(base) = base {
            steps.push(
                StepDef::along("prompt.extrudecrv.distance", base, up(cx))
                    .remembered("Distance", 10.0),
            );
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(distance) = cx.values.get(1).and_then(Value::number)
            && distance.abs() <= SHORTEST
        {
            return Err(Message::new("error.bad_distance"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(distance) = cx
            .values
            .get(1)
            .and_then(Value::number)
            .filter(|d| d.abs() > SHORTEST)
        else {
            return Ok(Made::default());
        };
        let up = up(cx);
        let solid = cx.toggle(SOLID);
        // Both sides: start as far below as the extrusion goes above.
        let (lift, length) = if cx.toggle(BOTH_SIDES) {
            (-distance, 2.0 * distance)
        } else {
            (0.0, distance)
        };
        let curves: Vec<(u64, Curve)> = chosen(cx)
            .into_iter()
            .map(|(id, curve)| (id, curve.translated(up.map(|v| v * lift))))
            .collect();
        // As in Rhino, closed curves made solid are grouped: an outline and the curves inside
        // it become one solid with holes. Everything else is extruded on its own.
        let (closed, open): (Vec<_>, Vec<_>) = curves
            .iter()
            .cloned()
            .partition(|(_, curve)| solid && curve.is_closed().unwrap_or(false));
        let mut groups: Vec<(u64, Vec<Curve>, bool)> = open
            .into_iter()
            .map(|(id, curve)| (id, vec![curve], false))
            .collect();
        let outlines: Vec<Curve> = closed.iter().map(|(_, curve)| curve.clone()).collect();
        for group in nested_groups(&outlines, up) {
            let members = group.iter().map(|&i| outlines[i].clone()).collect();
            groups.push((closed[group[0]].0, members, true));
        }
        let mut made = Made::default();
        for (id, members, cap) in groups {
            let body = Body::extruded(&members, up, length, cap)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        if cx.toggle(DELETE_INPUT) {
            made.remove = curves.iter().map(|(id, _)| *id).collect();
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn selected_square(doc: &mut Document) -> u64 {
        let id = doc.add(
            Curve::polyline(
                vec![
                    [0.0; 3],
                    [10.0, 0.0, 0.0],
                    [10.0, 10.0, 0.0],
                    [0.0, 10.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        id
    }

    #[test]
    fn a_closed_curve_becomes_a_solid_and_the_curve_stays() {
        let mut doc = Document::default();
        selected_square(&mut doc);
        let mut processor = run(&mut doc, &["extrudecrv"]);
        assert!(!processor.preview(&doc, None, "5").is_empty());
        processor.submit(&mut doc, "5");
        assert_eq!(doc.state().objects.len(), 2);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), 500.0));
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn solid_off_leaves_an_open_surface() {
        let mut doc = Document::default();
        selected_square(&mut doc);
        run(&mut doc, &["extrudecrv", "Solid", "2"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(!body.is_solid());
        assert!(close(body.area().unwrap(), 80.0));
    }

    #[test]
    fn both_sides_goes_up_and_down() {
        let mut doc = Document::default();
        selected_square(&mut doc);
        run(&mut doc, &["extrudecrv", "b", "3"]);
        let (low, high) = doc.state().objects[1]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!(close(low[2], -3.0) && close(high[2], 3.0));
    }

    #[test]
    fn enter_repeats_the_last_distance() {
        let mut doc = Document::default();
        selected_square(&mut doc);
        run(&mut doc, &["extrudecrv", "4"]);
        let id = doc.state().objects[0].id;
        doc.select(&[id]);
        run(&mut doc, &["extrudecrv", ""]);
        let body = doc.state().objects[2].shape.body().unwrap();
        assert!(close(body.volume().unwrap(), 400.0));
    }

    #[test]
    fn curves_inside_an_outline_make_holes_in_one_solid() {
        let mut doc = Document::default();
        let outline = selected_square(&mut doc);
        let a = doc.add(Curve::circle([3.0, 5.0, 0.0], 1.0).unwrap());
        let b = doc.add(Curve::circle([7.0, 5.0, 0.0], 1.0).unwrap());
        let apart = doc.add(Curve::circle([30.0, 0.0, 0.0], 1.0).unwrap());
        doc.select(&[outline, a, b, apart]);
        run(&mut doc, &["extrudecrv", "2"]);
        let bodies: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .map(|body| body.volume().unwrap())
            .collect();
        let pi = std::f64::consts::PI;
        assert_eq!(bodies.len(), 2, "one solid with holes, one cylinder");
        assert!(close(bodies[0], 2.0 * (100.0 - 2.0 * pi)));
        assert!(close(bodies[1], 2.0 * pi));
    }

    #[test]
    fn an_open_curve_becomes_a_surface() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["extrudecrv", "3"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(close(body.area().unwrap(), 12.0));
    }

    #[test]
    fn delete_input_takes_the_curve_away() {
        let mut doc = Document::default();
        selected_square(&mut doc);
        run(&mut doc, &["extrudecrv", "deleteinput", "5"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(doc.state().objects[0].shape.body().is_some());
    }
}
