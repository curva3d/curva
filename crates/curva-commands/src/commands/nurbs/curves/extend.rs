//! The `Extend` command.

use super::support::*;
use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, KernelError, Spline, cross, gap, sub, unit};

/// How Extend carries a curve on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Line,
    Arc,
    Smooth,
}

/// The curve carried on past its end by about `by` (a length), and the added piece alone.
fn extended(curve: &Curve, by: f64, reach: Reach) -> Result<(Curve, Curve), KernelError> {
    if reach != Reach::Arc {
        // The engine's extension: straight along the end tangent, or the curve's own end span
        // carried on, joined into one curve.
        let how = if reach == Reach::Line {
            curva_kernel::Carry::Tangent
        } else {
            curva_kernel::Carry::Natural
        };
        let whole = curve.extended_by(false, by, how)?;
        let end = curve
            .sample()?
            .last()
            .copied()
            .ok_or_else(|| KernelError("an empty curve".to_owned()))?;
        let piece = whole
            .cut_at(&[end])?
            .pop()
            .ok_or_else(|| KernelError("could not extend the curve".to_owned()))?;
        return Ok((whole, piece));
    }
    // CUSTOM KERNEL OPERATION: arc-extension
    // The engine has no arc extension: an arc keeping the end's curvature, or a line where the
    // curve is straight there.
    let spline = Spline::of(curve)?;
    let (end, d1, d2) = spline.end_derivatives();
    let t =
        unit(d1).ok_or_else(|| KernelError("the curve has no direction at its end".to_owned()))?;
    let speed = length(d1);
    let bend = cross(d1, d2);
    let curvature = length(bend) / speed.powi(3);
    let piece = if curvature < 1e-9 {
        Curve::line(end, plus(end, t, by))?
    } else {
        let radius = 1.0 / curvature;
        let inward = unit(cross(bend, d1)).unwrap_or([0.0; 3]);
        let centre = plus(end, inward, radius);
        let x = unit(sub(end, centre)).unwrap_or([1.0, 0.0, 0.0]);
        let angle = (by / radius).min(std::f64::consts::PI * 1.9);
        let tip = plus(
            plus(centre, x, radius * angle.cos()),
            t,
            radius * angle.sin(),
        );
        Curve::arc(centre, end, tip, x, t)?
    };
    Ok((Curve::compound(vec![curve.clone(), piece.clone()])?, piece))
}

/// The curve extended at its end or, when `at_start`, at its start.
fn extended_at(
    curve: &Curve,
    at_start: bool,
    by: f64,
    reach: Reach,
) -> Result<(Curve, Curve), KernelError> {
    if at_start {
        let (whole, piece) = extended(&curve.reversed(), by, reach)?;
        Ok((whole.reversed(), piece.reversed()))
    } else {
        extended(curve, by, reach)
    }
}

/// `Extend`: lengthens curves to boundary curves, or, when Enter is pressed instead of choosing
/// boundaries, by a length. `Type` is a line (the default), an arc keeping the end's curvature,
/// or smooth (the curve's own shape carried on). With a length, `AtStart` extends the start.
#[derive(Default)]
pub struct Extend;

impl Extend {
    fn reach(cx: &Context<'_>) -> Reach {
        match cx.mode {
            Some("Arc") => Reach::Arc,
            Some("Smooth") => Reach::Smooth,
            _ => Reach::Line,
        }
    }
}

impl Recipe for Extend {
    const NAME: &'static str = "extend";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("Line"),
        OptionDef::mode("Arc"),
        OptionDef::mode("Smooth"),
        OptionDef::toggle("AtStart", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extend.boundary")
                .curves()
                .optional(),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.extend.curves").curves());
        }
        if cx.values.len() >= 2 && cx.values[0] == Value::Empty {
            steps.push(StepDef::number("prompt.extend.length").remembered("Length", 10.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves = curves_of(cx.doc, objects(cx, 1));
        if curves.is_empty() {
            return Ok(Made::default());
        }
        let reach = Self::reach(cx);
        let mut made = Made::default();
        if cx.values[0] == Value::Empty {
            let Some(by) = number(cx, 2) else {
                return Ok(Made::default());
            };
            if by <= 0.0 {
                return Err(Message::new("error.bad_distance"));
            }
            for (id, curve) in curves {
                let (whole, _) =
                    extended_at(&curve, cx.toggle("AtStart"), by, reach).map_err(|e| failed(&e))?;
                made.replace.push((id, Shape::Curve(whole)));
            }
        } else {
            let boundaries: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
                .into_iter()
                .map(|(_, c)| c)
                .collect();
            for (id, curve) in curves {
                if let Some(whole) = to_boundary(&curve, &boundaries, reach)? {
                    made.replace.push((id, Shape::Curve(whole)));
                }
            }
        }
        made.message = Some(Message::with(
            "done.extend",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

/// The curve extended at whichever end reaches a boundary soonest, up to that boundary.
fn to_boundary(
    curve: &Curve,
    boundaries: &[Curve],
    reach: Reach,
) -> Result<Option<Curve>, Message> {
    // Far enough to cross any boundary: twenty times the size of everything involved.
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for points in std::iter::once(curve)
        .chain(boundaries)
        .filter_map(|c| c.sample().ok())
    {
        for p in points {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    let far = (gap(low, high) * 20.0).max(1.0);
    let mut best: Option<(f64, Curve)> = None;
    for at_start in [false, true] {
        let Ok((_, piece)) = extended_at(curve, at_start, far, reach) else {
            continue;
        };
        for boundary in boundaries {
            let Ok(hits) = curva_kernel::crossings(&piece, boundary) else {
                continue;
            };
            for hit in hits {
                let Ok(parts) = piece.cut_at(&[hit]) else {
                    continue;
                };
                let Some(first) = parts.first() else { continue };
                let reached = first.length().unwrap_or(f64::INFINITY);
                if reached < 1e-9 || best.as_ref().is_some_and(|(d, _)| *d <= reached) {
                    continue;
                }
                let whole = if at_start {
                    Curve::compound(vec![first.reversed().clone(), curve.clone()])
                } else {
                    Curve::compound(vec![curve.clone(), first.clone()])
                };
                if let Ok(whole) = whole {
                    best = Some((reached, whole));
                }
            }
        }
    }
    Ok(best.map(|(_, whole)| whole))
}

#[cfg(test)]
mod tests {

    use curva_document::Document;
    use curva_kernel::Curve;

    fn only_curve(doc: &Document) -> Curve {
        doc.state().objects.last().unwrap().curve().unwrap().clone()
    }

    #[test]
    fn extend_by_a_length_and_to_a_boundary() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extend");
        processor.submit(&mut doc, "");
        doc.select(&[line]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "3");
        assert!((only_curve(&doc).length().unwrap() - 8.0).abs() < 1e-6);
        // To a boundary: a wall at x = 12.
        let mut doc = Document::default();
        let wall = doc.add(Curve::line([12.0, -5.0, 0.0], [12.0, 5.0, 0.0]).unwrap());
        let line = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "extend");
        doc.select(&[wall]);
        processor.submit(&mut doc, "");
        doc.select(&[line]);
        processor.submit(&mut doc, "");
        let extended = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == line)
            .unwrap()
            .curve()
            .unwrap()
            .clone();
        assert!(
            (extended.length().unwrap() - 12.0).abs() < 1e-6,
            "{}",
            extended.length().unwrap()
        );
    }
}
