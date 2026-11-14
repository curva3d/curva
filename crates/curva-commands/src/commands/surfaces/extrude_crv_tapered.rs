//! The `ExtrudeCrvTapered` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Point, TaperCorners};

/// The first point of the first curve chosen, where distances are measured from.
fn first_point(cx: &Context<'_>, index: usize) -> Option<Point> {
    curves_of(cx.doc, objects(cx, index))
        .first()
        .and_then(|(_, c)| c.sample().ok())
        .and_then(|p| p.first().copied())
}

/// `ExtrudeCrvTapered`: pulls closed flat curves out of the plane with their walls leaning in
/// by `DraftAngle` degrees (out when negative). `Corners` closes the corners `Sharp` (the
/// default), `Round` or `Smooth`; `Solid` caps the ends.
#[derive(Default)]
pub struct ExtrudeCrvTapered;

impl Recipe for ExtrudeCrvTapered {
    const NAME: &'static str = "extrudecrvtapered";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("DraftAngle", 5.0),
        OptionDef::toggle("Solid", true),
        OptionDef::mode("Sharp"),
        OptionDef::mode("Round"),
        OptionDef::mode("Smooth"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvtapered.objects")
                .curves()
                .offering(&[]),
        ];
        if let Some(base) = first_point(cx, 0) {
            steps.push(
                StepDef::along("prompt.extrudecrv.distance", base, up(cx))
                    .remembered("Distance", 10.0),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(distance) = number(cx, 1).filter(|d| d.abs() > 1e-9) else {
            return Ok(Made::default());
        };
        let corners = match cx.mode {
            Some("Round") => TaperCorners::Round,
            Some("Smooth") => TaperCorners::Smooth,
            _ => TaperCorners::Sharp,
        };
        let draft = cx.number("DraftAngle").to_radians();
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            if !curve.is_closed().unwrap_or(false) {
                return Err(Message::new("error.tapered_closed"));
            }
            let body = Body::extruded_tapered(
                &curve,
                up(cx),
                distance,
                draft,
                corners,
                cx.toggle("Solid"),
            )
            .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;

    use curva_document::Document;
    use curva_kernel::Curve;

    fn square(doc: &mut Document, size: f64) -> u64 {
        doc.add(
            Curve::polyline(
                vec![
                    [0.0; 3],
                    [size, 0.0, 0.0],
                    [size, size, 0.0],
                    [0.0, size, 0.0],
                ],
                true,
            )
            .unwrap(),
        )
    }

    fn last_volume(doc: &Document) -> f64 {
        doc.state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn tapered_extrusions_lean_in() {
        let mut doc = Document::default();
        let id = square(&mut doc, 10.0);
        doc.select(&[id]);
        run(&mut doc, &["extrudecrvtapered", "DraftAngle", "45", "2"]);
        let expected = 2.0 / 3.0 * (100.0 + 36.0 + 60.0);
        assert!(
            (last_volume(&doc) - expected).abs() < 1e-3,
            "{}",
            last_volume(&doc)
        );
        // Open curves are refused.
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        run(&mut doc, &["extrudecrvtapered", "2"]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
