//! `ExtrudeCrv`: pulls curves straight out of the plane being drawn on into surfaces, or into
//! solids from closed curves, as in Rhino.

use super::{ask_objects, curves_of, geometry_failed, plane_normal, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_kernel::{Body, Curve, Point, gap, nested_groups, unit};

/// The distance used until another is typed, in millimetres.
const STARTING_DISTANCE: f64 = 10.0;
/// Where the document remembers the last distance and options.
const DISTANCE: &str = "extrudecrv.distance";
const SOLID: &str = "extrudecrv.solid";
const BOTH_SIDES: &str = "extrudecrv.bothsides";
const SOLID_OPTION: &str = "Solid";
const BOTH_SIDES_OPTION: &str = "BothSides";
/// Lines drawn along the extrusion in the preview, spread over each curve.
const PREVIEW_RUNGS: usize = 8;

/// The running command.
#[derive(Default)]
pub struct ExtrudeCrv {
    ids: Option<Vec<u64>>,
}

fn on(doc: &Document, key: &str, starting: bool) -> bool {
    doc.remembered(key).map_or(starting, |value| value != 0.0)
}

fn yes_no(value: bool) -> String {
    if value { "Yes" } else { "No" }.to_owned()
}

/// What the current settings are: distance, solid, both sides.
fn settings(doc: &Document) -> (f64, bool, bool) {
    (
        doc.remembered(DISTANCE).unwrap_or(STARTING_DISTANCE),
        on(doc, SOLID, true),
        on(doc, BOTH_SIDES, false),
    )
}

impl ExtrudeCrv {
    /// Where a clicked distance is measured from: the start of the first curve.
    fn base(&self, doc: &Document) -> Option<Point> {
        let ids = self.ids.as_ref()?;
        curves_of(doc, ids)
            .first()
            .and_then(|(_, curve)| curve.sample().ok())
            .and_then(|points| points.first().copied())
    }

    fn ask(&self, doc: &Document) -> Step {
        let (distance, solid, both) = settings(doc);
        Step::Ask(Request {
            prompt: Message::with(
                "prompt.extrudecrv.distance",
                vec![distance.to_string(), yes_no(solid), yes_no(both)],
            ),
            options: vec![SOLID_OPTION, BOTH_SIDES_OPTION],
            allow_empty: true,
            from: self.base(doc),
            ..Request::new("prompt.extrudecrv.distance", Kind::Number)
        })
    }

    fn make(&self, doc: &mut Document, distance: f64) -> Step {
        let Some(ids) = self.ids.clone() else {
            return unexpected();
        };
        let (_, solid, both) = settings(doc);
        let up = unit(plane_normal(doc)).unwrap_or([0.0, 0.0, 1.0]);
        // Both sides: start as far below as the extrusion goes above.
        let (lift, length) = if both {
            (-distance, 2.0 * distance)
        } else {
            (0.0, distance)
        };
        let curves: Vec<(u64, Curve)> = curves_of(doc, &ids)
            .into_iter()
            .map(|(id, curve)| (id, curve.translated(up.map(|v| v * lift))))
            .collect();
        // As in Rhino, closed curves made solid are grouped: an outline and the curves inside
        // it become one solid with holes. Everything else is extruded on its own.
        let (closed, open): (Vec<_>, Vec<_>) = curves
            .into_iter()
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
        let mut made = Vec::new();
        for (id, members, cap) in groups {
            match Body::extruded(&members, up, length, cap) {
                Ok(body) => made.push((id, body)),
                Err(error) => return geometry_failed(&error),
            }
        }
        let count = made.len();
        for (source, body) in made {
            let layer = doc.layer_of(source);
            doc.add_shape_on(curva_document::Shape::Body(body), layer);
        }
        Step::Done(Some(Message::with(
            "done.extrudecrv",
            vec![count.to_string()],
        )))
    }
}

impl Command for ExtrudeCrv {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.ids.is_none() {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            let curves: Vec<u64> = curves_of(doc, &chosen)
                .into_iter()
                .filter(|(_, curve)| !matches!(curve, Curve::Point { .. }))
                .map(|(id, _)| id)
                .collect();
            if curves.is_empty() {
                return ask_objects("prompt.extrudecrv.objects");
            }
            self.ids = Some(curves);
            return self.ask(doc);
        }
        match answer {
            Some(Answer::Option(SOLID_OPTION)) => {
                let flipped = !on(doc, SOLID, true);
                doc.remember(SOLID, f64::from(u8::from(flipped)));
                self.ask(doc)
            }
            Some(Answer::Option(_)) => {
                let flipped = !on(doc, BOTH_SIDES, false);
                doc.remember(BOTH_SIDES, f64::from(u8::from(flipped)));
                self.ask(doc)
            }
            Some(Answer::Empty) => {
                let (distance, ..) = settings(doc);
                self.make(doc, distance)
            }
            Some(Answer::Number(distance)) if distance.abs() > 1e-9 => {
                doc.remember(DISTANCE, distance);
                self.make(doc, distance)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let (remembered, _, both) = settings(doc);
        let distance = live
            .number
            .or_else(|| {
                let base = self.base(doc)?;
                live.cursor.map(|cursor| gap(cursor, base))
            })
            .filter(|d| d.abs() > 1e-9)
            .unwrap_or(remembered);
        let up = unit(plane_normal(doc)).unwrap_or([0.0, 0.0, 1.0]);
        let shift = |by: f64| up.map(|v| v * by);
        let (low, high) = if both {
            (-distance, distance)
        } else {
            (0.0, distance)
        };
        let mut shown = Vec::new();
        for (_, curve) in curves_of(doc, ids) {
            shown.push(curve.translated(shift(high)));
            if both {
                shown.push(curve.translated(shift(low)));
            }
            let Ok(points) = curve.sample() else {
                continue;
            };
            let step = (points.len() / PREVIEW_RUNGS).max(1);
            for p in points.iter().step_by(step).chain(points.last()) {
                let from = [0, 1, 2].map(|k| p[k] + up[k] * low);
                let to = [0, 1, 2].map(|k| p[k] + up[k] * high);
                shown.extend(Curve::line(from, to));
            }
        }
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
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
}
