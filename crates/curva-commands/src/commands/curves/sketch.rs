//! The `Sketch` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, run_points};
use curva_document::Shape;
use curva_kernel::{Curve, gap};

/// `Sketch`: a freehand curve. Hold the mouse button and draw (the window feeds the points as
/// the pointer moves), or click points; Enter or letting go ends it. The points are thinned and
/// a smooth curve is passed through them.
#[derive(Default)]
pub struct Sketch;

impl Recipe for Sketch {
    const NAME: &'static str = "sketch";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        run_steps(cx, |_| "prompt.sketch.point", |placed| placed >= 2)
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let placed = run_points(cx);
        if placed.len() < 2 {
            return Ok(Made::default());
        }
        let total: f64 = placed.windows(2).map(|w| gap(w[0], w[1])).sum();
        // Keep a point every fiftieth of the length, and the last.
        let step = total / 50.0;
        let mut kept = vec![placed[0]];
        for p in &placed[1..] {
            if gap(*p, *kept.last().unwrap_or(&placed[0])) >= step {
                kept.push(*p);
            }
        }
        let last = placed[placed.len() - 1];
        if gap(last, *kept.last().unwrap_or(&last)) > 1e-9 {
            kept.push(last);
        }
        let curve = if kept.len() == 2 {
            Curve::line(kept[0], kept[1])
        } else {
            Curve::interpolated(kept, false)
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.sketch")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    fn ends(curve: &Curve) -> (curva_kernel::Point, curva_kernel::Point) {
        let points = curve.sample().unwrap();
        (points[0], points[points.len() - 1])
    }

    #[test]
    fn sketch_passes_through_the_points_drawn() {
        let mut doc = Document::default();
        run(&mut doc, &["sketch", "0,0", "1,1", "2,0", "3,1", "4,0", ""]);
        let curve = only_curve(&doc);
        let (start, end) = ends(&curve);
        assert!(gap(start, [0.0; 3]) < 1e-9 && gap(end, [4.0, 0.0, 0.0]) < 1e-9);
    }
}
