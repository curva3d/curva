//! The `Match` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curve_at;
use crate::recipe::{Context, Made, ON_CURVE, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Point, Spline, dot};

/// `Match`: changes the end of a curve picked near it so it meets the end of another picked near
/// it, in position, along its tangent or in curvature (the default, as in Rhino 8, #770), moving
/// only the control points near that end.
#[derive(Default)]
pub struct Match;

/// The spline with enough control points that moving the last three leaves its start alone.
fn with_room(mut spline: Spline, needed: usize) -> Spline {
    if spline.degree < 2 {
        spline = spline.elevated(2 - spline.degree);
    }
    while spline.points.len() < needed + 1 {
        let (a, b) = spline.domain();
        // Split the last span, so the start is the last to be affected.
        let last = spline.knots[spline.points.len() - 1].max(a);
        spline = spline.inserted((last + b) / 2.0, 1);
    }
    spline
}

// CUSTOM KERNEL OPERATION: curve-matching
fn matched_end(spline: Spline, end: Point, d1: Point, d2: Point, how: Continuity) -> Spline {
    let needed = match how {
        Continuity::Position => 1,
        Continuity::Tangency => 2,
        Continuity::Curvature => 3,
    };
    let mut s = with_room(spline, needed);
    let (p, n) = (s.degree as f64, s.points.len() - 1);
    let deg = s.degree;
    let u = s.knots.clone();
    let weight = |q: [f64; 4]| q[3];
    let put = |s: &mut Spline, i: usize, at: Point| {
        let w = weight(s.points[i]);
        s.points[i] = [at[0] * w, at[1] * w, at[2] * w, w];
    };
    put(&mut s, n, end);
    if how == Continuity::Position {
        return s;
    }
    let span1 = u[n + deg] - u[n];
    let p1 = plus(end, d1, -span1 / p);
    put(&mut s, n - 1, p1);
    if how == Continuity::Tangency {
        return s;
    }
    // Curvature as Rhino 8 matches it (#770): the third point only moves sideways from the
    // second, along the target's normal, as far as the target's curvature needs; on a straight
    // target it sits on the second.
    let bend = curvature(d1, d2);
    let amount = length(bend);
    put(&mut s, n - 2, p1);
    if amount < 1e-12 {
        return s;
    }
    let normal = scaled(bend, 1.0 / amount);
    put(&mut s, n - 2, plus(p1, normal, 1.0));
    // The curvature one unit sideways gives, along the normal: it grows in step with the offset.
    let (_, e1, e2) = s.end_derivatives();
    let per_unit = dot(curvature(e1, e2), normal);
    if per_unit.abs() < 1e-12 {
        put(&mut s, n - 2, p1);
        return s;
    }
    put(&mut s, n - 2, plus(p1, normal, amount / per_unit));
    s
}

/// part of `d2` across `d1`, over the speed squared.
fn curvature(d1: Point, d2: Point) -> Point {
    let speed2 = dot(d1, d1).max(1e-24);
    let along = dot(d2, d1) / speed2;
    std::array::from_fn(|k| (d2[k] - along * d1[k]) / speed2)
}

impl Recipe for Match {
    const NAME: &'static str = "match";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        // Each curve picked near the end that matters, as Rhino asks (#770).
        let mut steps = vec![StepDef::pick("prompt.match.change")];
        if !cx.values.is_empty() {
            steps.push(StepDef::pick("prompt.match.target"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(at_a), Some(at_b)) = (
            cx.values.first().and_then(Value::point),
            cx.values.get(1).and_then(Value::point),
        ) else {
            return Ok(Made::default());
        };
        let (Some((id, a)), Some((b_id, b))) = (
            curve_at(cx.doc, at_a, ON_CURVE).map_err(|e| failed(&e))?,
            curve_at(cx.doc, at_b, ON_CURVE).map_err(|e| failed(&e))?,
        ) else {
            return Ok(Made::default());
        };
        if id == b_id {
            return Ok(Made::default());
        }
        let (a, b) = (
            Spline::of(&a).map_err(|e| failed(&e))?,
            Spline::of(&b).map_err(|e| failed(&e))?,
        );
        // The end of each curve nearer where it was picked.
        let (a_end, b_end) = (picked_end(&a, at_a), picked_end(&b, at_b));
        // The target: where b is, and how it carries on, seen from a arriving at it.
        let (end, bd1, bd2) = leaving(&b, b_end);
        let target_d1 = scaled(bd1, -1.0);
        let worked = if a_end { a.clone() } else { a.reversed() };
        let (_, old_d1, _) = worked.end_derivatives();
        let speed = length(old_d1).max(1e-12);
        let bspeed = length(target_d1).max(1e-12);
        let d1 = scaled(target_d1, speed / bspeed);
        let d2 = scaled(bd2, (speed / bspeed).powi(2));
        let changed = matched_end(worked, end, d1, d2, continuity(cx, Continuity::Curvature));
        let changed = if a_end { changed } else { changed.reversed() };
        let curve = changed.to_curve().map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(id, Shape::Curve(curve))],
            message: Some(Message::new("done.match")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {

    use curva_document::Document;
    use curva_kernel::{Curve, Spline, gap};

    #[test]
    fn match_moves_the_nearest_end_onto_the_other_curve() {
        let mut doc = Document::default();
        let a = doc.add(
            Curve::control_point(
                vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0], [6.0, 1.0, 0.0]],
                3,
                false,
            )
            .unwrap(),
        );
        doc.add(Curve::line([7.0, 0.0, 0.0], [12.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "match");
        processor.submit(&mut doc, "Curvature");
        processor.click(&mut doc, [6.0, 1.0, 0.0]);
        processor.click(&mut doc, [7.0, 0.0, 0.0]);
        let matched = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == a)
            .unwrap()
            .curve()
            .unwrap()
            .clone();
        let spline = Spline::of(&matched).unwrap();
        let (s, e) = spline.domain();
        assert!(gap(spline.point(e), [7.0, 0.0, 0.0]) < 1e-9);
        assert!(gap(spline.point(s), [0.0; 3]) < 1e-9);
        let (_, d1, d2) = spline.end_derivatives();
        assert!(d1[1].abs() < 1e-9 && d1[0] > 0.0, "{d1:?}");
        assert!(d2[1].abs() < 1e-6, "{d2:?}");
    }

    /// B's control points after `Match` (its default mode) onto `target`.
    fn matched_onto(target: Curve, pick: [f64; 3]) -> Vec<[f64; 3]> {
        let mut doc = Document::default();
        let points = vec![
            [6.0, 1.0, 0.0],
            [7.0, 3.0, 0.0],
            [9.0, 2.0, 0.0],
            [10.0, 4.0, 0.0],
        ];
        let b = doc.add(Curve::control_point(points, 3, false).unwrap());
        doc.add(target);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "match");
        processor.click(&mut doc, pick);
        processor.click(&mut doc, [5.0, 0.0, 0.0]);
        let object = doc.state().objects.iter().find(|o| o.id == b).unwrap();
        object.curve().unwrap().control_points().unwrap()
    }

    #[test]
    fn curvature_is_the_default_and_moves_the_third_point_sideways_as_in_rhino() {
        // The testing stream's case (#770), measured with Rhino 8's -Match.
        let close = |p: [f64; 3], q: [f64; 2]| gap(p, [q[0], q[1], 0.0]) < 1e-3;
        let straight = matched_onto(
            Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap(),
            [6.0, 1.0, 0.0],
        );
        let rhino = [[5.0, 0.0], [7.2361, 0.0], [7.2361, 0.0], [10.0, 4.0]];
        assert!(
            straight.iter().zip(rhino).all(|(p, q)| close(*p, q)),
            "{straight:?}"
        );
        let curved = Curve::control_point(
            vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, -1.0, 0.0], [5.0, 0.0, 0.0]],
            3,
            false,
        )
        .unwrap();
        let bent = matched_onto(curved, [6.0, 1.0, 0.0]);
        let rhino = [[5.0, 0.0], [6.5811, 1.5811], [0.3311, 7.8311], [10.0, 4.0]];
        assert!(
            bent.iter().zip(rhino).all(|(p, q)| close(*p, q)),
            "{bent:?}"
        );
    }

    #[test]
    fn the_end_picked_is_the_one_that_moves_as_in_rhino() {
        // Rhino 8 (#770): B picked near its far end (10,4) moves that end onto A.
        let close = |p: [f64; 3], q: [f64; 2]| gap(p, [q[0], q[1], 0.0]) < 1e-3;
        let far = matched_onto(
            Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap(),
            [10.0, 4.0, 0.0],
        );
        let rhino = [[6.0, 1.0], [7.2361, 0.0], [7.2361, 0.0], [5.0, 0.0]];
        assert!(far.iter().zip(rhino).all(|(p, q)| close(*p, q)), "{far:?}");
    }
}
