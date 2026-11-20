//! The `ExtractIsocurve` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

/// `ExtractIsocurve`: copies a surface's curve through a point, along `U`, `V` or `Both` (the
/// default).
#[derive(Default)]
pub struct ExtractIsocurve;

impl Recipe for ExtractIsocurve {
    const NAME: &'static str = "extractisocurve";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("U"),
        OptionDef::mode("V"),
        OptionDef::mode("Both"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extractisocurve.surface")
                .bodies()
                .one(),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.extractisocurve.point"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((id, body)), Some(at)) = (first_body(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let (face, _, _) = body.nearest_face(at).map_err(|e| failed(&e))?;
        let ways: &[bool] = match cx.mode {
            Some("U") => &[false],
            Some("V") => &[true],
            _ => &[false, true],
        };
        let mut made = Made::default();
        for along_v in ways {
            let iso = body
                .isocurve_through(face, at, *along_v)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Curve(iso),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extractisocurve",
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

    fn sheet(doc: &mut Document, y0: f64, y1: f64, z: f64) -> u64 {
        doc.add_body(
            Body::ruled(
                &line([0.0, y0, z], [4.0, y0, z]),
                &line([0.0, y1, z], [4.0, y1, z]),
                false,
            )
            .unwrap(),
        )
    }

    fn edge_along_y(doc: &Document, id: u64, y: f64) -> (u64, usize) {
        let body = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .shape
            .body()
            .unwrap()
            .clone();
        let edge = (0..body.record_edge_count())
            .find(|e| {
                body.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - y).abs() < 1e-9)
            })
            .unwrap();
        (id, edge)
    }

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    fn last_curve(doc: &Document) -> Curve {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.curve().cloned())
            .unwrap()
    }

    #[test]
    fn isocurves_shrink_and_split() {
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 0.0);
        doc.select(&[s]);
        run(&mut doc, &["extractisocurve", "U", "1,2"]);
        assert!((last_curve(&doc).length().unwrap() - 4.0).abs() < 1e-6);
        let mut doc = Document::default();
        let disc =
            doc.add_body(Body::planar(&[Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap()]).unwrap());
        doc.select(&[disc]);
        run(&mut doc, &["shrinktrimmedsrf"]);
        assert!((last_body(&doc).area().unwrap() - std::f64::consts::PI).abs() < 1e-3);
        let mut doc = Document::default();
        let s = sheet(&mut doc, 0.0, 4.0, 0.0);
        let cut = doc.add(line([2.0, -1.0, 0.0], [2.0, 5.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[s]);
        processor.submit(&mut doc, "splitsrf");
        doc.select(&[cut]);
        processor.submit(&mut doc, "");
        let pieces = doc
            .state()
            .objects
            .iter()
            .filter(|o| o.shape.body().is_some())
            .count();
        assert_eq!(pieces, 2);
    }

    #[test]
    fn blend_and_match_surfaces() {
        let mut doc = Document::default();
        let a = sheet(&mut doc, 0.0, 4.0, 0.0);
        let b = sheet(&mut doc, 7.0, 11.0, 3.0);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "blendsrf");
        doc.select_edges(&[edge_along_y(&doc, a, 4.0)]);
        processor.submit(&mut doc, "");
        doc.select_edges(&[edge_along_y(&doc, b, 7.0)]);
        processor.submit(&mut doc, "");
        assert_eq!(doc.state().objects.len(), 3, "{:?}", processor.log());
        let mut doc = Document::default();
        let a = sheet(&mut doc, 0.0, 4.0, 0.0);
        let b = sheet(&mut doc, 4.5, 8.5, 1.0);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "matchsrf");
        doc.select_edges(&[edge_along_y(&doc, a, 4.0)]);
        processor.submit(&mut doc, "");
        doc.select_edges(&[edge_along_y(&doc, b, 4.5)]);
        processor.submit(&mut doc, "");
        let changed = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((changed.1[1] - 4.5).abs() < 1e-6, "{changed:?}");
    }

    #[test]
    fn sweep_along_two_rails() {
        let mut doc = Document::default();
        let a = doc.add(line([0.0; 3], [0.0, 10.0, 0.0]));
        let b = doc.add(line([4.0, 0.0, 0.0], [8.0, 10.0, 0.0]));
        let p = doc.add(line([0.0; 3], [4.0, 0.0, 0.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "sweep2");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        doc.select(&[p]);
        processor.submit(&mut doc, "");
        // A flat quadrilateral: (4 + 8) / 2 × 10.
        assert!(
            (last_body(&doc).area().unwrap() - 60.0).abs() < 0.1,
            "{}",
            last_body(&doc).area().unwrap()
        );
    }
}
