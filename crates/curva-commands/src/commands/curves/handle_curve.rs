//! The `HandleCurve` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::recipe::{Context, Made, Recipe, StepDef, run_points};
use curva_document::Shape;
use curva_kernel::{Curve, Point, Spline, sub};

/// `HandleCurve`: a curve by points with handles, as a pen tool draws: click a point, then where
/// its handle goes; the curve leaves each point towards its handle and arrives from the mirror of
/// it. Enter ends it.
#[derive(Default)]
pub struct HandleCurve;

impl Recipe for HandleCurve {
    const NAME: &'static str = "handlecurve";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        run_steps(
            cx,
            |i| {
                if i % 2 == 0 {
                    "prompt.handlecurve.point"
                } else {
                    "prompt.handlecurve.handle"
                }
            },
            |placed| placed >= 4 && placed % 2 == 0,
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let placed = run_points(cx);
        // Pairs of a point and its handle; a point still without one uses itself.
        let pairs: Vec<(Point, Point)> = placed
            .chunks(2)
            .map(|pair| (pair[0], pair.get(1).copied().unwrap_or(pair[0])))
            .collect();
        let guides: Vec<Curve> = pairs
            .iter()
            .filter_map(|(at, handle)| {
                let mirror = plus(*at, sub(*at, *handle), 1.0);
                Curve::line(mirror, *handle).ok()
            })
            .collect();
        if pairs.len() < 2 {
            return Ok(Made {
                guides,
                ..Made::default()
            });
        }
        let pieces: Vec<Spline> = pairs
            .windows(2)
            .map(|w| {
                let ((a, ha), (b, hb)) = (w[0], w[1]);
                let arrive = plus(b, sub(b, hb), 1.0);
                bezier(&[a, ha, arrive, b], None)
            })
            .collect();
        let curve = Spline::joined(&pieces)
            .and_then(|s| s.to_curve())
            .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            guides,
            message: Some(Message::new("done.handlecurve")),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Spline, gap};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    fn ends(curve: &Curve) -> (curva_kernel::Point, curva_kernel::Point) {
        let points = curve.sample().unwrap();
        (points[0], points[points.len() - 1])
    }

    #[test]
    fn handle_curve_leaves_each_point_towards_its_handle() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["handlecurve", "0,0", "0,5", "10,0", "10,-5", ""],
        );
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        let (a, b) = spline.domain();
        assert!(gap(spline.point(a), [0.0; 3]) < 1e-9);
        assert!(gap(spline.point(b), [10.0, 0.0, 0.0]) < 1e-9);
        // Leaving upward, towards the first handle.
        let d = spline.derivative(a);
        assert!(d[0].abs() < 1e-3 * d[1] && d[1] > 0.0, "{d:?}");
    }

    #[test]
    fn blends_meet_both_curves_smoothly() {
        for (mode, smooth) in [("Position", false), ("Tangency", true), ("Curvature", true)] {
            let mut doc = Document::default();
            let a = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
            let b = doc.add(Curve::line([10.0, 3.0, 0.0], [15.0, 3.0, 0.0]).unwrap());
            let mut processor = crate::Processor::default();
            processor.submit(&mut doc, "blendcrv");
            processor.submit(&mut doc, mode);
            doc.select(&[a]);
            processor.submit(&mut doc, "");
            doc.select(&[b]);
            processor.submit(&mut doc, "");
            let blend = doc.state().objects.last().unwrap().curve().unwrap().clone();
            let (start, end) = ends(&blend);
            assert!(gap(start, [5.0, 0.0, 0.0]) < 1e-6, "{mode}: {start:?}");
            assert!(gap(end, [10.0, 3.0, 0.0]) < 1e-6, "{mode}: {end:?}");
            if smooth {
                let spline = Spline::of(&blend).unwrap();
                let (a0, a1) = spline.domain();
                let (d0, d1) = (spline.derivative(a0), spline.derivative(a1));
                assert!(d0[1].abs() < 1e-3 * d0[0].abs(), "{mode}: {d0:?}");
                assert!(d1[1].abs() < 1e-3 * d1[0].abs(), "{mode}: {d1:?}");
            }
        }
    }

    #[test]
    fn smooth_and_arc_extensions_carry_the_curve_on() {
        for kind in ["Smooth", "Arc"] {
            let mut doc = Document::default();
            let arc = doc.add(
                Curve::arc(
                    [0.0; 3],
                    [5.0, 0.0, 0.0],
                    [0.0, 5.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .unwrap(),
            );
            let mut processor = crate::Processor::default();
            processor.submit(&mut doc, "extend");
            processor.submit(&mut doc, kind);
            processor.submit(&mut doc, "");
            doc.select(&[arc]);
            processor.submit(&mut doc, "");
            processor.submit(&mut doc, "2");
            let longer = only_curve(&doc);
            assert!(
                longer.length().unwrap() > 5.0 * std::f64::consts::FRAC_PI_2 + 1.5,
                "{kind}"
            );
            // An arc carried on as an arc stays on its circle.
            if kind == "Arc" {
                let (_, end) = ends(&longer);
                assert!((gap(end, [0.0; 3]) - 5.0).abs() < 1e-6, "{end:?}");
            }
        }
    }
}
