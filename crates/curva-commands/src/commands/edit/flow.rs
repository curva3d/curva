//! The `Flow` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::support::*;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_kernel::{Point, Spline, cross, dot, sub, unit};

/// `Flow`, as in Rhino: moves objects from along one curve to along another, keeping where they sit
/// along and beside it. By default they keep their length along the curve (`Stretch` fits them to
/// the whole target) and the originals stay (`Copy`), as Rhino's defaults (#739).
const COPY: &str = "Copy";
const STRETCH: &str = "Stretch";

/// How far along `s` from its start each of `STEPS + 1` even parameter steps lies.
fn lengths(s: &Spline) -> Vec<(f64, f64)> {
    const STEPS: usize = 512;
    let (a, b) = s.domain();
    let mut out = vec![(a, 0.0)];
    let mut last = s.point(a);
    for k in 1..=STEPS {
        let t = a + (b - a) * k as f64 / STEPS as f64;
        let p = s.point(t);
        let run = out[k - 1].1 + curva_kernel::gap(p, last);
        out.push((t, run));
        last = p;
    }
    out
}

/// Read across a table of (parameter, length) pairs, from column `from` to the other, straight
/// between rows and held at the ends.
fn across(table: &[(f64, f64)], value: f64, from_length: bool) -> f64 {
    let key = |r: &(f64, f64)| if from_length { r.1 } else { r.0 };
    let give = |r: &(f64, f64)| if from_length { r.0 } else { r.1 };
    let k = table
        .partition_point(|r| key(r) < value)
        .clamp(1, table.len() - 1);
    let (lo, hi) = (&table[k - 1], &table[k]);
    let span = key(hi) - key(lo);
    let w = if span > 0.0 {
        ((value - key(lo)) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    give(lo) + (give(hi) - give(lo)) * w
}

#[derive(Default)]
pub struct Flow;

impl Recipe for Flow {
    const NAME: &'static str = "flow";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(COPY, true),
        OptionDef::toggle(STRETCH, false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.flow.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.flow.base").curves().one());
        }
        if cx.values.len() >= 2 {
            steps.push(StepDef::objects("prompt.flow.target").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some((_, base)), Some((_, target))) = (first_curve(cx, 1), first_curve(cx, 2)) else {
            return Ok(Made::default());
        };
        let (base, target) = (
            Spline::of(&base).map_err(|e| failed(&e))?,
            Spline::of(&target).map_err(|e| failed(&e))?,
        );
        let normal = up(cx);
        let frame = |s: &Spline, t: f64| -> (Point, Point, Point, Point) {
            let p = s.point(t);
            let tangent = unit(s.derivative(t)).unwrap_or([1.0, 0.0, 0.0]);
            let side = unit(cross(normal, tangent)).unwrap_or([0.0, 1.0, 0.0]);
            (p, tangent, side, cross(tangent, side))
        };
        let (base_run, target_run) = (lengths(&base), lengths(&target));
        let (base_length, target_length) = (
            base_run[base_run.len() - 1].1,
            target_run[target_run.len() - 1].1,
        );
        let stretch = cx.toggle(STRETCH);
        let map = move |p: Point| {
            let t = base.closest(p);
            let (bp, bt, bs, bu) = frame(&base, t);
            let d = sub(p, bp);
            let (along, beside, above) = (dot(d, bt), dot(d, bs), dot(d, bu));
            // As far along the target as along the base, or the same share of it with Stretch.
            let run = across(&base_run, t, false);
            let run = if stretch {
                run / base_length.max(1e-12) * target_length
            } else {
                run
            };
            let (tp, tt, ts, tu) = frame(&target, across(&target_run, run, true));
            plus(plus(plus(tp, tt, along), ts, beside), tu, above)
        };
        let mut made = deform_all(cx, objects(cx, 0), &map, "done.flow")?;
        // With Copy the originals stay and the flowed objects are added beside them.
        if cx.toggle(COPY) {
            made.derived = std::mem::take(&mut made.replace)
                .into_iter()
                .map(|(from, shape)| Derived {
                    shape,
                    from,
                    history: None,
                })
                .collect();
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn a_box_flows_onto_an_arc_keeping_its_length_and_the_original_as_in_rhino() {
        let mut doc = Document::default();
        let solid = doc.add_body(Body::cuboid([0.0; 3], [10.0, 1.0, 1.0]).unwrap());
        let base = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let target = doc.add(
            Curve::arc(
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .unwrap(),
        );
        let mut processor = crate::Processor::default();
        doc.select(&[solid]);
        processor.submit(&mut doc, "flow");
        for picked in [base, target] {
            doc.select(&[picked]);
            processor.submit(&mut doc, "");
        }
        let bodies: Vec<&Body> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .collect();
        // Rhino 8: the box stays and a copy covers 10 units of the arc, volume 9.4999, up to
        // y = 8.415.
        assert_eq!(bodies.len(), 2, "{:?}", processor.log());
        let flowed = bodies[1];
        // Deformed, not rebuilt as Rhino rebuilds it: 9.489, 0.1% under.
        assert!(
            (flowed.volume().unwrap() - 9.4999).abs() < 2e-2,
            "{}",
            flowed.volume().unwrap()
        );
        let (_, high) = flowed.bounds().unwrap();
        assert!((high[1] - 8.415).abs() < 1e-2, "{high:?}");
    }
}
