//! Commands that were waiting on the geometry engine (spec `docs/specs/engine-gaps.md`), now done
//! through solidcore or Curva's own flagged operations: `EdgeSrf`, `NetworkSrf`, `Patch`,
//! `ExtendSrf`, `Untrim`, `Section`, `Contour`, `Intersect`, `Project`, `MoveFace`, `PushPull`,
//! `Shear`, `RebuildUV`, `CurvatureGraph` and `Cap`.

use super::{bodies_of, curves_of};
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Carry, Curve, KernelError, Point, cross, dot, gap, sub, unit};

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

fn up(cx: &Context<'_>) -> Point {
    let [x, y] = cx.plane();
    unit(cross(x, y)).unwrap_or([0.0, 0.0, 1.0])
}

fn added_curves(curves: Vec<Curve>, from: u64) -> Vec<Derived> {
    curves
        .into_iter()
        .map(|curve| Derived {
            shape: Shape::Curve(curve),
            from,
            history: None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------

/// `EdgeSrf`: a surface bounded by two, three or four curves meeting end to end.
#[derive(Default)]
pub struct EdgeSrf;

impl Recipe for EdgeSrf {
    const NAME: &'static str = "edgesrf";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.edgesrf.curves")
                .curves()
                .at_least(2),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let body = match curves.len() {
            0 | 1 => return Ok(Made::default()),
            2 => Body::ruled(&curves[0], &curves[1], false),
            3 | 4 => Body::from_edges(&curves),
            _ => return Err(Message::new("error.edgesrf_count")),
        }
        .map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.edgesrf")),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------------------------

/// `Patch`: a surface filling a closed chain of curves.
#[derive(Default)]
pub struct Patch;

impl Recipe for Patch {
    const NAME: &'static str = "patch";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.patch.curves").curves()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        if curves.is_empty() {
            return Ok(Made::default());
        }
        // One closed curve is filled through its pieces.
        if curves.len() == 1
            && let Some(pieces) = curves[0].exploded()
        {
            curves = pieces;
        }
        let (body, off) = Body::patched(&curves).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::with("done.patch", vec![format!("{off:.4}")])),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `ExtendSrf`: lengthens a surface past a picked edge by a distance, `Smooth` (the surface
/// carried on, the default) or `Line` (straight along its tangent).
#[derive(Default)]
pub struct ExtendSrf;

impl Recipe for ExtendSrf {
    const NAME: &'static str = "extendsrf";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::mode("Smooth"), OptionDef::mode("Line")];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::edges("prompt.extendsrf.edge").one()];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.extendsrf.distance").remembered("Distance", 10.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Edges(edges)), Some(distance)) = (cx.values.first(), number(cx, 1)) else {
            return Ok(Made::default());
        };
        if distance <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let how = if cx.mode == Some("Line") {
            Carry::Tangent
        } else {
            Carry::Natural
        };
        let mut made = Made::default();
        for (id, edge) in edges {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let longer = body
                .extended_at(*edge, distance, how)
                .map_err(|e| failed(&e))?;
            made.replace.push((*id, Shape::Body(longer)));
        }
        made.message = Some(Message::with(
            "done.extendsrf",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Untrim`: takes a trim away: click a hole's edge to fill it, or an outer edge to bring the
/// face back to its whole surface.
#[derive(Default)]
pub struct Untrim;

impl Recipe for Untrim {
    const NAME: &'static str = "untrim";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::edges("prompt.untrim.edge").one()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Edges(edges)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, edge) in edges {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let whole = body.untrimmed_at(*edge).map_err(|e| failed(&e))?;
            made.replace.push((*id, Shape::Body(whole)));
        }
        made.message = Some(Message::with(
            "done.untrim",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Section`: curves where a vertical plane through a line cuts surfaces and solids: the line's
/// two points, the plane standing up out of the construction plane.
#[derive(Default)]
pub struct Section;

impl Recipe for Section {
    const NAME: &'static str = "section";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.section.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.section.start"));
        }
        if let Some(a) = point(cx, 1) {
            steps.push(StepDef::point("prompt.section.end").from(a));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b)) = (point(cx, 1), point(cx, 2)) else {
            return Ok(Made::default());
        };
        let normal =
            unit(cross(sub(b, a), up(cx))).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let cut = body.section(a, normal).map_err(|e| failed(&e))?;
            made.derived.extend(added_curves(cut, id));
        }
        made.guides = Curve::line(a, b).ok().into_iter().collect();
        made.message = Some(Message::with(
            "done.section",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Contour`: sections every `distance` along a direction: a base point, a point setting the
/// direction, then the distance between the planes.
#[derive(Default)]
pub struct Contour;

impl Recipe for Contour {
    const NAME: &'static str = "contour";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.contour.objects").bodies()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.contour.base"));
        }
        if let Some(a) = point(cx, 1) {
            steps.push(StepDef::point("prompt.contour.direction").from(a));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::number("prompt.contour.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(a), Some(b), Some(step)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if step <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let normal = unit(sub(b, a)).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let (low, high) = body.bounds().map_err(|e| failed(&e))?;
            // The body's extent along the direction, from its box's corners.
            let corners = (0..8).map(|i| {
                [
                    if i & 1 == 0 { low[0] } else { high[0] },
                    if i & 2 == 0 { low[1] } else { high[1] },
                    if i & 4 == 0 { low[2] } else { high[2] },
                ]
            });
            let along: Vec<f64> = corners.map(|c| dot(sub(c, a), normal)).collect();
            let (from, to) = along
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(*v), hi.max(*v))
                });
            let mut k = (from / step).ceil();
            let mut planes = 0;
            while k * step <= to && planes < 2000 {
                let at: Point = std::array::from_fn(|i| a[i] + normal[i] * k * step);
                made.derived.extend(added_curves(
                    body.section(at, normal).map_err(|e| failed(&e))?,
                    id,
                ));
                k += 1.0;
                planes += 1;
            }
        }
        made.message = Some(Message::with(
            "done.contour",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Intersect`: curves where surfaces and solids meet each other, and points where curves cross.
#[derive(Default)]
pub struct Intersect;

impl Recipe for Intersect {
    const NAME: &'static str = "intersect";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.intersect.objects").at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let ids = objects(cx, 0);
        let bodies = bodies_of(cx.doc, ids);
        let curves = curves_of(cx.doc, ids);
        let mut made = Made::default();
        for (i, (id, a)) in bodies.iter().enumerate() {
            for (_, b) in &bodies[i + 1..] {
                made.derived.extend(added_curves(
                    a.intersections(b).map_err(|e| failed(&e))?,
                    *id,
                ));
            }
        }
        for (i, (_, a)) in curves.iter().enumerate() {
            for (_, b) in &curves[i + 1..] {
                for at in curva_kernel::crossings(a, b).unwrap_or_default() {
                    made.add.push(Shape::Curve(Curve::Point { at }));
                }
            }
        }
        made.message = Some(Message::with(
            "done.intersect",
            vec![(made.derived.len() + made.add.len()).to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Project`: curves projected straight down the construction plane's normal onto surfaces.
#[derive(Default)]
pub struct Project;

impl Recipe for Project {
    const NAME: &'static str = "project";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.project.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::objects("prompt.project.onto").bodies());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let curves: Vec<Curve> = curves_of(cx.doc, objects(cx, 0))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let mut made = Made::default();
        let down = up(cx).map(|v| -v);
        for (id, body) in bodies_of(cx.doc, objects(cx, 1)) {
            made.derived.extend(added_curves(
                body.projected(&curves, down).map_err(|e| failed(&e))?,
                id,
            ));
        }
        made.message = Some(Message::with(
            "done.project",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `MoveFace` (and `PushPull`, its Serpentine3D name): moves picked faces of a solid in or out
/// along their normals by a distance, the solid following.
#[derive(Default)]
pub struct MoveFace<const PUSH: bool>;

impl<const PUSH: bool> Recipe for MoveFace<PUSH> {
    const NAME: &'static str = if PUSH { "pushpull" } else { "moveface" };

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::faces("prompt.moveface.faces")];
        if !cx.values.is_empty() {
            steps.push(StepDef::number("prompt.moveface.distance").remembered("Distance", 1.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Faces(picked)), Some(distance)) = (cx.values.first(), number(cx, 1))
        else {
            return Ok(Made::default());
        };
        if distance.abs() < 1e-9 {
            return Err(Message::new("error.bad_distance"));
        }
        let mut owners: Vec<u64> = picked.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, &owners) {
            let faces: Vec<usize> = picked
                .iter()
                .filter(|(o, _)| *o == id)
                .map(|(_, f)| *f)
                .collect();
            let moved = body
                .faces_offset(&faces, distance)
                .map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Body(moved)));
        }
        made.message = Some(Message::with(
            "done.moveface",
            vec![picked.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `Shear`: slants objects: an origin, a reference point setting the slant's direction, then
/// the angle. Points move along the reference direction in proportion to how far they are from
/// the line through the origin along it, in the construction plane.
#[derive(Default)]
pub struct Shear;

impl Shear {
    fn map(
        origin: Point,
        reference: Point,
        angle_degrees: f64,
        normal: Point,
    ) -> Option<[[f64; 4]; 4]> {
        let r = unit(sub(reference, origin))?;
        let p = unit(cross(normal, r))?;
        let k = angle_degrees.to_radians().tan();
        // x' = x + k ((x - o) · p) r: each axis's image, then the move.
        let image = |axis: usize| -> [f64; 4] {
            let mut row = [0.0; 4];
            row[axis] = 1.0;
            for j in 0..3 {
                row[j] += k * p[axis] * r[j];
            }
            row
        };
        let shift = -k * dot(origin, p);
        Some([
            image(0),
            image(1),
            image(2),
            [shift * r[0], shift * r[1], shift * r[2], 1.0],
        ])
    }
}

impl Recipe for Shear {
    const NAME: &'static str = "shear";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.shear.objects")];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.shear.origin"));
        }
        if let Some(o) = point(cx, 1) {
            steps.push(StepDef::point("prompt.shear.reference").from(o));
        }
        if cx.values.len() >= 3 {
            steps.push(StepDef::number("prompt.shear.angle").remembered("Angle", 15.0));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(o), Some(r), Some(angle)) = (point(cx, 1), point(cx, 2), number(cx, 3)) else {
            return Ok(Made::default());
        };
        if angle.abs() >= 89.0 {
            return Err(Message::new("error.shear_angle"));
        }
        let map =
            Self::map(o, r, angle, up(cx)).ok_or_else(|| Message::new("error.reference_point"))?;
        let mut made = Made::default();
        for object in cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| objects(cx, 0).contains(&o.id))
        {
            let shape = match &object.shape {
                Shape::Curve(c) => Shape::Curve(c.mapped_affine(map).map_err(|e| failed(&e))?),
                Shape::Body(b) => Shape::Body(b.transformed(map).map_err(|e| failed(&e))?),
            };
            made.replace.push((object.id, shape));
        }
        made.message = Some(Message::with(
            "done.shear",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `RebuildUV`: refits untrimmed surfaces with a new degree and number of control points each
/// way.
#[derive(Default)]
pub struct RebuildUv;

impl Recipe for RebuildUv {
    const NAME: &'static str = "rebuilduv";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("PointCountU", 6, 2, 100),
        OptionDef::whole("PointCountV", 6, 2, 100),
        OptionDef::whole("DegreeU", 3, 1, 11),
        OptionDef::whole("DegreeV", 3, 1, 11),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.rebuilduv.objects")
                .bodies()
                .offering(&["PointCountU", "PointCountV", "DegreeU", "DegreeV"]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = |name: &str| cx.number(name).round().max(1.0) as usize;
        let degrees = [whole("DegreeU"), whole("DegreeV")];
        let counts = [whole("PointCountU"), whole("PointCountV")];
        let spans = [
            counts[0].saturating_sub(degrees[0]).max(1),
            counts[1].saturating_sub(degrees[1]).max(1),
        ];
        let mut made = Made::default();
        let mut worst: f64 = 0.0;
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let (rebuilt, off) = body.rebuilt_uv(degrees, spans).map_err(|e| failed(&e))?;
            worst = worst.max(off);
            made.replace.push((id, Shape::Body(rebuilt)));
        }
        made.message = Some(Message::with(
            "done.rebuilduv",
            vec![made.replace.len().to_string(), format!("{worst:.4}")],
        ));
        Ok(made)
    }
}

// ---------------------------------------------------------------------------------------------

/// `CurvatureGraph`: shows curves' curvature combs, scaled by `Scale`, with `Density` teeth;
/// Enter ends it.
#[derive(Default)]
pub struct CurvatureGraph;

impl Recipe for CurvatureGraph {
    const NAME: &'static str = "curvaturegraph";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("Scale", 10.0),
        OptionDef::whole("Density", 60, 2, 2000),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.curvaturegraph.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.curvaturegraph.done").optional());
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let scale = cx.number("Scale");
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let teeth = cx.number("Density").round().max(2.0) as usize;
        let mut guides = Vec::new();
        let mut most: f64 = 0.0;
        for (_, curve) in curves_of(cx.doc, objects(cx, 0)) {
            let Ok(comb) = curve.curvature_comb(teeth) else {
                continue;
            };
            // Teeth stand out of the curve, away from its centre of curvature, as in Rhino.
            let tips: Vec<Point> = comb
                .iter()
                .map(|(at, k)| {
                    most = most.max(dot(*k, *k).sqrt());
                    std::array::from_fn(|i| at[i] - k[i] * scale)
                })
                .collect();
            for ((at, _), tip) in comb.iter().zip(&tips) {
                if gap(*at, *tip) > 1e-9
                    && let Ok(tooth) = Curve::line(*at, *tip)
                {
                    guides.push(tooth);
                }
            }
            if tips.len() >= 2
                && let Ok(outline) = Curve::polyline(tips, false)
            {
                guides.push(outline);
            }
        }
        Ok(Made {
            guides,
            message: Some(Message::with(
                "done.curvaturegraph",
                vec![format!("{most:.5}")],
            )),
            ..Made::default()
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// `Cap`: closes the flat open borders of surfaces and polysurfaces with flat faces; closed ones
/// become solids.
#[derive(Default)]
pub struct Cap;

impl Recipe for Cap {
    const NAME: &'static str = "cap";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.cap.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            if body.is_solid() {
                continue;
            }
            let capped = body.capped().map_err(|e| failed(&e))?;
            made.replace.push((id, Shape::Body(capped)));
        }
        made.message = Some(Message::with(
            "done.cap",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn last_body(doc: &Document) -> Body {
        doc.state()
            .objects
            .iter()
            .rev()
            .find_map(|o| o.shape.body().cloned())
            .unwrap()
    }

    fn curves_made(doc: &Document, before: usize) -> Vec<Curve> {
        doc.state().objects[before..]
            .iter()
            .filter_map(|o| o.curve().cloned())
            .collect()
    }

    #[test]
    fn edge_network_and_patch_surfaces() {
        let mut doc = Document::default();
        let ids: Vec<u64> = [
            line([0.0; 3], [3.0, 0.0, 0.0]),
            line([3.0, 0.0, 0.0], [3.0, 3.0, 0.0]),
            line([3.0, 3.0, 0.0], [0.0, 3.0, 0.0]),
            line([0.0, 3.0, 0.0], [0.0; 3]),
        ]
        .into_iter()
        .map(|c| doc.add(c))
        .collect();
        doc.select(&ids);
        run(&mut doc, &["edgesrf"]);
        assert!((last_body(&doc).area().unwrap() - 9.0).abs() < 1e-6);
        doc.select(&ids);
        run(&mut doc, &["patch"]);
        assert!((last_body(&doc).area().unwrap() - 9.0).abs() < 1e-2);
        let mut doc = Document::default();
        let wave = |y: f64| {
            Curve::control_point(vec![[0.0, y, 0.0], [2.0, y, 1.0], [4.0, y, 0.0]], 2, false)
                .unwrap()
        };
        let across = |x: f64| {
            let z = if (x - 2.0).abs() < 1e-9 { 0.5 } else { 0.0 };
            Curve::control_point(vec![[x, 0.0, z], [x, 1.5, z], [x, 3.0, z]], 2, false).unwrap()
        };
        let ids: Vec<u64> = [across(2.0), wave(0.0), across(0.0), wave(3.0), across(4.0)]
            .into_iter()
            .map(|c| doc.add(c))
            .collect();
        doc.select(&ids);
        run(&mut doc, &["networksrf"]);
        assert!(last_body(&doc).area().unwrap() > 12.0);
    }

    #[test]
    fn extend_and_untrim_surfaces() {
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "4,4"]);
        let id = doc.state().objects[0].id;
        let edge = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .mesh()
            .unwrap()
            .edge_ids[0];
        doc.select_edges(&[(id, edge)]);
        run(&mut doc, &["extendsrf", "", "2"]);
        assert!((last_body(&doc).area().unwrap() - 24.0).abs() < 1e-6);
        let mut doc = Document::default();
        let holed = Body::planar(&[
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
            Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap(),
        ])
        .unwrap();
        let hole_edge = (0..holed.record_edge_count())
            .find(|e| holed.edge_curve(*e).unwrap().is_closed().unwrap())
            .unwrap();
        let id = doc.add_body(holed);
        doc.select_edges(&[(id, hole_edge)]);
        run(&mut doc, &["untrim", ""]);
        assert!((last_body(&doc).area().unwrap() - 16.0).abs() < 1e-6);
    }

    #[test]
    fn section_contour_intersect_and_project() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap());
        doc.select(&[cube]);
        run(&mut doc, &["section", "1,-1", "1,5"]);
        let cut = curves_made(&doc, 1);
        assert_eq!(cut.len(), 1);
        assert!(
            (cut[0].length().unwrap() - 14.0).abs() < 1e-6,
            "{}",
            cut[0].length().unwrap()
        );
        let before = doc.state().objects.len();
        doc.select(&[cube]);
        run(&mut doc, &["contour", "0,0,0", "0,0,1", "1"]);
        // Planes at z = 0, 1, 2, 3, 4.
        assert_eq!(curves_made(&doc, before).len(), 5);
        let other = doc.add_body(Body::cuboid([1.0, 1.0, 1.0], [2.0, 3.0, 4.0]).unwrap());
        let before = doc.state().objects.len();
        doc.select(&[cube, other]);
        run(&mut doc, &["intersect"]);
        assert!(!curves_made(&doc, before).is_empty());
        let mut doc = Document::default();
        run(&mut doc, &["plane", "0,0", "5,5"]);
        let floor = doc.state().objects[0].id;
        let wire = doc.add(line([1.0, 1.0, 3.0], [4.0, 1.0, 3.0]));
        let mut processor = crate::Processor::default();
        doc.select(&[wire]);
        processor.submit(&mut doc, "project");
        doc.select(&[floor]);
        processor.submit(&mut doc, "");
        let shadow = curves_made(&doc, 2);
        assert_eq!(shadow.len(), 1);
        assert!(
            shadow[0]
                .sample()
                .unwrap()
                .iter()
                .all(|p| p[2].abs() < 1e-9)
        );
    }

    #[test]
    fn move_faces_shear_rebuild_and_cap() {
        let mut doc = Document::default();
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let mesh = cube.mesh().unwrap();
        let top = mesh
            .triangle_faces
            .iter()
            .zip(&mesh.triangles)
            .find(|(_, t)| {
                t.iter()
                    .all(|i| (mesh.positions[*i as usize][2] - 1.0).abs() < 1e-9)
            })
            .map(|(f, _)| *f)
            .unwrap();
        let id = doc.add_body(cube);
        doc.select_faces(&[(id, top)]);
        run(&mut doc, &["pushpull", "", "0.5"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        doc.select(&[id]);
        run(&mut doc, &["shear", "0,0", "1,0", "30"]);
        assert!((last_body(&doc).volume().unwrap() - 1.5).abs() < 1e-6);
        let mut doc = Document::default();
        let a = Curve::control_point(vec![[0.0; 3], [2.0, 0.0, 1.0], [4.0, 0.0, 0.0]], 2, false)
            .unwrap();
        let b = Curve::control_point(
            vec![[0.0, 3.0, 0.0], [2.0, 3.0, 1.0], [4.0, 3.0, 0.0]],
            2,
            false,
        )
        .unwrap();
        let sheet = doc.add_body(Body::ruled(&a, &b, false).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "rebuilduv");
        for typed in ["PointCountU", "8", "PointCountV", "8"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[sheet]);
        processor.submit(&mut doc, "");
        let (rows, cols, _) = last_body(&doc).control_net().unwrap();
        assert_eq!((rows, cols), (8, 8));
        let mut doc = Document::default();
        let square = Curve::polyline(
            vec![[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            true,
        )
        .unwrap();
        let tube = doc.add_body(Body::extruded(&[square], [0.0, 0.0, 1.0], 3.0, false).unwrap());
        doc.select(&[tube]);
        run(&mut doc, &["cap"]);
        assert!(last_body(&doc).is_solid());
    }

    #[test]
    fn curvature_graph_shows_teeth_and_reports() {
        let mut doc = Document::default();
        let id = doc.add(Curve::circle([0.0; 3], 5.0).unwrap());
        doc.select(&[id]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "curvaturegraph");
        assert!(
            !processor
                .preview(&doc, Some([0.0, 0.0, 0.0]), "")
                .is_empty()
        );
        processor.submit(&mut doc, "");
        assert!(
            processor.log().iter().any(|l| l.contains("0.2")),
            "{:?}",
            processor.log()
        );
    }
}
