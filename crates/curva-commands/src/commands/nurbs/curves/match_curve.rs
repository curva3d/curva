//! The `Match` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Point, Spline};

/// `Match`: changes the end of a curve nearest another so it meets that curve, in position,
/// along its tangent (the default) or in curvature, moving only the control points near that end.
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
    let span2 = u[n + deg - 1] - u[n];
    let q0: Point = std::array::from_fn(|k| d1[k] - d2[k] * span2 / (p - 1.0));
    let span3 = u[n + deg - 1] - u[n - 1];
    put(&mut s, n - 2, plus(p1, q0, -span3 / p));
    s
}

impl Recipe for Match {
    const NAME: &'static str = "match";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.match.change").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.match.target").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (first, second) = (
            curves_of(cx.doc, objects(cx, 0)),
            curves_of(cx.doc, objects(cx, 1)),
        );
        let (Some((id, a)), Some((_, b))) = (first.first(), second.first()) else {
            return Ok(Made::default());
        };
        let (a, b) = (
            Spline::of(a).map_err(|e| failed(&e))?,
            Spline::of(b).map_err(|e| failed(&e))?,
        );
        let (a_end, b_end) = nearest_ends(&a, &b);
        // The target: where b is, and how it carries on, seen from a arriving at it.
        let (end, bd1, bd2) = leaving(&b, b_end);
        let target_d1 = scaled(bd1, -1.0);
        let worked = if a_end { a.clone() } else { a.reversed() };
        let (_, old_d1, _) = worked.end_derivatives();
        let speed = length(old_d1).max(1e-12);
        let bspeed = length(target_d1).max(1e-12);
        let d1 = scaled(target_d1, speed / bspeed);
        let d2 = scaled(bd2, (speed / bspeed).powi(2));
        let changed = matched_end(worked, end, d1, d2, continuity(cx, Continuity::Tangency));
        let changed = if a_end { changed } else { changed.reversed() };
        let curve = changed.to_curve().map_err(|e| failed(&e))?;
        Ok(Made {
            replace: vec![(*id, Shape::Curve(curve))],
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
        let b = doc.add(Curve::line([7.0, 0.0, 0.0], [12.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "match");
        processor.submit(&mut doc, "Curvature");
        doc.select(&[a]);
        processor.submit(&mut doc, "");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
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
}
