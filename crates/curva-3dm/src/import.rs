//! A [`Model`] read from a file brought into a Curva document.

use crate::model::{self, Brep, Geometry, Model, P3};
use curva_document::blocks::{BlockDefinition, Instance};
use curva_document::{
    Document, Layer, Linetype, Object, PrintWidth, SavedCPlane, SavedView, Shape, State, Unit,
};
use curva_kernel::record::{
    Brep as Record, Curve as RecordCurve, Curve2, Edge, Face, Loop, OrientedEdge, OrientedFace,
    Region, Shell, Surface as RecordSurface, Vertex,
};
use curva_kernel::{Body, Curve, KernelError, NurbsSurface, Spline};
use std::collections::BTreeMap;

/// What a read brought in and what it left out, for the command bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Objects brought in, by kind.
    pub brought: Vec<(String, usize)>,
    /// Things left out, by kind.
    pub skipped: Vec<(String, usize)>,
    /// Anything else worth saying, such as a change of unit.
    pub notes: Vec<String>,
}

impl Report {
    fn count(list: &mut Vec<(String, usize)>, kind: &str) {
        match list.iter_mut().find(|(k, _)| k == kind) {
            Some((_, n)) => *n += 1,
            None => list.push((kind.to_owned(), 1)),
        }
    }

    /// The report as one line: what came in, then what was left out.
    #[must_use]
    pub fn summary(&self) -> String {
        let list = |items: &[(String, usize)]| {
            items
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut out = if self.brought.is_empty() {
            "nothing brought in".to_owned()
        } else {
            format!("brought in {}", list(&self.brought))
        };
        if !self.skipped.is_empty() {
            out.push_str(&format!("; left out {}", list(&self.skipped)));
        }
        for note in &self.notes {
            out.push_str("; ");
            out.push_str(note);
        }
        out
    }
}

/// Millimetres in one of openNURBS' length units, if Curva knows it.
fn millimetres_per(unit: u32) -> Option<f64> {
    Some(match unit {
        1 => 1e-3,
        2 => 1.0,
        3 => 10.0,
        4 => 1000.0,
        5 => 1e6,
        6 => 2.54e-5,
        7 => 0.0254,
        8 => 25.4,
        9 => 304.8,
        10 => 1_609_344.0,
        12 => 1e-7,
        13 => 1e-6,
        14 => 100.0,
        15 => 1e4,
        16 => 1e5,
        17 => 1e9,
        18 => 1e12,
        19 => 914.4,
        20 => 25.4 / 72.0,
        21 => 25.4 / 6.0,
        22 => 1_852_000.0,
        _ => return None,
    })
}

fn unit_name(unit: u32) -> &'static str {
    match unit {
        1 => "microns",
        3 => "centimetres",
        5 => "kilometres",
        6 => "microinches",
        7 => "mils",
        8 => "inches",
        9 => "feet",
        10 => "miles",
        14 => "decimetres",
        19 => "yards",
        _ => "another unit",
    }
}

/// A file read as a new model, with the file's layers only.
#[must_use]
pub fn open(model: &Model) -> (Document, Report) {
    let mut report = Report::default();
    let mut state = State {
        layers: Vec::new(),
        next_layer: 0,
        ..State::default()
    };
    // Rhino's metres, inches and feet are Curva's own; any other unit comes in as millimetres.
    state.unit = match model.unit {
        4 => Unit::Metres,
        8 => Unit::Inches,
        9 => Unit::Feet,
        _ => Unit::Millimetres,
    };
    bring_in(model, &mut state, &mut report);
    if state.layers.is_empty() {
        state.layers = State::default().layers;
        state.next_layer = State::default().next_layer;
    }
    state.current_layer = state.layers[0].id;
    let mut doc = Document::default();
    doc.restore(state);
    add_views_and_planes(model, &mut doc, &mut report);
    (doc, report)
}

/// A file's contents added to a model: layers merged by name, everything scaled to its unit.
/// The caller records the change for undo.
pub fn import(model: &Model, doc: &mut Document) -> Report {
    let mut report = Report::default();
    let mut state = doc.state().clone();
    bring_in(model, &mut state, &mut report);
    doc.restore(state);
    add_views_and_planes(model, doc, &mut report);
    report
}

fn bring_in(model: &Model, state: &mut State, report: &mut Report) {
    let file_mm = millimetres_per(model.unit).unwrap_or_else(|| {
        report
            .notes
            .push("the file's unit is unknown, read as millimetres".to_owned());
        1.0
    });
    let scale = file_mm / state.unit.millimetres();
    if (scale - 1.0).abs() > 1e-12 {
        report.notes.push(format!(
            "converted from {} to {}",
            unit_name(model.unit),
            state.unit.symbol()
        ));
    }
    for (kind, n) in &model.skipped {
        report.skipped.push((kind.clone(), *n));
    }

    // Layers: by their full names, parents first, merged with any of the same name.
    let full_names = layer_names(&model.layers);
    let layer_ids: Vec<u64> = model
        .layers
        .iter()
        .zip(&full_names)
        .map(|(layer, name)| {
            if let Some(existing) = state.layers.iter().find(|l| &l.name == name) {
                return existing.id;
            }
            let id = state.next_layer;
            state.next_layer += 1;
            state.layers.push(Layer {
                id,
                name: name.clone(),
                colour: colour_of(layer.color),
                visible: layer.visible,
                locked: layer.locked,
                print_width: (layer.print_width > 0.0)
                    .then(|| PrintWidth::clamped(layer.print_width)),
                linetype: Linetype::Continuous,
            });
            id
        })
        .collect();
    let layer_of = |index: usize| layer_ids.get(index).copied();

    // Groups: a new Curva group for each of the file's.
    let group_ids: Vec<u64> = model
        .groups
        .iter()
        .map(|_| {
            let id = state.next_group;
            state.next_group += 1;
            id
        })
        .collect();

    // Block definitions first, so instances can name them.
    let mut block_ids: BTreeMap<crate::archive::Uuid, u64> = BTreeMap::new();
    for block in &model.blocks {
        let id = state.next_block;
        state.next_block += 1;
        block_ids.insert(block.id, id);
    }
    let in_blocks: BTreeMap<crate::archive::Uuid, crate::archive::Uuid> = model
        .blocks
        .iter()
        .flat_map(|b| b.objects.iter().map(move |o| (*o, b.id)))
        .collect();
    let mut definitions: BTreeMap<crate::archive::Uuid, Vec<Object>> = BTreeMap::new();
    let mut own_colours = 0;

    for object in &model.objects {
        let a = &object.attributes;
        let (shape, kind) = match shape_of(&object.geometry, &block_ids) {
            Ok(made) => made,
            Err(why) => {
                Report::count(&mut report.skipped, &format!("unreadable ({why})"));
                continue;
            }
        };
        let shape = match scaled(shape, scale) {
            Ok(shape) => shape,
            Err(why) => {
                Report::count(&mut report.skipped, &format!("unreadable ({why})"));
                continue;
            }
        };
        if a.color_source == model::Source::Object {
            own_colours += 1;
        }
        let id = state.next_id;
        state.next_id += 1;
        let made = Object {
            id,
            shape,
            layer: layer_of(a.layer).unwrap_or(state.current_layer),
            hidden: !a.visible || a.mode == 1,
            name: a.name.clone(),
            recipe: None,
            print_width: match a.print_width_source {
                model::Source::Object if a.print_width > 0.0 => {
                    PrintWidth::Millimetres(PrintWidth::clamped(a.print_width))
                }
                model::Source::Object => PrintWidth::Default,
                _ => PrintWidth::ByLayer,
            },
            locked: a.mode == 2,
            group: a.groups.first().and_then(|g| group_ids.get(*g)).copied(),
            live: None,
            input_of: None,
            linetype: Linetype::ByLayer,
            user_text: a.user_text.iter().cloned().collect(),
        };
        if let Some(block) = in_blocks.get(&a.id) {
            definitions.entry(*block).or_default().push(made);
        } else if a.mode == 3 {
            // Part of a definition the file does not list: left out.
            Report::count(&mut report.skipped, "loose block parts");
        } else {
            Report::count(&mut report.brought, kind);
            state.objects.push(std::sync::Arc::new(made));
        }
    }
    for block in &model.blocks {
        let Some(id) = block_ids.get(&block.id).copied() else {
            continue;
        };
        let mut name = block.name.clone();
        while state.blocks.iter().any(|b| b.name == name) {
            name.push_str(" (imported)");
        }
        let objects = definitions.remove(&block.id).unwrap_or_default();
        if block.linked.is_some() && objects.is_empty() {
            report.notes.push(format!(
                "block \"{}\" is linked to {}, which was not read",
                block.name,
                block.linked.clone().unwrap_or_default()
            ));
        }
        state.blocks.push(BlockDefinition {
            id,
            name,
            objects,
            linked: None,
        });
    }
    if own_colours > 0 {
        report.notes.push(format!(
            "{own_colours} objects had their own colour and show in their layer's"
        ));
    }
}

/// Each layer's full name, as Rhino shows nested layers: `Parent::Child`.
fn layer_names(layers: &[model::Layer]) -> Vec<String> {
    layers
        .iter()
        .map(|layer| {
            let mut name = layer.name.clone();
            let mut parent = layer.parent;
            let mut depth = 0;
            while parent != crate::archive::Uuid::NIL && depth < 32 {
                let Some(up) = layers.iter().find(|l| l.id == parent) else {
                    break;
                };
                name = format!("{}::{name}", up.name);
                parent = up.parent;
                depth += 1;
            }
            name
        })
        .collect()
}

/// A layer colour: Rhino's black, which would vanish on Curva's dark viewport, as Curva's own.
fn colour_of(c: [u8; 3]) -> [f32; 3] {
    if c == [0, 0, 0] {
        curva_document::STANDARD_COLOUR
    } else {
        c.map(|v| f32::from(v) / 255.0)
    }
}

fn scaled(shape: Shape, scale: f64) -> Result<Shape, String> {
    if (scale - 1.0).abs() < 1e-12 {
        return Ok(shape);
    }
    let m = [
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    curva_document::blocks::shape_placed(&shape, &m).ok_or_else(|| "scaling failed".to_owned())
}

fn shape_of(
    geometry: &Geometry,
    blocks: &BTreeMap<crate::archive::Uuid, u64>,
) -> Result<(Shape, &'static str), String> {
    let text = |e: KernelError| e.0;
    Ok(match geometry {
        Geometry::Point(p) => (Shape::Curve(Curve::Point { at: *p }), "points"),
        Geometry::Curve(c) => (Shape::Curve(curve(c).map_err(text)?), "curves"),
        Geometry::Surface(s) => (Shape::Body(surface_body(s).map_err(text)?), "surfaces"),
        Geometry::Brep(b) => (Shape::Body(brep_body(b).map_err(text)?), "polysurfaces"),
        Geometry::Extrusion(e) => (Shape::Body(extrusion_body(e).map_err(text)?), "extrusions"),
        Geometry::Instance { block, xform } => {
            let definition = *blocks.get(block).ok_or("a block that is not defined")?;
            // openNURBS transforms columns; Curva's placements are rows.
            let placement = std::array::from_fn(|i| std::array::from_fn(|j| xform[j][i]));
            (
                Shape::Instance(Instance {
                    definition,
                    placement,
                }),
                "block instances",
            )
        }
    })
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn lerp(a: P3, b: P3, t: f64) -> P3 {
    std::array::from_fn(|k| a[k] + t * (b[k] - a[k]))
}

/// Where `s` falls between `d[0]` and `d[1]`, from 0 to 1.
fn fraction(d: [f64; 2], s: f64) -> f64 {
    if (d[1] - d[0]).abs() < f64::MIN_POSITIVE {
        0.0
    } else {
        (s - d[0]) / (d[1] - d[0])
    }
}

/// A file curve as a Curva curve, each kind as itself where Curva has it.
fn curve(c: &model::Curve) -> Result<Curve, KernelError> {
    match c {
        model::Curve::Line { ends, .. } => Curve::line(ends[0], ends[1]),
        model::Curve::Polyline { points, .. } => {
            let closed =
                points.len() > 3 && curva_kernel::gap(points[0], points[points.len() - 1]) < 1e-12;
            let mut points = points.clone();
            if closed {
                points.pop();
            }
            if points.len() == 2 && !closed {
                return Curve::line(points[0], points[1]);
            }
            Curve::polyline(points, closed)
        }
        model::Curve::Arc {
            plane,
            radius,
            angles,
            ..
        } => {
            let sweep = angles[1] - angles[0];
            let dir = |a: f64| -> P3 {
                std::array::from_fn(|k| a.cos() * plane.x[k] + a.sin() * plane.y[k])
            };
            if (sweep - std::f64::consts::TAU).abs() < 1e-9 {
                Curve::circle_on(plane.origin, *radius, dir(angles[0]), dir(angles[0] + FRAC))
            } else {
                Ok(Curve::Arc {
                    centre: plane.origin,
                    radius: *radius,
                    x: dir(angles[0]),
                    y: dir(angles[0] + FRAC),
                    sweep,
                })
            }
        }
        model::Curve::Nurbs(n) => {
            let spline = spline_of(n)?;
            match as_circle(&spline) {
                Some(circle) => Ok(circle),
                None => spline.to_curve(),
            }
        }
        model::Curve::Poly { segments, .. } => {
            let parts = segments.iter().map(curve).collect::<Result<Vec<_>, _>>()?;
            if parts.len() == 1 {
                return Ok(parts
                    .into_iter()
                    .next()
                    .unwrap_or(Curve::Point { at: [0.0; 3] }));
            }
            Curve::compound(parts)
        }
    }
}

const FRAC: f64 = std::f64::consts::FRAC_PI_2;

/// A rational quadratic spline that is exactly a circle or an arc of one, as Curva's own circle
/// or arc, as Rhino writes circles in NURBS form.
fn as_circle(s: &Spline) -> Option<Curve> {
    if s.degree != 2 || s.points.iter().all(|p| (p[3] - 1.0).abs() < 1e-12) {
        return None;
    }
    let (a, b) = s.domain();
    let at = |f: f64| s.point(a + (b - a) * f);
    let start = at(0.0);
    let end = at(1.0);
    let closed =
        curva_kernel::gap(start, end) < 1e-9 * (1.0 + start.iter().map(|v| v.abs()).sum::<f64>());
    let (p, q) = if closed {
        (at(1.0 / 3.0), at(2.0 / 3.0))
    } else {
        (at(0.5), end)
    };
    // The circle through start, p and q.
    let u = sub(p, start);
    let v = sub(q, start);
    let n = curva_kernel::cross(u, v);
    let nn: f64 = n.iter().map(|x| x * x).sum();
    if nn < 1e-24 {
        return None;
    }
    let uu: f64 = u.iter().map(|x| x * x).sum();
    let vv: f64 = v.iter().map(|x| x * x).sum();
    let t1 = curva_kernel::cross(n, u);
    let t2 = curva_kernel::cross(v, n);
    let centre: P3 = std::array::from_fn(|k| start[k] + (vv * t1[k] + uu * t2[k]) / (2.0 * nn));
    let radius = curva_kernel::gap(centre, start);
    let normal = curva_kernel::unit(n)?;
    for i in 0..=48 {
        let x = at(f64::from(i) / 48.0);
        let off: f64 = (0..3).map(|k| (x[k] - centre[k]) * normal[k]).sum();
        if (curva_kernel::gap(centre, x) - radius).abs() > 1e-9 * radius.max(1.0)
            || off.abs() > 1e-9 * radius.max(1.0)
        {
            return None;
        }
    }
    let x = curva_kernel::unit(sub(start, centre))?;
    let y = curva_kernel::cross(normal, x);
    if closed {
        return Curve::circle_on(centre, radius, x, y).ok();
    }
    let d = sub(end, centre);
    let along: f64 = (0..3).map(|k| d[k] * x[k]).sum();
    let across: f64 = (0..3).map(|k| d[k] * y[k]).sum();
    let mut sweep = across.atan2(along);
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    Some(Curve::Arc {
        centre,
        radius,
        x,
        y,
        sweep,
    })
}

/// A NURBS curve as a clamped spline, keeping its parameter.
fn spline_of(n: &model::NurbsCurve) -> Result<Spline, KernelError> {
    let mut knots = Vec::with_capacity(n.knots.len() + 2);
    knots.push(*n.knots.first().unwrap_or(&0.0));
    knots.extend_from_slice(&n.knots);
    knots.push(*n.knots.last().unwrap_or(&1.0));
    Spline {
        degree: n.order - 1,
        knots,
        points: n.points.clone(),
    }
    .clamped()
}

/// The part of a file curve between parameters `a` and `b` of it, as a spline on `[a, b]`.
fn portion(c: &model::Curve, a: f64, b: f64) -> Result<Spline, KernelError> {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    Ok(match c {
        model::Curve::Line { ends, domain } => {
            let p = |s: f64| lerp(ends[0], ends[1], fraction(*domain, s));
            Spline {
                degree: 1,
                knots: vec![a, a, b, b],
                points: vec![w(p(a)), w(p(b))],
            }
        }
        model::Curve::Polyline { points, params } => {
            let mut knots = vec![params.first().copied().unwrap_or(0.0)];
            knots.extend_from_slice(params);
            knots.push(params.last().copied().unwrap_or(1.0));
            cut(
                Spline {
                    degree: 1,
                    knots,
                    points: points.iter().map(|p| w(*p)).collect(),
                },
                a,
                b,
            )
        }
        model::Curve::Arc {
            plane,
            radius,
            angles,
            domain,
        } => {
            let angle = |s: f64| angles[0] + fraction(*domain, s) * (angles[1] - angles[0]);
            let (from, to) = (angle(a), angle(b));
            let dir = |t: f64| -> P3 {
                std::array::from_fn(|k| t.cos() * plane.x[k] + t.sin() * plane.y[k])
            };
            let arc = Curve::Arc {
                centre: plane.origin,
                radius: *radius,
                x: dir(from),
                y: dir(from + FRAC),
                sweep: (to - from).abs().max(1e-12),
            };
            Spline::of(&arc)?.reparameterized(a, b)
        }
        model::Curve::Nurbs(n) => cut(spline_of(n)?, a, b),
        model::Curve::Poly { segments, params } => {
            let mut parts = Vec::new();
            for (i, segment) in segments.iter().enumerate() {
                let (p0, p1) = (params[i], params.get(i + 1).copied().unwrap_or(params[i]));
                let (lo, hi) = (a.max(p0), b.min(p1));
                if hi - lo <= 1e-12 * (1.0 + hi.abs()) {
                    continue;
                }
                let own = domain_of(segment);
                let local = |s: f64| own[0] + fraction([p0, p1], s) * (own[1] - own[0]);
                parts.push(portion(segment, local(lo), local(hi))?.reparameterized(lo, hi));
            }
            let joined = Spline::joined(&parts)?;
            joined.reparameterized(a, b)
        }
    })
}

/// A curve's parameter interval.
fn domain_of(c: &model::Curve) -> [f64; 2] {
    match c {
        model::Curve::Line { domain, .. } | model::Curve::Arc { domain, .. } => *domain,
        model::Curve::Polyline { params, .. } | model::Curve::Poly { params, .. } => [
            params.first().copied().unwrap_or(0.0),
            params.last().copied().unwrap_or(1.0),
        ],
        model::Curve::Nurbs(n) => {
            let d = n.order - 2;
            [
                n.knots.get(d).copied().unwrap_or(0.0),
                n.knots.get(n.points.len() - 1).copied().unwrap_or(1.0),
            ]
        }
    }
}

fn w(p: P3) -> [f64; 4] {
    [p[0], p[1], p[2], 1.0]
}

/// The spline between `a` and `b`.
fn cut(s: Spline, a: f64, b: f64) -> Spline {
    let mut s = s;
    if let Some((_, right)) = s.split(a) {
        s = right;
    }
    if let Some((left, _)) = s.split(b) {
        s = left;
    }
    s
}

/// A file surface as a Curva surface, untrimmed.
fn surface_body(s: &model::Surface) -> Result<Body, KernelError> {
    match s {
        model::Surface::Revolution {
            curve: profile,
            axis,
            angles,
            ..
        } => {
            let profile = curve(profile)?;
            let direction = sub(axis[1], axis[0]);
            let profile = if angles[0].abs() > 1e-12 {
                profile.mapped_affine(turn(axis[0], direction, angles[0]))?
            } else {
                profile
            };
            Body::revolved(&[profile], axis[0], direction, angles[1] - angles[0], false)
        }
        _ => nurbs_surface(s)?.to_body(),
    }
}

/// A turn about the line through `origin` along `axis`, as a row placement.
fn turn(origin: P3, axis: P3, angle: f64) -> [[f64; 4]; 4] {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let [x, y, z] = axis.map(|v| v / len.max(f64::MIN_POSITIVE));
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    // Column-vector rotation, then transposed into rows.
    let r = [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ];
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[j][i];
        }
    }
    let moved: P3 = std::array::from_fn(|j| {
        origin[j] - (origin[0] * m[0][j] + origin[1] * m[1][j] + origin[2] * m[2][j])
    });
    m[3] = [moved[0], moved[1], moved[2], 1.0];
    m
}

/// A file surface's NURBS form, on the file's own parameters.
fn nurbs_surface(s: &model::Surface) -> Result<NurbsSurface, KernelError> {
    match s {
        model::Surface::Nurbs(n) => {
            let full = |k: &[f64]| {
                let mut out = vec![*k.first().unwrap_or(&0.0)];
                out.extend_from_slice(k);
                out.push(*k.last().unwrap_or(&1.0));
                out
            };
            clamped_surface(NurbsSurface {
                degree: [n.order[0] - 1, n.order[1] - 1],
                u_knots: full(&n.knots[0]),
                v_knots: full(&n.knots[1]),
                points: n.points.clone(),
            })
        }
        model::Surface::Plane {
            plane,
            domain,
            extents,
        } => {
            let at = |x: f64, y: f64| -> P3 {
                std::array::from_fn(|k| plane.origin[k] + x * plane.x[k] + y * plane.y[k])
            };
            let [[x0, x1], [y0, y1]] = *extents;
            let [[u0, u1], [v0, v1]] = *domain;
            NurbsSurface::new(
                [1, 1],
                vec![u0, u0, u1, u1],
                vec![v0, v0, v1, v1],
                &[vec![at(x0, y0), at(x0, y1)], vec![at(x1, y0), at(x1, y1)]],
                None,
            )
        }
        model::Surface::Sum { curves, base } => {
            let a = portion(
                &curves[0],
                domain_of(&curves[0])[0],
                domain_of(&curves[0])[1],
            )?;
            let b = portion(
                &curves[1],
                domain_of(&curves[1])[0],
                domain_of(&curves[1])[1],
            )?;
            let rational = |s: &Spline| s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12);
            if rational(&a) || rational(&b) {
                return Err(KernelError("a sum surface of rational curves".to_owned()));
            }
            let net: Vec<Vec<P3>> = a
                .points
                .iter()
                .map(|p| {
                    b.points
                        .iter()
                        .map(|q| std::array::from_fn(|k| p[k] + q[k] + base[k]))
                        .collect()
                })
                .collect();
            NurbsSurface::new([a.degree, b.degree], a.knots, b.knots, &net, None)
        }
        model::Surface::Revolution { .. } => Err(KernelError(
            "a surface of revolution has no NURBS form here".to_owned(),
        )),
    }
}

/// A surface with knots repeated at both ends of its domain, both ways.
fn clamped_surface(s: NurbsSurface) -> Result<NurbsSurface, KernelError> {
    // Along v: each row as a spline; along u: each column.
    let rows: Vec<Spline> = s
        .points
        .iter()
        .map(|row| {
            Spline {
                degree: s.degree[1],
                knots: s.v_knots.clone(),
                points: row.clone(),
            }
            .clamped()
        })
        .collect::<Result<_, _>>()?;
    let v_knots = rows.first().map(|r| r.knots.clone()).unwrap_or_default();
    let net: Vec<Vec<[f64; 4]>> = rows.into_iter().map(|r| r.points).collect();
    let columns: Vec<Spline> = (0..net.first().map_or(0, Vec::len))
        .map(|j| {
            Spline {
                degree: s.degree[0],
                knots: s.u_knots.clone(),
                points: net.iter().map(|row| row[j]).collect(),
            }
            .clamped()
        })
        .collect::<Result<_, _>>()?;
    let u_knots = columns.first().map(|c| c.knots.clone()).unwrap_or_default();
    let count = columns.first().map_or(0, |c| c.points.len());
    let points = (0..count)
        .map(|i| columns.iter().map(|c| c.points[i]).collect())
        .collect();
    Ok(NurbsSurface {
        degree: s.degree,
        u_knots,
        v_knots,
        points,
    })
}

fn record_curve(s: &Spline) -> RecordCurve {
    if s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12) {
        RecordCurve::RationalSpline {
            knots: s.knots.clone(),
            homogeneous: s.points.clone(),
        }
    } else {
        RecordCurve::Spline {
            knots: s.knots.clone(),
            points: s.points.iter().map(|p| [p[0], p[1], p[2]]).collect(),
        }
    }
}

fn record_curve2(s: &Spline) -> Curve2 {
    if s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12) {
        Curve2::RationalSpline {
            knots: s.knots.clone(),
            homogeneous: s.points.iter().map(|p| [p[0], p[1], p[3]]).collect(),
        }
    } else {
        Curve2::Spline {
            knots: s.knots.clone(),
            points: s.points.iter().map(|p| [p[0], p[1]]).collect(),
        }
    }
}

/// `x ↦ scale x + shift`.
#[derive(Clone, Copy, Debug)]
struct Linear {
    scale: f64,
    shift: f64,
}

impl Linear {
    /// The map taking `from` onto `to`.
    fn between(from: [f64; 2], to: [f64; 2]) -> Self {
        let span = from[1] - from[0];
        let scale = if span.abs() < f64::MIN_POSITIVE {
            1.0
        } else {
            (to[1] - to[0]) / span
        };
        Self {
            scale,
            shift: to[0] - scale * from[0],
        }
    }

    fn at(self, x: f64) -> f64 {
        self.scale * x + self.shift
    }
}

/// How a file surface's parameter along one direction becomes its record surface's.
#[derive(Clone, Debug)]
enum Warp {
    /// Linearly, so trim curves carry over exactly.
    Linear(Linear),
    /// Through circular arcs: the file's parameter is linear in the angle, the record's runs
    /// over rational quadratic arcs, one per span of [`arc_spans`].
    Arcs {
        /// The file's parameter to the angle.
        angle: Linear,
        /// The arcs' spans of angle; each span's knots are its angles.
        spans: Vec<[f64; 2]>,
    },
}

impl Warp {
    const SAME: Self = Self::Linear(Linear {
        scale: 1.0,
        shift: 0.0,
    });

    fn at(&self, x: f64) -> f64 {
        match self {
            Self::Linear(l) => l.at(x),
            Self::Arcs { angle, spans } => {
                let theta = angle.at(x);
                let span = spans
                    .iter()
                    .find(|s| theta <= s[1].max(s[0]))
                    .or(spans.last())
                    .copied()
                    .unwrap_or([theta, theta]);
                let half = (span[1] - span[0]) / 2.0;
                if half.abs() < 1e-15 {
                    return span[0];
                }
                // A rational quadratic arc with equal end weights runs linearly in the tangent
                // of the half angle from its middle.
                let mid = span[0] + half;
                let s = 0.5 * (1.0 + ((theta - mid) / 2.0).tan() / (half / 2.0).tan());
                span[0] + s.clamp(0.0, 1.0) * (span[1] - span[0])
            }
        }
    }
}

/// Spans of at most a quarter turn covering `a` to `b`.
fn arc_spans(a: f64, b: f64) -> Vec<[f64; 2]> {
    let n = ((b - a).abs() / FRAC - 1e-9).ceil().max(1.0) as usize;
    let step = (b - a) / n as f64;
    (0..n)
        .map(|k| [a + k as f64 * step, a + (k + 1) as f64 * step])
        .collect()
}

/// The knots of arcs over `spans`, and for each control point its angle (a middle point's is
/// its span's middle) and weight.
fn arc_controls(spans: &[[f64; 2]]) -> (Vec<f64>, Vec<(f64, f64)>) {
    let mut knots = Vec::new();
    let mut controls = Vec::new();
    for (k, s) in spans.iter().enumerate() {
        if k == 0 {
            knots.extend([s[0]; 3]);
            controls.push((s[0], 1.0));
        }
        let half = (s[1] - s[0]) / 2.0;
        controls.push((s[0] + half, half.cos()));
        controls.push((s[1], 1.0));
        if k + 1 == spans.len() {
            knots.extend([s[1]; 3]);
        } else {
            knots.extend([s[1]; 2]);
        }
    }
    (knots, controls)
}

/// A point turned about the line through `origin` along unit `axis` by `angle`, pushed out by
/// `1 / weight` from the axis for an arc's middle control point.
fn turned(p: P3, origin: P3, axis: P3, angle: f64, weight: f64) -> P3 {
    let along: f64 = (0..3).map(|k| (p[k] - origin[k]) * axis[k]).sum();
    let foot: P3 = std::array::from_fn(|k| origin[k] + along * axis[k]);
    let r = sub(p, foot);
    let s = curva_kernel::cross(axis, r);
    let (sin, cos) = angle.sin_cos();
    std::array::from_fn(|k| foot[k] + (r[k] * cos + s[k] * sin) / weight)
}

/// An arc of a file as a spline whose knots are angles, spans of at most a quarter turn.
fn arc_spline(plane: &model::Plane, radius: f64, angles: [f64; 2]) -> (Spline, Vec<[f64; 2]>) {
    let spans = arc_spans(angles[0], angles[1]);
    let (knots, controls) = arc_controls(&spans);
    let points = controls
        .iter()
        .map(|(angle, weight)| {
            let (sin, cos) = angle.sin_cos();
            let p: P3 = std::array::from_fn(|k| {
                plane.origin[k] + radius * (cos * plane.x[k] + sin * plane.y[k]) / weight
            });
            [p[0] * weight, p[1] * weight, p[2] * weight, *weight]
        })
        .collect();
    (
        Spline {
            degree: 2,
            knots,
            points,
        },
        spans,
    )
}

/// A record surface, how a file surface's (u, v) become its own, and its domain.
type Placed = (RecordSurface, [Warp; 2], [[f64; 2]; 2]);

/// A file surface as the engine's record surface, with how the file's (u, v) become the
/// record's, and the record's domain.
fn record_surface(s: &model::Surface) -> Result<Placed, KernelError> {
    match s {
        model::Surface::Plane {
            plane,
            domain,
            extents,
        } => Ok((
            RecordSurface::Plane {
                origin: plane.origin,
                u_axis: plane.x,
                v_axis: plane.y,
            },
            [
                Warp::Linear(Linear::between(domain[0], extents[0])),
                Warp::Linear(Linear::between(domain[1], extents[1])),
            ],
            *extents,
        )),
        model::Surface::Revolution {
            curve: profile,
            axis,
            angles,
            domain,
            transposed,
        } => {
            // The profile as a spline, arcs on angle knots so its parameter is known.
            let (profile_spline, profile_warp) = match profile.as_ref() {
                model::Curve::Arc {
                    plane,
                    radius,
                    angles: arc_angles,
                    domain: arc_domain,
                } => {
                    let (spline, spans) = arc_spline(plane, *radius, *arc_angles);
                    (
                        spline,
                        Warp::Arcs {
                            angle: Linear::between(*arc_domain, *arc_angles),
                            spans,
                        },
                    )
                }
                other => {
                    let d = domain_of(other);
                    (portion(other, d[0], d[1])?, Warp::SAME)
                }
            };
            let direction = curva_kernel::unit(sub(axis[1], axis[0]))
                .ok_or_else(|| KernelError("a revolution axis of no length".to_owned()))?;
            let spans = arc_spans(angles[0], angles[1]);
            let (angle_knots, controls) = arc_controls(&spans);
            // net[i][j]: i along the angle, j along the profile.
            let net: Vec<Vec<[f64; 4]>> = controls
                .iter()
                .map(|(angle, arc_weight)| {
                    profile_spline
                        .points
                        .iter()
                        .map(|h| {
                            let w = h[3];
                            let p = [h[0] / w, h[1] / w, h[2] / w];
                            let q = turned(p, axis[0], direction, *angle, *arc_weight);
                            let ww = w * arc_weight;
                            [q[0] * ww, q[1] * ww, q[2] * ww, ww]
                        })
                        .collect()
                })
                .collect();
            let angle_warp = Warp::Arcs {
                angle: Linear::between(*domain, *angles),
                spans,
            };
            let (a0, a1) = (angles[0], angles[1]);
            let (p0, p1) = profile_spline.domain();
            let (u_knots, v_knots, points, warps, record_domain) = if *transposed {
                // u along the profile: the net turned over.
                let turned_net = (0..profile_spline.points.len())
                    .map(|j| net.iter().map(|row| row[j]).collect())
                    .collect();
                (
                    profile_spline.knots.clone(),
                    angle_knots,
                    turned_net,
                    [profile_warp, angle_warp],
                    [[p0, p1], [a0, a1]],
                )
            } else {
                (
                    angle_knots,
                    profile_spline.knots.clone(),
                    net,
                    [angle_warp, profile_warp],
                    [[a0, a1], [p0, p1]],
                )
            };
            Ok((
                RecordSurface::RationalSpline {
                    u_knots,
                    v_knots,
                    homogeneous: points,
                },
                warps,
                record_domain,
            ))
        }
        _ => {
            let n = nurbs_surface(s)?;
            let domain = n.domain();
            Ok((n.as_record(), [Warp::SAME, Warp::SAME], domain))
        }
    }
}

/// A trim curve in the file surface's parameters taken onto the record surface's: exactly
/// where both maps are linear, as a straight line where it lies along one parameter, and
/// otherwise through closely spaced points.
fn warped(pcurve: Spline, warps: &[Warp; 2]) -> Spline {
    if let [Warp::Linear(u), Warp::Linear(v)] = warps {
        let mut out = pcurve;
        for p in &mut out.points {
            let w = p[3];
            p[0] = u.scale * p[0] + u.shift * w;
            p[1] = v.scale * p[1] + v.shift * w;
            p[2] = 0.0;
        }
        return out;
    }
    let (a, b) = pcurve.domain();
    let count = (8 * pcurve.points.len()).clamp(16, 256);
    let samples: Vec<P3> = (0..=count)
        .map(|i| {
            let p = pcurve.point(a + (b - a) * i as f64 / count as f64);
            [warps[0].at(p[0]), warps[1].at(p[1]), 0.0]
        })
        .collect();
    let first = samples[0];
    let last = samples[samples.len() - 1];
    let along = |k: usize| {
        samples
            .iter()
            .all(|p| (p[k] - first[k]).abs() <= 1e-12 * (1.0 + first[k].abs()))
    };
    let points: Vec<[f64; 4]> = if along(0) || along(1) {
        vec![w(first), w(last)]
    } else {
        samples.iter().map(|p| w(*p)).collect()
    };
    let n = points.len();
    let mut knots = vec![a, a];
    for i in 1..n - 1 {
        knots.push(a + (b - a) * i as f64 / (n - 1) as f64);
    }
    knots.extend([b, b]);
    Spline {
        degree: 1,
        knots,
        points,
    }
}

/// A B-rep as a Curva body: the file's own topology turned into the engine's record, which the
/// engine checks and heals.
fn brep_body(b: &Brep) -> Result<Body, KernelError> {
    let bad = |why: &str| KernelError(format!("a B-rep Curva cannot read: {why}"));
    let index = |i: i32| usize::try_from(i).ok();
    let mut surfaces = Vec::with_capacity(b.surfaces.len());
    for s in &b.surfaces {
        surfaces.push(match s {
            Some(s) => Some(record_surface(s)?),
            None => None,
        });
    }
    let tolerance = 1e-5;
    let vertices: Vec<Vertex> = b
        .vertices
        .iter()
        .map(|v| Vertex {
            point: v.point,
            tolerance: if v.tolerance.is_finite() && v.tolerance > 0.0 {
                v.tolerance.max(tolerance)
            } else {
                tolerance
            },
        })
        .collect();
    let mut edges = Vec::with_capacity(b.edges.len());
    for e in &b.edges {
        let c3 = index(e.curve)
            .and_then(|i| b.curves3.get(i))
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("an edge without its curve"))?;
        let mut spline = portion(c3, e.proxy_domain[0], e.proxy_domain[1])?;
        if e.reversed {
            spline = spline.reversed();
        }
        let interval = {
            let (a, b) = spline.domain();
            [a, b]
        };
        edges.push(Edge {
            curve: record_curve(&spline),
            nurbs: None,
            interval,
            vertices: [
                u32::try_from(e.vertices[0]).map_err(|_| bad("an edge without its start"))?,
                u32::try_from(e.vertices[1]).map_err(|_| bad("an edge without its end"))?,
            ],
            tolerance: if e.tolerance.is_finite() && e.tolerance > 0.0 {
                e.tolerance.max(tolerance)
            } else {
                tolerance
            },
            uses: Vec::new(),
        });
    }
    let mut faces = Vec::with_capacity(b.faces.len());
    for f in &b.faces {
        let (surface, maps, domain) = index(f.surface)
            .and_then(|i| surfaces.get(i))
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("a face without its surface"))?
            .clone();
        let mut loops = Vec::new();
        for l in f
            .loops
            .iter()
            .filter_map(|l| index(*l))
            .filter_map(|l| b.loops.get(l))
        {
            let mut uses = Vec::new();
            for t in l
                .trims
                .iter()
                .filter_map(|t| index(*t))
                .filter_map(|t| b.trims.get(t))
            {
                // A trim at a pole has no edge: the engine closes the loop there itself.
                let Some(edge_index) = index(t.edge) else {
                    continue;
                };
                let edge = edges
                    .get(edge_index)
                    .ok_or_else(|| bad("a trim without its edge"))?;
                let c2 = index(t.curve)
                    .and_then(|i| b.curves2.get(i))
                    .and_then(Option::as_ref)
                    .ok_or_else(|| bad("a trim without its curve"))?;
                let mut pcurve = portion(c2, t.proxy_domain[0], t.proxy_domain[1])?;
                if t.proxy_reversed {
                    pcurve = pcurve.reversed();
                }
                // Onto the record surface's parameters.
                pcurve = warped(pcurve, &maps);
                let forward = !t.reversed3d;
                if !forward {
                    pcurve = pcurve.reversed();
                }
                let pcurve = pcurve.reparameterized(edge.interval[0], edge.interval[1]);
                uses.push(OrientedEdge {
                    edge: u32::try_from(edge_index).map_err(|_| bad("too many edges"))?,
                    forward,
                    pcurve: record_curve2(&pcurve),
                });
            }
            if !uses.is_empty() {
                loops.push(Loop::Edges { edges: uses });
            }
        }
        // The domain is already in the record surface's parameters.
        let natural = [sorted(domain[0]), sorted(domain[1])];
        faces.push(Face {
            surface,
            nurbs: None,
            domain: natural,
            loops,
        });
    }
    // Shells: faces joined through shared edges; a shell whose every edge has two uses is
    // closed and bounds a solid.
    let face_count = faces.len();
    let mut parent: Vec<usize> = (0..face_count).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
    for (fi, face) in faces.iter().enumerate() {
        for l in &face.loops {
            if let Loop::Edges { edges: uses } = l {
                for u in uses {
                    users[u.edge as usize].push(fi);
                }
            }
        }
    }
    for list in &users {
        for pair in list.windows(2) {
            let (a, b) = (root(&mut parent, pair[0]), root(&mut parent, pair[1]));
            parent[a] = b;
        }
    }
    let mut shell_of: BTreeMap<usize, usize> = BTreeMap::new();
    let mut shells: Vec<Vec<OrientedFace>> = Vec::new();
    for (fi, f) in b.faces.iter().enumerate() {
        let r = root(&mut parent, fi);
        let s = *shell_of.entry(r).or_insert_with(|| {
            shells.push(Vec::new());
            shells.len() - 1
        });
        shells[s].push(OrientedFace {
            face: u32::try_from(fi).map_err(|_| bad("too many faces"))?,
            forward: !f.reversed,
        });
    }
    let closed: Vec<bool> = shells
        .iter()
        .map(|faces_in| {
            let inside: Vec<usize> = faces_in.iter().map(|f| f.face as usize).collect();
            users
                .iter()
                .filter(|list| list.iter().any(|f| inside.contains(f)))
                .all(|list| list.len() == 2)
        })
        .collect();
    let shell_ids: Vec<u32> = (0..shells.len()).map(|s| s as u32).collect();
    let mut regions = vec![Region {
        shells: shell_ids.clone(),
        void: false,
    }];
    for (s, is_closed) in closed.iter().enumerate() {
        if *is_closed {
            regions.push(Region {
                shells: vec![s as u32],
                void: false,
            });
        }
    }
    let record = Record {
        version: curva_kernel::record::VERSION,
        tolerance,
        regions,
        shells: shells
            .into_iter()
            .map(|faces| Shell::Faces {
                faces,
                wires: Vec::new(),
            })
            .collect(),
        faces,
        edges,
        vertices,
    };
    Body::from_foreign_record(&record)
}

fn sorted(d: [f64; 2]) -> [f64; 2] {
    if d[0] <= d[1] { d } else { [d[1], d[0]] }
}

/// A file curve moved by a rigid row placement.
fn placed(c: &model::Curve, m: &[[f64; 4]; 4]) -> model::Curve {
    let point = |p: P3| -> P3 {
        std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
    };
    let vector =
        |p: P3| -> P3 { std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j]) };
    match c {
        model::Curve::Line { ends, domain } => model::Curve::Line {
            ends: ends.map(point),
            domain: *domain,
        },
        model::Curve::Polyline { points, params } => model::Curve::Polyline {
            points: points.iter().map(|p| point(*p)).collect(),
            params: params.clone(),
        },
        model::Curve::Arc {
            plane,
            radius,
            angles,
            domain,
        } => model::Curve::Arc {
            plane: model::Plane {
                origin: point(plane.origin),
                x: vector(plane.x),
                y: vector(plane.y),
                z: vector(plane.z),
            },
            radius: *radius,
            angles: *angles,
            domain: *domain,
        },
        model::Curve::Nurbs(n) => {
            let mut n = n.clone();
            for h in &mut n.points {
                let w = h[3];
                let q = point([h[0] / w, h[1] / w, h[2] / w]);
                *h = [q[0] * w, q[1] * w, q[2] * w, w];
            }
            n.dimension = 3;
            model::Curve::Nurbs(n)
        }
        model::Curve::Poly { segments, params } => model::Curve::Poly {
            segments: segments.iter().map(|s| placed(s, m)).collect(),
            params: params.clone(),
        },
    }
}

/// An extrusion object as a Curva body: its profiles pulled along its path, capped as the file
/// says.
fn extrusion_body(e: &model::Extrusion) -> Result<Body, KernelError> {
    let along = sub(e.path[1], e.path[0]);
    let length = (along[0] * along[0] + along[1] * along[1] + along[2] * along[2]).sqrt();
    let z = curva_kernel::unit(along)
        .ok_or_else(|| KernelError("an extrusion of no length".to_owned()))?;
    let y = curva_kernel::unit(e.up).unwrap_or([0.0, 1.0, 0.0]);
    let x = curva_kernel::cross(y, z);
    let o = e.path[0];
    let placement = [
        [x[0], x[1], x[2], 0.0],
        [y[0], y[1], y[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [o[0], o[1], o[2], 1.0],
    ];
    // Placed while still file curves, so circles stay circles.
    let profiles = e
        .profiles
        .iter()
        .map(|p| curve(&placed(p, &placement)))
        .collect::<Result<Vec<_>, _>>()?;
    let cap = e.caps[0] && e.caps[1] && profiles.iter().all(|p| p.is_closed().unwrap_or(false));
    if let Ok(body) = Body::extruded(&profiles, z, length, cap) {
        return Ok(body);
    }
    // Profiles the sweep does not take (weighted ones): each side a surface of its own, the
    // caps flat faces, sewn together.
    let by: P3 = z.map(|v| v * length);
    let mut parts = Vec::new();
    for profile in &profiles {
        let spline = Spline::of(profile)?;
        let points = spline
            .points
            .iter()
            .map(|h| {
                vec![
                    *h,
                    [
                        h[0] + by[0] * h[3],
                        h[1] + by[1] * h[3],
                        h[2] + by[2] * h[3],
                        h[3],
                    ],
                ]
            })
            .collect();
        parts.push(
            NurbsSurface {
                degree: [spline.degree, 1],
                u_knots: spline.knots.clone(),
                v_knots: vec![0.0, 0.0, 1.0, 1.0],
                points,
            }
            .to_body()?,
        );
    }
    if cap {
        let bottom = Body::planar(&profiles)?;
        parts.push(bottom.translated(by));
        parts.push(bottom);
    }
    Body::assembled(&parts)
}

/// Curva's field of view, by which a view's distance gives how much of the model it shows.
pub(crate) const HALF_FIELD: f64 = 0.35;

fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: P3) -> Option<P3> {
    let length = dot3(a, a).sqrt();
    (length > 1e-12 && length.is_finite()).then(|| a.map(|v| v / length))
}

/// A file's view as a Curva saved view, its sizes multiplied by `scale`.
fn saved_view(v: &model::View, scale: f64) -> Option<SavedView> {
    let forward = unit3(v.direction)?;
    // Rhino keeps the up it was given, such as Z for a view looking down at an angle: squared
    // to the line of sight here.
    let up = unit3(sub(v.up, forward.map(|c| c * dot3(v.up, forward))))?;
    let eye = v.location.map(|c| c * scale);
    let target = v.target.map(|c| c * scale);
    // Curva's target lies on the line of sight.
    let along = dot3(sub(target, eye), forward);
    let along = if along > 1e-12 {
        along
    } else {
        0.5 * (v.frustum[4] + v.frustum[5]) * scale
    };
    let target: P3 = std::array::from_fn(|k| eye[k] + along * forward[k]);
    let distance = if v.parallel {
        // A flat view's zoom is its frustum's height.
        (v.frustum[3] - v.frustum[2]).abs() * 0.5 * scale / HALF_FIELD.tan()
    } else {
        along
    };
    if !(distance.is_finite() && distance > 0.0) {
        return None;
    }
    let pitch = (-forward[2]).clamp(-1.0, 1.0).asin();
    let yaw = (-forward[1]).atan2(-forward[0]);
    let looks = |axis: usize, sign: f64| v.parallel && forward[axis] * sign > 1.0 - 1e-9;
    let kind = if looks(2, -1.0) {
        "Top"
    } else if looks(1, 1.0) {
        "Front"
    } else if looks(0, -1.0) {
        "Right"
    } else {
        "Perspective"
    };
    // Yaw and pitch hold views with Z up; flat views along an axis keep their own directions, as
    // Curva's standard views do, and so does any view that is not upright.
    let right = unit3(cross3(forward, [0.0, 0.0, 1.0]));
    let z_up = kind == "Perspective"
        && right.is_some_and(|r| dot3(cross3(r, forward), up) > 1.0 - 1e-6)
        && pitch.abs() < 1.55;
    let axes = if z_up {
        None
    } else {
        let right = unit3(cross3(forward, up))?;
        Some([forward, right, cross3(right, forward)])
    };
    Some(SavedView {
        name: v.name.clone(),
        kind: kind.to_owned(),
        target,
        yaw,
        pitch,
        distance,
        axes,
        parallel: v.parallel,
    })
}

/// A file's named views and construction planes for a Curva model of `unit`; views Curva cannot
/// use (no name, a broken camera) are counted in the report.
fn views_and_planes(
    model: &Model,
    unit: Unit,
    report: &mut Report,
) -> (Vec<SavedView>, Vec<SavedCPlane>) {
    let scale = millimetres_per(model.unit).unwrap_or(1.0) / unit.millimetres();
    let mut views = Vec::new();
    for v in &model.views {
        match saved_view(v, scale).filter(|s| !s.name.trim().is_empty()) {
            Some(saved) => views.push(saved),
            None => Report::count(&mut report.skipped, "unusable named views"),
        }
    }
    let planes = model
        .cplanes
        .iter()
        .filter_map(|c| {
            let x = unit3(c.plane.x)?;
            let y = unit3(sub(c.plane.y, x.map(|v| v * dot3(c.plane.y, x))))?;
            Some(SavedCPlane {
                name: c.name.clone(),
                origin: c.plane.origin.map(|v| v * scale),
                x,
                y,
            })
        })
        .collect();
    (views, planes)
}

/// Adds a file's named views and planes to a document, keeping any it already has by that name.
fn add_views_and_planes(model: &Model, doc: &mut Document, report: &mut Report) {
    let (views, planes) = views_and_planes(model, doc.state().unit, report);
    for v in views {
        if doc.saved_view(&v.name).is_none() {
            Report::count(&mut report.brought, "named views");
            doc.save_view(v);
        }
    }
    for p in planes {
        if doc.saved_cplane(&p.name).is_none() {
            Report::count(&mut report.brought, "named construction planes");
            doc.save_cplane(p);
        }
    }
}
