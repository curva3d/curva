//! The `EdgeSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

/// `EdgeSrf`: a surface bounded by two, three or four curves meeting end to end.
#[derive(Default)]
pub struct EdgeSrf;

impl Recipe for EdgeSrf {
    const NAME: &'static str = "edgesrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.edgesrf.curves")
                .curves()
                .at_least(2),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let body = match curves.len() {
            0 | 1 => return Ok(Made::default()),
            2 => Body::ruled(&curves[0], &curves[1], false),
            3 | 4 => Body::from_edges(&curves),
            _ => return Err(Message::new("error.edgesrf_count")),
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.edgesrf")),
            ..Made::default()
        })
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
    fn edge_network_and_patch_surfaces() {
        let mut doc = Document::default();
        let ids: Vec<u64> = [
            line([0.0; 3], [3.0, 0.0, 0.0]),
            line([3.0, 0.0, 0.0], [3.0, 3.0, 0.0]),
            line([3.0, 3.0, 0.0], [0.0, 3.0, 0.0]),
            line([0.0, 3.0, 0.0], [0.0; 3]),
        ]
        .into_iter()
        .map(|c| doc.add(c))
        .collect();
        doc.select(&ids);
        run(&mut doc, &["edgesrf"]);
        assert!((last_body(&doc).area().unwrap() - 9.0).abs() < 1e-6);
        doc.select(&ids);
        run(&mut doc, &["patch"]);
        assert!((last_body(&doc).area().unwrap() - 9.0).abs() < 1e-2);
        let mut doc = Document::default();
        let wave = |y: f64| {
            Curve::control_point(vec![[0.0, y, 0.0], [2.0, y, 1.0], [4.0, y, 0.0]], 2, false)
                .unwrap()
        };
        let across = |x: f64| {
            let z = if (x - 2.0).abs() < 1e-9 { 0.5 } else { 0.0 };
            Curve::control_point(vec![[x, 0.0, z], [x, 1.5, z], [x, 3.0, z]], 2, false).unwrap()
        };
        let ids: Vec<u64> = [across(2.0), wave(0.0), across(0.0), wave(3.0), across(4.0)]
            .into_iter()
            .map(|c| doc.add(c))
            .collect();
        doc.select(&ids);
        run(&mut doc, &["networksrf"]);
        assert!(last_body(&doc).area().unwrap() > 12.0);
    }
}
