//! The `SimplifyCrv` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Point, cross, dot, gap, sub, unit};

/// The centre of the circle through three points, if they are not in a line.
fn circle_through(a: Point, b: Point, c: Point) -> Option<Point> {
    let (ab, ac) = (sub(b, a), sub(c, a));
    let n = cross(ab, ac);
    let nn = dot(n, n);
    if nn < 1e-18 {
        return None;
    }
    let term1 = scaled(cross(n, ab), dot(ac, ac));
    let term2 = scaled(cross(ac, n), dot(ab, ab));
    Some(plus(
        a,
        std::array::from_fn(|k| (term1[k] + term2[k]) / (2.0 * nn)),
        1.0,
    ))
}

// CUSTOM KERNEL OPERATION: curve-simplifying
fn simpler(piece: &Curve, tolerance: f64) -> Option<Curve> {
    if matches!(
        piece,
        Curve::Line { .. } | Curve::Arc { .. } | Curve::Circle { .. }
    ) {
        return None;
    }
    let points = piece.divided(64).ok()?;
    let (start, end) = (points[0], points[points.len() - 1]);
    let chord = unit(sub(end, start));
    if let Some(dir) = chord {
        let far = points
            .iter()
            .map(|p| {
                let d = sub(*p, start);
                length(sub(d, scaled(dir, dot(d, dir))))
            })
            .fold(0.0, f64::max);
        if far <= tolerance {
            return Curve::line(start, end).ok();
        }
    }
    let middle = points[points.len() / 2];
    let centre = circle_through(start, middle, end)?;
    let radius = gap(centre, start);
    if points
        .iter()
        .all(|p| (gap(*p, centre) - radius).abs() <= tolerance)
    {
        let x = unit(sub(start, centre))?;
        let n = unit(cross(x, sub(middle, centre)))?;
        let y = cross(n, x);
        return Curve::arc(centre, start, end, x, y).ok();
    }
    None
}

/// `SimplifyCrv`: replaces the parts of curves that are lines or arcs within the tolerance with
/// exact lines and arcs.
#[derive(Default)]
pub struct SimplifyCrv;

impl Recipe for SimplifyCrv {
    const NAME: &'static str = "simplifycrv";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.simplifycrv.objects").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let tolerance = 1e-3;
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let pieces = curve.exploded().unwrap_or_else(|| vec![curve.clone()]);
            let mut changed = false;
            let simplified: Vec<Curve> = pieces
                .iter()
                .map(|piece| {
                    simpler(piece, tolerance).map_or_else(
                        || piece.clone(),
                        |s| {
                            changed = true;
                            s
                        },
                    )
                })
                .collect();
            if !changed {
                continue;
            }
            let whole = if simplified.len() == 1 {
                simplified[0].clone()
            } else {
                Curve::compound(simplified).map_err(|e| failed(&e))?
            };
            made.replace.push((id, Shape::Curve(whole)));
        }
        made.message = Some(Message::with(
            "done.simplifycrv",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Spline};

    #[test]
    fn simplify_turns_straight_and_round_nurbs_into_lines_and_arcs() {
        let mut doc = Document::default();
        let straight = doc.add(
            Curve::control_point(
                vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                3,
                false,
            )
            .unwrap(),
        );
        let round = doc.add(
            Spline::of(
                &Curve::arc(
                    [0.0; 3],
                    [5.0, 0.0, 0.0],
                    [0.0, 5.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .unwrap(),
            )
            .unwrap()
            .to_curve()
            .unwrap(),
        );
        doc.select(&[straight, round]);
        run(&mut doc, &["simplifycrv"]);
        let kinds: Vec<bool> = doc
            .state()
            .objects
            .iter()
            .map(|o| matches!(o.curve(), Some(Curve::Line { .. } | Curve::Arc { .. })))
            .collect();
        assert_eq!(kinds, vec![true, true]);
    }
}
