//! Rhino's `SubDLoft` for curves. Measured in Rhino 8 (23 November 2026, curves picked by id):
//! it asks "Select curves and boundary edge chains in loft order", then "Adjust edge loop
//! position. Press Enter when done ( Flip  Automatic  Natural  SnapToKnots=Yes )" and "Lofting
//! options ( ClosedLoft=No  AdjustSegmentCounts=No  DivisionsAlong=1 )"; three circles give a SubD
//! of 24 points, 40 edges and 16 faces, its end rings creased, and the curves stay. See
//! `curva_mesh::loft_ring` for how each curve becomes a ring. Boundary edge chains, open smooth
//! curves, the edge loop step and AdjustSegmentCounts are not here yet.

use crate::Message;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_mesh::{SubD, loft_ring};

const CLOSED_LOFT: &str = "ClosedLoft";
const DIVISIONS: &str = "DivisionsAlong";

/// The recipe.
#[derive(Default)]
pub struct SubDLoft;

impl Recipe for SubDLoft {
    const NAME: &'static str = "subdloft";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(CLOSED_LOFT, false),
        OptionDef::whole(DIVISIONS, 1, 1, 100),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.subdloft.curves")
                .curves()
                .at_least(2)
                .offering(&[CLOSED_LOFT, DIVISIONS]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(chosen)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let curves = curves_of(cx.doc, chosen);
        if curves.len() < 2 {
            return Err(Message::new("error.subdloft_two"));
        }
        let mut rings = Vec::new();
        let mut closed = Vec::new();
        for (_, curve) in &curves {
            let (ring, is_closed) =
                loft_ring(curve).map_err(|e| Message::with("error.subdloft_curve", vec![e.0]))?;
            rings.push(ring);
            closed.push(is_closed);
        }
        if closed.iter().any(|&c| c != closed[0]) {
            return Err(Message::new("error.subdloft_mixed"));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let divisions = cx.number(DIVISIONS) as usize;
        let subd = SubD::lofted(&rings, closed[0], cx.toggle(CLOSED_LOFT), divisions)
            .map_err(|e| Message::with("error.subdloft_curve", vec![e.0]))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::SubD(subd),
                from: curves[0].0,
                history: None,
            }],
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    fn circles(doc: &mut Document) {
        for (z, radius) in [(0.0, 2.0), (2.0, 1.0), (4.0, 2.0)] {
            doc.add_shape(Shape::Curve(Curve::Circle {
                centre: [0.0, 0.0, z],
                radius,
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
            }));
        }
    }

    #[test]
    fn three_circles_loft_as_in_rhino() {
        let mut doc = Document::default();
        circles(&mut doc);
        let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&ids);
        run(&mut doc, &["subdloft"]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 4, "the curves stay");
        let subd = objects[3].shape.subd().unwrap();
        let cage = subd.cage();
        assert_eq!(
            (cage.vertex_count(), cage.edge_count(), cage.face_count()),
            (24, 40, 16)
        );
    }
}
