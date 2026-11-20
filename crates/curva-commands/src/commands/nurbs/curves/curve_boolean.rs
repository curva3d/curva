//! The `CurveBoolean` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, dot, sub};

/// A point in the plane's own coordinates.
fn flat(p: Point, origin: Point, x: Point, y: Point) -> [f64; 2] {
    let d = sub(p, origin);
    [dot(d, x), dot(d, y)]
}

fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut crossed = false;
    for (index, a) in ring.iter().enumerate() {
        let b = ring[(index + 1) % ring.len()];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                crossed = !crossed;
            }
        }
    }
    crossed
}

/// The outlines of the regions of closed flat `curves` that hold the `picks`, on the plane of
/// directions `[x, y]`: all picked regions together when `combine`, else one list per pick.
/// Each list is closed curves, an outer outline and its holes; for `CurveBoolean` and `Hatch`'s
/// `Boundary`.
///
/// # Errors
///
/// When no pick is inside any curve, or the outlines cannot be joined.
// CUSTOM KERNEL OPERATION: curve-boolean-regions
pub(crate) fn regions(
    curves: &[Curve],
    picks: &[Point],
    [x, y]: [Point; 2],
    combine: bool,
) -> Result<Vec<Vec<Curve>>, Message> {
    let Some(&origin) = picks.first() else {
        return Ok(Vec::new());
    };
    let rings: Vec<Vec<[f64; 2]>> = curves
        .iter()
        .map(|c| {
            c.sample()
                .map(|points| points.iter().map(|p| flat(*p, origin, x, y)).collect())
                .unwrap_or_default()
        })
        .collect();
    let signature = |p: [f64; 2]| -> Vec<bool> { rings.iter().map(|r| inside(p, r)).collect() };
    let wanted: Vec<Vec<bool>> = picks
        .iter()
        .map(|p| signature(flat(*p, origin, x, y)))
        .filter(|s| s.iter().any(|inside| *inside))
        .collect();
    if wanted.is_empty() {
        return Err(Message::new("error.curveboolean_outside"));
    }
    // Every curve cut where the others cross it.
    let mut pieces = Vec::new();
    for (i, curve) in curves.iter().enumerate() {
        let mut cuts = Vec::new();
        for (j, other) in curves.iter().enumerate() {
            if i != j {
                cuts.extend(curva_kernel::crossings(curve, other).unwrap_or_default());
            }
        }
        pieces.extend(curve.cut_at(&cuts).unwrap_or_else(|_| vec![curve.clone()]));
    }
    let size = rings
        .iter()
        .flatten()
        .fold(0.0_f64, |m, p| m.max(p[0].abs()).max(p[1].abs()))
        .max(1.0);
    let nudge = size * 1e-5;
    let groups: Vec<Vec<Vec<bool>>> = if combine {
        vec![wanted.clone()]
    } else {
        wanted.iter().map(|w| vec![w.clone()]).collect()
    };
    let mut found = Vec::new();
    for region in groups {
        let held = |p: [f64; 2]| region.contains(&signature(p));
        let edges: Vec<Curve> = pieces
            .iter()
            .filter(|piece| {
                let Ok(points) = piece.sample() else {
                    return false;
                };
                let middle = points.len() / 2;
                let (a, b) = (
                    points[middle.saturating_sub(1)],
                    points[(middle + 1).min(points.len() - 1)],
                );
                let m = points[middle];
                let along = flat(sub(b, a), [0.0; 3], x, y);
                let n = (along[0].hypot(along[1])).max(1e-12);
                let side = [-along[1] / n * nudge, along[0] / n * nudge];
                let at = flat(m, origin, x, y);
                held([at[0] + side[0], at[1] + side[1]]) != held([at[0] - side[0], at[1] - side[1]])
            })
            .cloned()
            .collect();
        found.push(curva_kernel::join_all(&edges).map_err(|e| failed(&e))?);
    }
    Ok(found)
}

/// `CurveBoolean`: closed flat curves that overlap, then clicks inside the regions to keep; Enter
/// makes their outlines. `CombineRegions` (the default) outlines the regions picked together; off,
/// each separately. `DeleteInput` takes the curves away.
#[derive(Default)]
pub struct CurveBoolean;

impl Recipe for CurveBoolean {
    const NAME: &'static str = "curveboolean";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle("CombineRegions", true),
        OptionDef::toggle("DeleteInput", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.curveboolean.curves").curves()];
        if cx.values.is_empty() {
            return steps;
        }
        // Then a run of clicks inside regions, which Enter ends once there is one.
        let clicks = &cx.values[1..];
        for (i, _) in clicks.iter().enumerate() {
            let step = StepDef::point("prompt.curveboolean.inside");
            steps.push(if i > 0 { step.optional() } else { step });
        }
        if clicks.last() != Some(&Value::Empty) {
            let step = StepDef::point("prompt.curveboolean.inside");
            steps.push(if clicks.is_empty() {
                step
            } else {
                step.optional()
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<(u64, Curve)> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .filter(|(_, c)| c.is_closed().unwrap_or(false))
            .collect();
        let picks: Vec<Point> = cx.values[1..].iter().filter_map(Value::point).collect();
        if curves.is_empty() || picks.is_empty() {
            return Ok(Made::default());
        }
        let only: Vec<Curve> = curves.iter().map(|(_, c)| c.clone()).collect();
        let found = regions(&only, &picks, cx.plane(), cx.toggle("CombineRegions"))?;
        let mut made = Made::default();
        for outline in found.into_iter().flatten() {
            made.add.push(Shape::Curve(outline));
        }
        if cx.toggle("DeleteInput") {
            made.remove = curves.iter().map(|(id, _)| *id).collect();
        }
        made.message = Some(Message::with(
            "done.curveboolean",
            vec![made.add.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn curve_boolean_outlines_the_regions_picked() {
        let mut doc = Document::default();
        let a = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        let b = doc.add(
            Curve::polyline(
                vec![
                    [2.0, 2.0, 0.0],
                    [6.0, 2.0, 0.0],
                    [6.0, 6.0, 0.0],
                    [2.0, 6.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        // The overlap only: a 2 × 2 square.
        run(&mut doc, &["curveboolean", "3,3", ""]);
        let outline = only_curve(&doc);
        assert!(outline.is_closed().unwrap());
        assert!(
            (outline.length().unwrap() - 8.0).abs() < 1e-6,
            "{}",
            outline.length().unwrap()
        );
        // Both squares together: the union's outline, 24 long.
        let mut doc = Document::default();
        let a = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        let b = doc.add(
            Curve::polyline(
                vec![
                    [2.0, 2.0, 0.0],
                    [6.0, 2.0, 0.0],
                    [6.0, 6.0, 0.0],
                    [2.0, 6.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        run(&mut doc, &["curveboolean", "1,1", "3,3", "5,5", ""]);
        let union = only_curve(&doc);
        assert!(
            (union.length().unwrap() - 24.0).abs() < 1e-6,
            "{}",
            union.length().unwrap()
        );
    }
}
