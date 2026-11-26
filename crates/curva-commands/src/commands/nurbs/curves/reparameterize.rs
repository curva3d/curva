//! The `Reparameterize` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::{bodies_of, curves_of};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::Spline;

/// `Reparameterize`, as in Rhino: gives curves a new parameter range, start and end, offering the
/// first curve's own range; `Automatic` gives each curve the range of its length (#741).
/// Surfaces chosen alone get a new box, U start and end then V start and end, offering the first
/// surface's own; `Automatic` gives each the lengths of its edges.
const AUTOMATIC: &str = "Automatic";

/// The first chosen curve's own parameter range, offered as the default.
fn current_range(cx: &Context<'_>) -> (f64, f64) {
    curves_of(cx.doc, objects(cx, 0))
        .first()
        .and_then(|(_, c)| Spline::of(c).ok())
        .map_or((0.0, 1.0), |s| s.domain())
}

/// Whether surfaces were chosen and no curves: then the steps ask for a surface's box.
fn for_surfaces(cx: &Context<'_>) -> bool {
    curves_of(cx.doc, objects(cx, 0)).is_empty() && !bodies_of(cx.doc, objects(cx, 0)).is_empty()
}

/// The first chosen surface's own box, offered as the default.
fn current_box(cx: &Context<'_>) -> [[f64; 2]; 2] {
    bodies_of(cx.doc, objects(cx, 0))
        .first()
        .and_then(|(_, b)| b.surface_domain().ok())
        .unwrap_or([[0.0, 1.0], [0.0, 1.0]])
}

#[derive(Default)]
pub struct Reparameterize;

impl Recipe for Reparameterize {
    const NAME: &'static str = "reparameterize";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::finish(AUTOMATIC)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.reparameterize.objects")];
        if cx.values.is_empty() || cx.mode == Some(AUTOMATIC) {
            return steps;
        }
        let asks: Vec<(&'static str, &'static str, f64)> = if for_surfaces(cx) {
            let [[u0, u1], [v0, v1]] = current_box(cx);
            vec![
                ("prompt.reparameterize.u_start", "UStart", u0),
                ("prompt.reparameterize.u_end", "UEnd", u1),
                ("prompt.reparameterize.v_start", "VStart", v0),
                ("prompt.reparameterize.v_end", "VEnd", v1),
            ]
        } else {
            let (start, end) = current_range(cx);
            vec![
                ("prompt.reparameterize.start", "Start", start),
                ("prompt.reparameterize.end", "End", end),
            ]
        };
        for (i, (prompt, key, value)) in asks.into_iter().enumerate() {
            if cx.values.len() > i {
                let step = StepDef::number(prompt).remembered(key, value);
                steps.push(if i == 0 {
                    step.offering(&[AUTOMATIC])
                } else {
                    step
                });
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        if for_surfaces(cx) {
            return surfaces_made(cx);
        }
        let automatic = cx.mode == Some(AUTOMATIC);
        let range = (number(cx, 1), number(cx, 2));
        if !automatic && (range.0.is_none() || range.1.is_none()) {
            return Ok(Made::default());
        }
        if let (Some(start), Some(end)) = range
            && end <= start
        {
            return Err(Message::new("error.reparameterize_order"));
        }
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let spline = Spline::of(&curve).map_err(|e| failed(&e))?;
            // Automatic: each curve's range runs from 0 to its length, as in Rhino.
            let (start, end) = if automatic {
                (0.0, curve.length().map_err(|e| failed(&e))?)
            } else {
                (range.0.unwrap_or(0.0), range.1.unwrap_or(1.0))
            };
            let curve = spline
                .reparameterized(start, end)
                .to_curve()
                .map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Curve(curve)));
        }
        made.message = Some(Message::with(
            "done.reparameterize",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

/// The chosen surfaces with their new boxes: the one typed, or with `Automatic` each one's edge
/// lengths from 0, as in Rhino.
fn surfaces_made(cx: &Context<'_>) -> Result<Made, Message> {
    let automatic = cx.mode == Some(AUTOMATIC);
    let typed = (1..=4).map(|i| number(cx, i)).collect::<Option<Vec<f64>>>();
    let typed = match (automatic, typed) {
        (true, _) => None,
        (false, Some(t)) => Some([[t[0], t[1]], [t[2], t[3]]]),
        (false, None) => return Ok(Made::default()),
    };
    if typed.is_some_and(|[u, v]| u[1] <= u[0] || v[1] <= v[0]) {
        return Err(Message::new("error.reparameterize_order"));
    }
    let mut made = Made::default();
    for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
        let domain = match typed {
            Some(domain) => domain,
            None => {
                let [lu, lv] = body.surface_edge_lengths().map_err(|e| failed(&e))?;
                [[0.0, lu], [0.0, lv]]
            }
        };
        let changed = body
            .surface_reparameterized(domain)
            .map_err(|e| failed(&e))?;
        made.replace.push((id, Shape::Body(changed)));
    }
    made.message = Some(Message::with(
        "done.reparameterize",
        vec![made.replace.len().to_string()],
    ));
    Ok(made)
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Spline};

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn automatic_gives_the_curve_the_range_of_its_length_and_enter_keeps_the_range() {
        let mut doc = Document::default();
        let points = vec![[0.0; 3], [1.0, 2.0, 0.0], [3.0, 2.0, 0.0], [4.0, 0.0, 0.0]];
        let id = doc.add(Curve::control_point(points, 3, false).unwrap());
        let before = Spline::of(&only_curve(&doc)).unwrap().domain();
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "", ""]);
        let kept = Spline::of(&only_curve(&doc)).unwrap().domain();
        assert!((kept.0 - before.0).abs() < 1e-12 && (kept.1 - before.1).abs() < 1e-12);
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "automatic"]);
        let curve = only_curve(&doc);
        let (start, end) = Spline::of(&curve).unwrap().domain();
        assert!(
            start.abs() < 1e-12 && (end - curve.length().unwrap()).abs() < 1e-9,
            "{start} {end}"
        );
    }

    #[test]
    fn reparameterize_and_make_uniform() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(
                vec![
                    [0.0; 3],
                    [1.0, 2.0, 0.0],
                    [3.0, 2.0, 0.0],
                    [4.0, 0.0, 0.0],
                    [6.0, 1.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "10", "20"]);
        assert_eq!(
            Spline::of(&only_curve(&doc)).unwrap().domain(),
            (10.0, 20.0)
        );
        run(&mut doc, &["makeuniform"]);
        let spline = Spline::of(&only_curve(&doc)).unwrap();
        let inner = &spline.knots[4..spline.knots.len() - 4];
        assert!((inner[0] - 15.0).abs() < 1e-9, "{:?}", spline.knots);
    }

    #[test]
    fn a_surface_gets_the_box_typed_or_its_edge_lengths_as_in_rhino() {
        let mut doc = Document::default();
        // A curved sheet (an arch ruled to a line): a plane keeps its box of distances in Curva.
        let bow = Curve::control_point(vec![[0.0; 3], [5.0, 0.0, 3.0], [10.0, 0.0, 0.0]], 2, false)
            .unwrap();
        let line = Curve::line([0.0, 4.0, 0.0], [10.0, 4.0, 0.0]).unwrap();
        let id = doc.add_body(curva_kernel::Body::ruled(&bow, &line, false).unwrap());
        doc.select(&[id]);
        // Rhino 8: U start, U end, V start, V end.
        run(&mut doc, &["reparameterize", "2", "5", "7", "9"]);
        let body = doc.state().objects[0].shape.body().unwrap();
        assert_eq!(body.surface_domain().unwrap(), [[2.0, 5.0], [7.0, 9.0]]);
        doc.select(&[id]);
        run(&mut doc, &["reparameterize", "automatic"]);
        let body = doc.state().objects[0].shape.body().unwrap();
        let [[u0, u1], [v0, v1]] = body.surface_domain().unwrap();
        let mut sizes = [u1 - u0, v1 - v0];
        sizes.sort_by(f64::total_cmp);
        // Each way from 0 to its edge's length: 4 across, the arch's length (over 10) along.
        assert!(u0 == 0.0 && v0 == 0.0 && (sizes[0] - 4.0).abs() < 1e-9 && sizes[1] > 10.0);
    }
}
