//! The `Silhouette` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;

/// `Silhouette`: the outlines of surfaces and solids seen straight down on the construction plane.
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
                    shape: Shape::Curve(curve),
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
