//! The `RailRevolve` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Point, gap, sub};

/// `RailRevolve`: turns a profile curve a full turn about an axis, stretched away from the axis
/// so it follows a rail curve round it.
#[derive(Default)]
pub struct RailRevolve;

/// Angles at which the profile is placed exactly; the surface is skinned between them.
const RAIL_STATIONS: usize = 72;

impl Recipe for RailRevolve {
    const NAME: &'static str = "railrevolve";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.railrevolve.profile")
                .curves()
                .one(),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.railrevolve.rail").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::point("prompt.railrevolve.axis_start"));
        }
        if let Some(a) = point(cx, 2) {
            steps.push(StepDef::point("prompt.railrevolve.axis_end").from(a));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (profiles, rails) = (
            curves_of(cx.doc, objects(cx, 0)),
            curves_of(cx.doc, objects(cx, 1)),
        );
        let (Some((id, profile)), Some((_, rail)), Some(a), Some(b)) =
            (profiles.first(), rails.first(), point(cx, 2), point(cx, 3))
        else {
            return Ok(Made::default());
        };
        // Where the profile touches the rail: its point nearest it.
        let touch = profile
            .divided(200)
            .map_err(|e| failed(&e))?
            .into_iter()
            .min_by(|p, q| {
                let d = |x: &Point| rail.closest_point(*x).map_or(f64::INFINITY, |r| gap(*x, r));
                d(p).total_cmp(&d(q))
            })
            .ok_or_else(|| Message::new("error.reference_point"))?;
        let body = Body::rail_revolved(profile, rail, a, sub(b, a), touch, RAIL_STATIONS)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            derived: vec![Derived {
                shape: Shape::Body(body),
                from: *id,
                history: None,
            }],
            message: Some(Message::new("done.railrevolve")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {

    use curva_document::Document;
    use curva_kernel::Curve;

    fn rail_length(doc: &Document, id: u64) -> f64 {
        doc.state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .curve()
            .unwrap()
            .length()
            .unwrap()
    }

    #[test]
    fn rail_revolve_follows_the_rail() {
        // A vertical profile 3 out from the axis, following an ellipse rail 3 by 5: the side
        // wall's area is the ellipse's perimeter times 2.
        let mut doc = Document::default();
        let profile = doc.add(Curve::line([3.0, 0.0, 0.0], [3.0, 0.0, 2.0]).unwrap());
        let rail = doc.add(Curve::ellipse([0.0; 3], [3.0, 0.0, 0.0], [0.0, 5.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[profile]);
        processor.submit(&mut doc, "railrevolve");
        doc.select(&[rail]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "0,0,0");
        processor.submit(&mut doc, "0,0,1");
        let wall = doc
            .state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .area()
            .unwrap();
        let perimeter = rail_length(&doc, rail);
        assert!(
            (wall - 2.0 * perimeter).abs() < 0.01 * wall,
            "{wall} against {}",
            2.0 * perimeter
        );
    }
}
