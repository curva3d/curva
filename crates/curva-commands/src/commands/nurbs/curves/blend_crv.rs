//! The `BlendCrv` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curve_at;
use crate::recipe::{Context, Made, ON_CURVE, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Spline, gap};

/// `BlendCrv`: a curve joining the ends of two curves picked near them (#728), meeting them in
/// position, along their tangents, or in curvature (the default), as Rhino's BlendCrv.
#[derive(Default)]
pub struct BlendCrv;

impl Recipe for BlendCrv {
    const NAME: &'static str = "blendcrv";
    const OPTIONS: &'static [OptionDef] = &CONTINUITY;

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::pick("prompt.blendcrv.first")];
        if !cx.values.is_empty() {
            steps.push(StepDef::pick("prompt.blendcrv.second"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(at_a), Some(at_b)) = (
            cx.values.first().and_then(Value::point),
            cx.values.get(1).and_then(Value::point),
        ) else {
            return Ok(Made::default());
        };
        let (Some((a_id, a)), Some((b_id, b))) = (
            curve_at(cx.doc, at_a, ON_CURVE).map_err(|e| failed(&e))?,
            curve_at(cx.doc, at_b, ON_CURVE).map_err(|e| failed(&e))?,
        ) else {
            return Ok(Made::default());
        };
        if a_id == b_id {
            return Ok(Made::default());
        }
        let (a, b) = (&a, &b);
        let (sa, sb) = (
            Spline::of(a).map_err(|e| failed(&e))?,
            Spline::of(b).map_err(|e| failed(&e))?,
        );
        // Blended from the end of each curve nearer where it was picked, as Rhino does (#728).
        let (a_end, b_end) = (picked_end(&sa, at_a), picked_end(&sb, at_b));
        let (p0, _, _) = leaving(&sa, a_end);
        let (p3, _, _) = leaving(&sb, b_end);
        if gap(p0, p3) <= 1e-9 {
            return Err(Message::new("error.blend_touching"));
        }
        // The engine's blend: the least-degree Bézier meeting each end as asked.
        let contact = match continuity(cx, Continuity::Curvature) {
            Continuity::Position => curva_kernel::Contact::Position,
            Continuity::Tangency => curva_kernel::Contact::Tangency,
            Continuity::Curvature => curva_kernel::Contact::Curvature,
        };
        let curve = a
            .blended_with(a_end, contact, b, b_end, contact, 1.0 / 3.0)
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.blendcrv")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    #[test]
    fn the_blend_starts_from_the_end_picked_as_in_rhino() {
        // Rhino 8 (#728): picking the first line near (0,0) blends from (0,0) round to the
        // second line's start, not from the nearer end (5,0).
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        doc.add(Curve::line([8.0, 3.0, 0.0], [8.0, 8.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "blendcrv");
        processor.click(&mut doc, [0.5, 0.0, 0.0]);
        processor.click(&mut doc, [8.0, 3.5, 0.0]);
        let blend = doc.state().objects.last().unwrap().curve().unwrap().clone();
        let points = blend.sample().unwrap();
        let (start, end) = (points[0], points[points.len() - 1]);
        let near = |p: [f64; 3], q: [f64; 3]| gap(p, q) < 1e-6;
        assert!(
            (near(start, [0.0; 3]) && near(end, [8.0, 3.0, 0.0]))
                || (near(end, [0.0; 3]) && near(start, [8.0, 3.0, 0.0])),
            "{start:?} {end:?}"
        );
    }
}
