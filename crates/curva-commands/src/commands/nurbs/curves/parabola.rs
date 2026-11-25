//! The `Parabola` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, cross, dot, gap, sub, unit};

/// `Parabola`: its vertex, its focus, then an end point, which sets how far it reaches.
/// Rhino's option to start from the vertex rather than the focus.
pub(crate) const VERTEX: &str = "Vertex";

/// From the focus, the way the parabola opens and a point it reaches, its vertex and focal length,
/// as Rhino makes it (#707): the end is as far from the focus as from the directrix.
pub(crate) fn from_focus(
    focus: curva_kernel::Point,
    toward: curva_kernel::Point,
    end: curva_kernel::Point,
) -> Option<(curva_kernel::Point, curva_kernel::Point, f64)> {
    let axis = unit(sub(toward, focus))?;
    let to_end = sub(end, focus);
    let focal = (dot(to_end, to_end).sqrt() - dot(to_end, axis)) / 2.0;
    (focal > 1e-9).then(|| (plus(focus, axis, -focal), axis, focal))
}

#[derive(Default)]
pub struct Parabola;

impl Recipe for Parabola {
    const NAME: &'static str = "parabola";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode(VERTEX)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        // From the focus, as in Rhino; `Vertex` starts from the vertex.
        if cx.mode != Some(VERTEX) {
            let mut steps = vec![StepDef::point("prompt.parabola.focus_first").offering(&[VERTEX])];
            if let Some(f) = point(cx, 0) {
                steps.push(StepDef::point("prompt.parabola.direction").from(f));
                if point(cx, 1).is_some() {
                    steps.push(StepDef::point("prompt.parabola.end").from(f));
                }
            }
            return steps;
        }
        let mut steps = vec![StepDef::point("prompt.parabola.vertex")];
        if let Some(v) = point(cx, 0) {
            steps.push(StepDef::point("prompt.parabola.focus").from(v));
            if let Some(f) = point(cx, 1) {
                steps.push(StepDef::point("prompt.parabola.end").from(f));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(first), Some(second)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(end) = point(cx, 2) else {
            return Ok(Made {
                guides: Curve::line(first, second).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, y] = cx.plane();
        let (v, axis, focal) = if cx.mode == Some(VERTEX) {
            let axis =
                unit(sub(second, first)).ok_or_else(|| Message::new("error.reference_point"))?;
            (first, axis, gap(first, second))
        } else {
            from_focus(first, second, end).ok_or_else(|| Message::new("error.reference_point"))?
        };
        let side = cross(cross(x, y), axis);
        let half = dot(sub(end, v), side).abs();
        if half <= 1e-9 {
            return Err(Message::new("error.reference_point"));
        }
        let curve = parabola_piece(v, axis, side, focal, -half, half).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.parabola")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn parabola_paraboloid_and_centroids() {
        let mut doc = Document::default();
        run(&mut doc, &["parabola", "vertex", "0,0", "0,1", "4,0"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(
            points
                .iter()
                .all(|p| (p[1] - p[0] * p[0] / 4.0).abs() < 0.05),
            "{points:?}"
        );
        let mut doc = Document::default();
        run(&mut doc, &["paraboloid", "vertex", "0,0,0", "0,0,1", "4"]);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        // The volume of a paraboloid is half the cylinder around it: π r² h / 2, with r² = 4fh.
        let expected = std::f64::consts::PI * 16.0 * 4.0 / 2.0;
        assert!(
            (volume - expected).abs() < 1e-4 * expected,
            "{volume} against {expected}"
        );
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [2.0, 4.0, 6.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["volumecentroid"]);
        let Some(Curve::Point { at }) = doc.state().objects.last().unwrap().curve() else {
            panic!("no point");
        };
        assert!(curva_kernel::gap(*at, [1.0, 2.0, 3.0]) < 1e-6, "{at:?}");
    }
}
