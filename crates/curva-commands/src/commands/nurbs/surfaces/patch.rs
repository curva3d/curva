//! The `Patch` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve};

const SPANS: &str = "UVSpans";
const STIFFNESS: &str = "Stiffness";

/// `Patch`: a surface filling a closed chain of curves. Three or four sides make one surface
/// through them; any other boundary one smooth surface fitted over it and trimmed back to it, as
/// Rhino makes it, with `UVSpans` spans each way (10) and `Stiffness` (1) (#684).
#[derive(Default)]
pub struct Patch;

impl Recipe for Patch {
    const NAME: &'static str = "patch";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SPANS, 10, 1, 100),
        OptionDef::number(STIFFNESS, 1.0),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.patch.curves")
                .curves()
                .offering(&[SPANS, STIFFNESS]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        let boundary = curves.clone();
        // One closed curve is filled through its pieces.
        if curves.len() == 1
            && let Some(pieces) = curves[0].exploded()
        {
            curves = pieces;
        }
        // Three or four sides make one surface through them; any other boundary one fitted
        // surface trimmed to it, as Rhino's Patch makes it (#684). The pieces are the fallback.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let spans = cx.number(SPANS).round().max(1.0) as usize;
        let one = (3..=4)
            .contains(&curves.len())
            .then(|| Body::from_edges(&curves).ok())
            .flatten()
            .or_else(|| Body::fitted_patch(&boundary, spans, cx.number(STIFFNESS)).ok());
        let (body, off) = match one {
            Some(body) => (body, 0.0),
            None => Body::patched(&curves).map_err(|e| failed(&e))?,
        };
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::with("done.patch", vec![format!("{off:.4}")])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn four_sides_make_one_surface_as_in_rhino() {
        let mut doc = Document::default();
        let corners = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 3.0],
            [0.0, 10.0, 0.0],
        ];
        let id = doc.add(Curve::polyline(corners, true).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["patch"]);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert_eq!(body.face_count().unwrap(), 1);
        // Rhino's fitted patch is 102.9385; a surface through the four sides is 102.9399.
        assert!((body.area().unwrap() - 102.9399).abs() < 1e-3);
    }

    #[test]
    fn a_six_sided_boundary_is_one_trimmed_surface_as_in_rhino() {
        let mut doc = Document::default();
        let corners: Vec<[f64; 3]> = (0..6)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 6.0;
                [
                    10.0 * a.cos(),
                    10.0 * a.sin(),
                    if k == 0 { 3.0 } else { 0.0 },
                ]
            })
            .collect();
        let id = doc.add(Curve::polyline(corners, true).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["patch"]);
        // Rhino's Patch: one surface over the boundary, trimmed to it (#684).
        let patch = doc.state().objects.last().unwrap().shape.body().unwrap();
        assert_eq!(patch.face_count().unwrap(), 1);
    }
}
