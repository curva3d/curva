//! The `Silhouette` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, cross, gap, sub, unit};

/// `Silhouette`: the outlines of surfaces and solids seen straight down on the construction plane.
/// How far, as a share of an outline's size, its points may stray from a circle or a line and it be
/// made that exactly.
const FIT: f64 = 1e-4;

/// The outline the engine traced through points, made exact where it is a circle or a line and
/// otherwise smooth through its points, as Rhino fits its silhouettes (#712: a sphere's outline
/// came out as a 64-sided polyline).
fn fitted(traced: Curve) -> Curve {
    let Ok(mut points) = traced.control_points() else {
        return traced;
    };
    let closed = traced.is_closed().unwrap_or(false);
    if closed && points.len() > 1 && gap(points[0], points[points.len() - 1]) < 1e-12 {
        points.pop();
    }
    let n = points.len();
    if n < 3 {
        return traced;
    }
    // Only a fine tracing of a curved outline is fitted: a turn sharper than about 20° between
    // two pieces is a real corner, as a box's, kept as it is.
    let corner = (1..n - 1).any(|k| {
        let (u, v) = (sub(points[k], points[k - 1]), sub(points[k + 1], points[k]));
        match (unit(u), unit(v)) {
            (Some(u), Some(v)) => curva_kernel::dot(u, v) < SMOOTH_TURN,
            _ => true,
        }
    });
    if corner {
        return traced;
    }
    let size = points
        .iter()
        .map(|p| gap(*p, points[0]))
        .fold(0.0, f64::max);
    let tolerance = FIT * size.max(1e-9);
    // A straight run of points is one line.
    let (first, last) = (points[0], points[n - 1]);
    if !closed
        && let Some(along) = unit(sub(last, first))
        && points.iter().all(|p| {
            let d = sub(*p, first);
            let along_d = curva_kernel::dot(d, along);
            gap(d, along.map(|v| v * along_d)) <= tolerance
        })
    {
        return Curve::line(first, last).unwrap_or(traced);
    }
    // A circle through three points spread round it, if every point lies on it.
    let (a, b, c) = (points[0], points[n / 3], points[2 * n / 3]);
    if closed
        && let Some(axes) = unit(cross(sub(b, a), sub(c, a)))
            .and_then(|normal| Some((unit(sub(b, a))?, normal)))
            .map(|(x, normal)| [x, cross(normal, x)])
        && let Some((centre, radius)) = crate::commands::circle_through(a, b, c, axes)
        && points
            .iter()
            .all(|p| (gap(*p, centre) - radius).abs() <= tolerance)
        && let Ok(circle) = Curve::circle_on(centre, radius, axes[0], axes[1])
    {
        return circle;
    }
    Curve::interpolated(points, closed).unwrap_or(traced)
}

/// The cosine of the sharpest turn a traced curved outline makes between two pieces (about 20°).
const SMOOTH_TURN: f64 = 0.94;

#[derive(Default)]
pub struct Silhouette;

impl Recipe for Silhouette {
    const NAME: &'static str = "silhouette";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.silhouette.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let view = up(cx).map(|v| -v);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            for curve in body.silhouette(view).map_err(|e| failed(&e))? {
                made.derived.push(Derived {
                    shape: Shape::Curve(fitted(curve)),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.silhouette",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    #[test]
    fn a_spheres_outline_is_an_exact_circle_as_in_rhino() {
        let mut doc = Document::default();
        let ball = doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        doc.select(&[ball]);
        run(&mut doc, &["silhouette"]);
        let outline = doc.state().objects.last().unwrap().curve().unwrap();
        assert!(matches!(outline, Curve::Circle { .. }), "{outline:?}");
        assert!((outline.length().unwrap() - 31.4159).abs() < 1e-3);
    }

    #[test]
    fn silhouettes_unrolling_extruding_faces_and_fillets() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap());
        doc.select(&[cube]);
        run(&mut doc, &["silhouette"]);
        assert!(doc.state().objects.len() > 1);
        let mut doc = Document::default();
        let low = Curve::arc(
            [0.0; 3],
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let high = Curve::arc(
            [0.0, 0.0, 3.0],
            [2.0, 0.0, 3.0],
            [0.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        let strip = doc.add_body(Body::ruled(&low, &high, false).unwrap());
        doc.select(&[strip]);
        run(&mut doc, &["unrollsrf"]);
        let flat = last_body(&doc);
        assert!(flat.bounds().unwrap().1[2].abs() < 1e-9);
        let mut doc = Document::default();
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let mesh = cube.mesh().unwrap();
        let top = mesh
            .triangle_faces
            .iter()
            .zip(&mesh.triangles)
            .find(|(_, t)| {
                t.iter()
                    .all(|i| (mesh.positions[*i as usize][2] - 1.0).abs() < 1e-9)
            })
            .map(|(f, _)| *f)
            .unwrap();
        let id = doc.add_body(cube);
        doc.select_faces(&[(id, top)]);
        run(&mut doc, &["extrudeface", "", "0.5"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        let mut doc = Document::default();
        let floor = doc.add_body(
            Body::ruled(
                &line([0.0; 3], [10.0, 0.0, 0.0]),
                &line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]),
                false,
            )
            .unwrap(),
        );
        let wall = doc.add_body(
            Body::ruled(
                &line([0.0; 3], [10.0, 0.0, 0.0]),
                &line([0.0, 0.0, 10.0], [10.0, 0.0, 10.0]),
                false,
            )
            .unwrap(),
        );
        let mut processor = crate::Processor::default();
        doc.select(&[floor]);
        processor.submit(&mut doc, "filletsrf");
        doc.select(&[wall]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "2");
        let area = last_body(&doc).area().unwrap();
        assert!(
            (area - std::f64::consts::PI * 10.0).abs() < 0.02 * std::f64::consts::PI * 10.0,
            "{area}"
        );
    }
}
