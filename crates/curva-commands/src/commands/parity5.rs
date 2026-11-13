//! The surface, solid, organising and view commands still missing for Rhino parity (spec
//! `docs/specs/rhino-parity-0.4.1.md`): `ExtrudeCrvTapered`, `ExtrudeCrvAlongCrv`,
//! `ExtrudeCrvToPoint`, `RailRevolve`, `Pyramid`, `ExtractSrf`, `MergeAllFaces`, `Wirecut`,
//! `Distribute`, `SelColor`, `Lock`, `Unlock`, `Group`, `Ungroup`, `ShowEdges` and `Plan`.

use super::{ask_objects, bodies_of, curves_of};
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use crate::{Answer, Command, Message, Step, ViewRequest};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve, KernelError, Point, TaperCorners, cross, dot, gap, sub, unit};

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn number(cx: &Context<'_>, index: usize) -> Option<f64> {
    cx.values.get(index).and_then(Value::number)
}

fn objects<'a>(cx: &'a Context<'_>, index: usize) -> &'a [u64] {
    match cx.values.get(index) {
        Some(Value::Objects(ids)) => ids,
        _ => &[],
    }
}

fn failed(error: &KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}

/// The first point of the first curve chosen, where distances are measured from.
fn first_point(cx: &Context<'_>, index: usize) -> Option<Point> {
    curves_of(cx.doc, objects(cx, index))
        .first()
        .and_then(|(_, c)| c.sample().ok())
        .and_then(|p| p.first().copied())
}

/// The lowest and highest corners of a shape.
fn bounds_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape {
        Shape::Body(body) => body.bounds().ok(),
        Shape::Curve(curve) => {
            let points = curve.sample().ok()?;
            let first = *points.first()?;
            Some(points.iter().fold((first, first), |(mut lo, mut hi), p| {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
                (lo, hi)
            }))
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `ExtrudeCrvTapered`: pulls closed flat curves out of the plane with their walls leaning in
/// by `DraftAngle` degrees (out when negative). `Corners` closes the corners `Sharp` (the
/// default), `Round` or `Smooth`; `Solid` caps the ends.
#[derive(Default)]
pub struct ExtrudeCrvTapered;

impl Recipe for ExtrudeCrvTapered {
    const NAME: &'static str = "extrudecrvtapered";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("DraftAngle", 5.0),
        OptionDef::toggle("Solid", true),
        OptionDef::mode("Sharp"),
        OptionDef::mode("Round"),
        OptionDef::mode("Smooth"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvtapered.objects")
                .curves()
                .offering(&[]),
        ];
        if let Some(base) = first_point(cx, 0) {
            steps.push(
                StepDef::along("prompt.extrudecrv.distance", base, up(cx))
                    .remembered("Distance", 10.0),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(distance) = number(cx, 1).filter(|d| d.abs() > 1e-9) else {
            return Ok(Made::default());
        };
        let corners = match cx.mode {
            Some("Round") => TaperCorners::Round,
            Some("Smooth") => TaperCorners::Smooth,
            _ => TaperCorners::Sharp,
        };
        let draft = cx.number("DraftAngle").to_radians();
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            if !curve.is_closed().unwrap_or(false) {
                return Err(Message::new("error.tapered_closed"));
            }
            let body = Body::extruded_tapered(&curve, up(cx), distance, draft, corners, cx.toggle("Solid"))
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `ExtrudeCrvAlongCrv`: moves curves along a path curve without turning them; closed flat
/// curves make solids with `Solid`.
#[derive(Default)]
pub struct ExtrudeCrvAlongCrv;

impl Recipe for ExtrudeCrvAlongCrv {
    const NAME: &'static str = "extrudecrvalongcrv";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Solid", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvalongcrv.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.extrudecrvalongcrv.path").curves().one());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let paths = curves_of(cx.doc, objects(cx, 1));
        let Some((_, path)) = paths.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let cap = cx.toggle("Solid") && curve.is_closed().unwrap_or(false);
            let body = Body::swept_translated(std::slice::from_ref(&curve), path, cap)
                .map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `ExtrudeCrvToPoint`: lines from every point of curves to one point: cones and pyramids,
/// solids from closed curves with `Solid`.
#[derive(Default)]
pub struct ExtrudeCrvToPoint;

impl Recipe for ExtrudeCrvToPoint {
    const NAME: &'static str = "extrudecrvtopoint";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Solid", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.extrudecrvtopoint.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.extrudecrvtopoint.apex"));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(apex) = point(cx, 1) else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let cap = cx.toggle("Solid") && curve.is_closed().unwrap_or(false);
            let body = Body::extruded_to_point(&curve, apex, cap).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.extrudecrv",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `RailRevolve`: turns a profile curve a full turn about an axis, stretched away from the axis
/// so it follows a rail curve round it.
#[derive(Default)]
pub struct RailRevolve;

/// Angles at which the profile is placed exactly; the surface is skinned between them.
const RAIL_STATIONS: usize = 72;

impl Recipe for RailRevolve {
    const NAME: &'static str = "railrevolve";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.railrevolve.profile").curves().one()];
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
        let (profiles, rails) = (curves_of(cx.doc, objects(cx, 0)), curves_of(cx.doc, objects(cx, 1)));
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

// ---------------------------------------------------------------------------------------------

/// `Pyramid`: a pyramid on a regular polygon: its centre, a corner (or, with `Circumscribed`,
/// the middle of a side), then its height. `NumSides` sets how many sides.
#[derive(Default)]
pub struct Pyramid;

impl Recipe for Pyramid {
    const NAME: &'static str = "pyramid";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("NumSides", 5, 3, 100),
        OptionDef::toggle("Circumscribed", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.pyramid.centre")];
        if let Some(c) = point(cx, 0) {
            steps.push(StepDef::point("prompt.pyramid.corner").from(c));
            if point(cx, 1).is_some() {
                steps.push(StepDef::along("prompt.pyramid.height", c, up(cx)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(corner)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let normal = up(cx);
        let toward = sub(corner, c);
        let flat = sub(toward, normal.map(|v| v * dot(toward, normal)));
        let x = unit(flat).ok_or_else(|| Message::new("error.reference_point"))?;
        let y = cross(normal, x);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sides = cx.number("NumSides").round().max(3.0) as usize;
        let step = std::f64::consts::TAU / sides as f64;
        let mut radius = dot(flat, flat).sqrt();
        let mut start = 0.0;
        if cx.toggle("Circumscribed") {
            radius /= (step / 2.0).cos();
            start = step / 2.0;
        }
        let corners: Vec<Point> = (0..sides)
            .map(|i| {
                let angle = start + step * i as f64;
                plus(plus(c, x, radius * angle.cos()), y, radius * angle.sin())
            })
            .collect();
        let base = Curve::polyline(corners, true).map_err(|e| failed(&e))?;
        let Some(height) = number(cx, 2).filter(|h| h.abs() > 1e-9) else {
            return Ok(Made {
                guides: vec![base],
                ..Made::default()
            });
        };
        let body = Body::pyramid(&base, plus(c, normal, height)).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.pyramid")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `ExtractSrf`: takes picked faces out of polysurfaces as surfaces of their own; with `Copy`,
/// copies them and leaves the polysurface whole.
#[derive(Default)]
pub struct ExtractSrf;

impl Recipe for ExtractSrf {
    const NAME: &'static str = "extractsrf";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("Copy", false)];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::faces("prompt.extractsrf.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Faces(picked)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut owners: Vec<u64> = picked.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, &owners) {
            let faces: Vec<usize> = picked
                .iter()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, face)| *face)
                .collect();
            let count = body.face_count().map_err(|e| failed(&e))?;
            let taken = body.faces_only(&faces).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(taken),
                from: id,
                history: None,
            });
            if !cx.toggle("Copy") {
                let rest: Vec<usize> = (0..count).filter(|f| !faces.contains(f)).collect();
                if rest.is_empty() {
                    made.remove.push(id);
                } else {
                    let left = body.faces_only(&rest).map_err(|e| failed(&e))?;
                    made.replace.push((id, Shape::Body(left)));
                }
            }
        }
        made.message = Some(Message::with(
            "done.extractsrf",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `MergeAllFaces`: merges the faces of polysurfaces that lie on one surface and share an edge.
#[derive(Default)]
pub struct MergeAllFaces;

impl Recipe for MergeAllFaces {
    const NAME: &'static str = "mergeallfaces";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.mergeallfaces.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        let mut merged = 0;
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let before = body.face_count().map_err(|e| failed(&e))?;
            let after_body = body.faces_merged().map_err(|e| failed(&e))?;
            let after = after_body.face_count().map_err(|e| failed(&e))?;
            if after < before {
                merged += before - after;
                made.replace.push((id, Shape::Body(after_body)));
            }
        }
        made.message = Some(Message::with("done.mergeallfaces", vec![merged.to_string()]));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Wirecut`: cuts solids with a curve pulled straight through them, out of the plane being drawn
/// on. `KeepAll` (the default) keeps both parts; off, only the part behind the cut.
#[derive(Default)]
pub struct Wirecut;

impl Recipe for Wirecut {
    const NAME: &'static str = "wirecut";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("KeepAll", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.wirecut.curve").curves().one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.wirecut.objects").bodies());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let cutters = curves_of(cx.doc, objects(cx, 0));
        let Some((_, cutter)) = cutters.first() else {
            return Ok(Made::default());
        };
        let normal = up(cx);
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 1)) {
            let (low, high) = body.bounds().map_err(|e| failed(&e))?;
            let reach = gap(low, high).max(1.0) * 2.0;
            // The curve pulled through from well behind to well in front.
            let sheet = Body::extruded(
                &[cutter.translated(normal.map(|v| -v * reach))],
                normal,
                2.0 * reach,
                false,
            )
            .map_err(|e| failed(&e))?;
            let Some((behind, front)) = body.split_by(&sheet).map_err(|e| failed(&e))? else {
                continue;
            };
            made.replace.push((id, Shape::Body(behind)));
            if cx.toggle("KeepAll") {
                made.derived.push(Derived {
                    shape: Shape::Body(front),
                    from: id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.wirecut",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Distribute`: spaces objects evenly along `X` (the default), `Y` or `Z`, the first and last
/// staying put: their centres evenly apart, or with `Gap`, the gaps between them.
#[derive(Default)]
pub struct Distribute;

impl Recipe for Distribute {
    const NAME: &'static str = "distribute";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode("X"),
        OptionDef::mode("Y"),
        OptionDef::mode("Z"),
        OptionDef::toggle("Gap", false),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.distribute.objects")
                .at_least(3)
                .offering(&["X", "Y", "Z", "Gap"]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let axis = match cx.mode {
            Some("Y") => 1,
            Some("Z") => 2,
            _ => 0,
        };
        let mut items: Vec<(u64, Shape, f64, f64)> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| objects(cx, 0).contains(&o.id))
            .filter_map(|o| {
                let (lo, hi) = bounds_of(&o.shape)?;
                Some((o.id, o.shape.clone(), lo[axis], hi[axis]))
            })
            .collect();
        if items.len() < 3 {
            return Ok(Made::default());
        }
        items.sort_by(|a, b| (a.2 + a.3).total_cmp(&(b.2 + b.3)));
        let count = items.len();
        let mut made = Made::default();
        if cx.toggle("Gap") {
            let span = items[count - 1].3 - items[0].2;
            let widths: f64 = items.iter().map(|i| i.3 - i.2).sum();
            let gap_size = (span - widths) / (count - 1) as f64;
            let mut at = items[0].3 + gap_size;
            for (id, shape, lo, hi) in &items[1..count - 1] {
                let mut by = [0.0; 3];
                by[axis] = at - lo;
                made.replace.push((*id, shape.translated(by)));
                at += hi - lo + gap_size;
            }
        } else {
            let centre = |i: &(u64, Shape, f64, f64)| (i.2 + i.3) / 2.0;
            let (first, last) = (centre(&items[0]), centre(&items[count - 1]));
            for (index, item) in items.iter().enumerate().take(count - 1).skip(1) {
                let wanted = first + (last - first) * index as f64 / (count - 1) as f64;
                let mut by = [0.0; 3];
                by[axis] = wanted - centre(item);
                made.replace.push((item.0, item.1.translated(by)));
            }
        }
        made.message = Some(Message::with("done.distribute", vec![count.to_string()]));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// The colour an object shows in: its layer's.
fn colour_of(doc: &Document, id: u64) -> Option<[f32; 3]> {
    let layer = doc.layer_of(id)?;
    doc.layers().iter().find(|l| l.id == layer).map(|l| l.colour)
}

/// `SelColor`: selects every object showing in the same colour as the one picked.
#[derive(Default)]
pub struct SelColor;

impl Command for SelColor {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        let Some(colour) = chosen.first().and_then(|id| colour_of(doc, *id)) else {
            return ask_objects("prompt.selcolor.object");
        };
        let found: Vec<u64> = doc
            .selectable()
            .into_iter()
            .filter(|id| colour_of(doc, *id) == Some(colour))
            .collect();
        doc.select(&found);
        Step::Done(Some(Message::with(
            "done.select",
            vec![found.len().to_string()],
        )))
    }
}

/// `Lock`: locks objects: they stay in sight, drawn dimmed, but cannot be picked or changed.
#[derive(Default)]
pub struct Lock;

impl Command for Lock {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.lock.objects");
        }
        let count = doc.lock(&chosen);
        Step::Done(Some(Message::with("done.lock", vec![count.to_string()])))
    }
}

/// `Unlock`: unlocks every locked object.
#[derive(Default)]
pub struct Unlock;

impl Command for Unlock {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let count = doc.unlock_all();
        Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
    }
}

/// `Group`: groups objects, so picking one picks them all.
#[derive(Default)]
pub struct Group;

impl Command for Group {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.len() < 2 {
            return ask_objects("prompt.group.objects");
        }
        doc.group(&chosen);
        Step::Done(Some(Message::with(
            "done.group",
            vec![chosen.len().to_string()],
        )))
    }
}

/// `Ungroup`: breaks up the groups of the objects chosen.
#[derive(Default)]
pub struct Ungroup;

impl Command for Ungroup {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.ungroup.objects");
        }
        let count = doc.ungroup(&chosen);
        Step::Done(Some(Message::with("done.ungroup", vec![count.to_string()])))
    }
}

// ---------------------------------------------------------------------------------------------

/// `ShowEdges`: shows the edges of surfaces and polysurfaces, the open (naked) ones only with
/// `NakedEdges` (the default), and counts them; Enter ends it.
#[derive(Default)]
pub struct ShowEdges;

impl Recipe for ShowEdges {
    const NAME: &'static str = "showedges";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle("NakedEdges", true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.showedges.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.showedges.done").optional());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut guides = Vec::new();
        let (mut all, mut naked) = (0, 0);
        for (_, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let open = body.naked_edges().map_err(|e| failed(&e))?;
            let mesh = body.mesh().map_err(|e| failed(&e))?;
            all += mesh.edge_ids.len();
            naked += open.len();
            let shown: Vec<usize> = if cx.toggle("NakedEdges") {
                open
            } else {
                mesh.edge_ids.clone()
            };
            for edge in shown {
                if let Ok(curve) = body.edge_curve(edge) {
                    guides.push(curve);
                }
            }
        }
        Ok(Made {
            guides,
            message: Some(Message::with(
                "done.showedges",
                vec![all.to_string(), naked.to_string()],
            )),
            ..Made::default()
        })
    }
}

/// `Plan`: turns the last-used viewport to look straight down on its construction plane.
#[derive(Default)]
pub struct Plan;

impl Command for Plan {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::Plan)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use crate::ViewRequest;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn square(doc: &mut Document, size: f64) -> u64 {
        doc.add(
            Curve::polyline(
                vec![[0.0; 3], [size, 0.0, 0.0], [size, size, 0.0], [0.0, size, 0.0]],
                true,
            )
            .unwrap(),
        )
    }

    fn last_volume(doc: &Document) -> f64 {
        doc.state()
            .objects
            .last()
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn tapered_extrusions_lean_in() {
        let mut doc = Document::default();
        let id = square(&mut doc, 10.0);
        doc.select(&[id]);
        run(&mut doc, &["extrudecrvtapered", "DraftAngle", "45", "2"]);
        let expected = 2.0 / 3.0 * (100.0 + 36.0 + 60.0);
        assert!((last_volume(&doc) - expected).abs() < 1e-3, "{}", last_volume(&doc));
        // Open curves are refused.
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[line]);
        run(&mut doc, &["extrudecrvtapered", "2"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn extrude_along_a_curve_and_to_a_point() {
        let mut doc = Document::default();
        let id = square(&mut doc, 2.0);
        let path = doc.add(Curve::line([0.0; 3], [3.0, 0.0, 4.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[id]);
        processor.submit(&mut doc, "extrudecrvalongcrv");
        doc.select(&[path]);
        processor.submit(&mut doc, "");
        assert!((last_volume(&doc) - 16.0).abs() < 1e-3);
        let mut doc = Document::default();
        let id = square(&mut doc, 3.0);
        doc.select(&[id]);
        run(&mut doc, &["extrudecrvtopoint", "1.5,1.5,4"]);
        assert!((last_volume(&doc) - 12.0).abs() < 1e-6);
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
        let wall = doc.state().objects.last().unwrap().shape.body().unwrap().area().unwrap();
        let perimeter = rail_length(&doc, rail);
        assert!((wall - 2.0 * perimeter).abs() < 0.01 * wall, "{wall} against {}", 2.0 * perimeter);
    }

    fn rail_length(doc: &Document, id: u64) -> f64 {
        doc.state().objects.iter().find(|o| o.id == id).unwrap().curve().unwrap().length().unwrap()
    }

    #[test]
    fn pyramids_on_regular_polygons() {
        let mut doc = Document::default();
        run(&mut doc, &["pyramid", "NumSides", "4", "0,0", "1,0", "3"]);
        // A square of diagonal 2 (area 2), 3 high: volume 2.
        assert!((last_volume(&doc) - 2.0).abs() < 1e-6, "{}", last_volume(&doc));
        let mut doc = Document::default();
        run(&mut doc, &["pyramid", "NumSides", "4", "Circumscribed", "0,0", "1,0", "3"]);
        // Sides touching radius 1: a 2 × 2 square, volume 4.
        assert!((last_volume(&doc) - 4.0).abs() < 1e-6, "{}", last_volume(&doc));
    }

    #[test]
    fn extract_a_face_and_merge_faces() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select_faces(&[(cube, 0)]);
        run(&mut doc, &["extractsrf", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        let faces: Vec<usize> = doc
            .state()
            .objects
            .iter()
            .map(|o| o.shape.body().unwrap().face_count().unwrap())
            .collect();
        assert_eq!(faces, vec![5, 1]);
        // Two boxes joined side by side have coplanar faces; merging them leaves a box's six.
        let mut doc = Document::default();
        let a = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let b = Body::cuboid([1.0, 0.0, 0.0], [1.0, 1.0, 1.0]).unwrap();
        let joined = a.combined(&b, curva_kernel::Combine::Union).unwrap().unwrap();
        let id = doc.add_body(joined);
        doc.select(&[id]);
        run(&mut doc, &["mergeallfaces"]);
        assert_eq!(doc.state().objects[0].shape.body().unwrap().face_count().unwrap(), 6);
    }

    #[test]
    fn wirecut_cuts_a_box_in_two() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let cutter = doc.add(Curve::line([-1.0, 1.0, 0.0], [3.0, 1.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        doc.select(&[cutter]);
        processor.submit(&mut doc, "wirecut");
        doc.select(&[cube]);
        processor.submit(&mut doc, "");
        let volumes: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .map(|b| b.volume().unwrap())
            .collect();
        assert_eq!(volumes.len(), 2, "{volumes:?}");
        assert!(volumes.iter().all(|v| (v - 4.0).abs() < 1e-6), "{volumes:?}");
    }

    #[test]
    fn distribute_spaces_centres_and_gaps() {
        let mut doc = Document::default();
        let ids: Vec<u64> = [0.0, 1.0, 9.0, 10.0]
            .iter()
            .map(|x| doc.add(Curve::line([*x, 0.0, 0.0], [x + 1.0, 0.0, 0.0]).unwrap()))
            .collect();
        doc.select(&ids);
        run(&mut doc, &["distribute"]);
        let starts: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .map(|o| o.curve().unwrap().sample().unwrap()[0][0])
            .collect();
        for (got, want) in starts.iter().zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0]) {
            assert!((got - want).abs() < 1e-9, "{starts:?}");
        }
    }

    #[test]
    fn select_by_colour_lock_and_group() {
        let mut doc = Document::default();
        let red = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(red, [1.0, 0.0, 0.0]);
        let a = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([0.0, 1.0, 0.0], [1.0, 1.0, 0.0]).unwrap());
        let c = doc.add(Curve::line([0.0, 2.0, 0.0], [1.0, 2.0, 0.0]).unwrap());
        doc.move_to_layer(&[a, c], red);
        doc.select(&[a]);
        run(&mut doc, &["selcolor"]);
        assert_eq!(doc.selection(), vec![a, c]);
        doc.select(&[b]);
        run(&mut doc, &["lock"]);
        assert!(!doc.selectable().contains(&b));
        run(&mut doc, &["unlock"]);
        assert!(doc.selectable().contains(&b));
        doc.select(&[a, b]);
        run(&mut doc, &["group"]);
        let mut picked = doc.with_groups(&[a]);
        picked.sort_unstable();
        assert_eq!(picked, vec![a, b]);
        doc.select(&[a]);
        run(&mut doc, &["ungroup"]);
        assert_eq!(doc.with_groups(&[a]), vec![a]);
        // Locking is one undo step.
        doc.select(&[c]);
        run(&mut doc, &["lock"]);
        doc.undo();
        assert!(doc.selectable().contains(&c));
    }

    #[test]
    fn show_edges_counts_and_plan_turns_the_view() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,3"]);
        doc.select(&[doc.state().objects[0].id]);
        let processor = run(&mut doc, &["showedges", ""]);
        assert!(processor.log().iter().any(|line| line.contains("4 edges, 4 of them naked")), "{:?}", processor.log());
        let mut processor = run(&mut doc, &["plan"]);
        assert_eq!(processor.take_view_request(), Some(ViewRequest::Plan));
    }
}
