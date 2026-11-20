//! The `NetworkSrf` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::curves_of;
use crate::recipe::{Context, Made, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point, dot, sub, unit};

/// The curves split into two families running across each other, each in order along the other.
fn families(curves: &[Curve]) -> Option<(Vec<Curve>, Vec<Curve>)> {
    let chord = |c: &Curve| -> Option<(Point, Point)> {
        let points = c.sample().ok()?;
        let (a, b) = (*points.first()?, *points.last()?);
        Some((unit(sub(b, a))?, points[points.len() / 2]))
    };
    let first = chord(curves.first()?)?.0;
    let mut us: Vec<(Curve, Point)> = Vec::new();
    let mut vs: Vec<(Curve, Point)> = Vec::new();
    for curve in curves {
        let (direction, middle) = chord(curve)?;
        if dot(direction, first).abs() > 0.7 {
            us.push((curve.clone(), middle));
        } else {
            vs.push((curve.clone(), middle));
        }
    }
    if us.len() < 2 || vs.len() < 2 {
        return None;
    }
    // Each family in order along the other family's direction.
    let along_v = chord(&vs[0].0)?.0;
    let along_u = first;
    us.sort_by(|a, b| dot(a.1, along_v).total_cmp(&dot(b.1, along_v)));
    vs.sort_by(|a, b| dot(a.1, along_u).total_cmp(&dot(b.1, along_u)));
    Some((
        us.into_iter().map(|(c, _)| c).collect(),
        vs.into_iter().map(|(c, _)| c).collect(),
    ))
}

/// `NetworkSrf`: a surface through a network of curves running two ways.
#[derive(Default)]
pub struct NetworkSrf;

impl Recipe for NetworkSrf {
    const NAME: &'static str = "networksrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.networksrf.curves")
                .curves()
                .at_least(4),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.len() < 4 {
            return Ok(Made::default());
        }
        let (us, vs) = families(&curves).ok_or_else(|| Message::new("error.network_families"))?;
        let (body, off) = Body::from_network(&us, &vs).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::with("done.networksrf", vec![format!("{off:.4}")])),
            ..Made::default()
        })
    }
}
