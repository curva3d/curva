//! The `Taper` command, as in Rhino: the axis, then the distances from it at its start and end, the
//! objects scaled from one to the other along it (#713).

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_kernel::Point;

/// `Taper`: narrows or widens objects along an axis: the axis, then how much bigger they are at
/// its end (1 keeps them).
#[derive(Default)]
pub struct Taper;

impl Recipe for Taper {
    const NAME: &'static str = "taper";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        // The size at the axis's start and end, as distances from it, as Rhino asks (#713).
        let start = point(cx, 1).map_or_else(
            || StepDef::number("prompt.taper.start"),
            |a| StepDef::number("prompt.taper.start").from(a),
        );
        let mut steps = axis_steps(cx, "prompt.taper.objects", Some(start));
        if cx.values.len() >= 4 {
            let end = StepDef::number("prompt.taper.end");
            steps.push(match point(cx, 2) {
                Some(b) => end.from(b),
                None => end,
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(start), Some(end)) =
            (point(cx, 1), point(cx, 2), number(cx, 3), number(cx, 4))
        else {
            return Ok(Made::default());
        };
        if start <= 0.0 || end <= 0.0 {
            return Err(Message::new("error.taper_factor"));
        }
        // Each point's distance from the axis scaled from the start distance to the end one.
        let map = move |p: Point| {
            let (t, foot, r) = along_axis(p, a, b);
            plus(foot, r, (start + (end - start) * t) / start)
        };
        deform_all(cx, objects(cx, 0), &map, "done.taper")
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn a_box_tapers_about_one_of_its_edges_as_in_rhino() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        doc.select(&[cube]);
        let processor = run(&mut doc, &["taper", "0,0,0", "0,0,10", "5", "2.5"]);
        let tapered = doc.state().objects[0].shape.body().unwrap();
        assert!(tapered.is_solid(), "{:?}", processor.log());
        // Half size at the top, about the edge on the axis: the top is 5 × 5 at the corner.
        let (_, high) = tapered.bounds().unwrap();
        assert!(
            (high[2] - 10.0).abs() < 1e-6 && high[0] <= 10.0 + 1e-6,
            "{high:?}"
        );
    }
}
