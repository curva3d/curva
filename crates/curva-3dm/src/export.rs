//! A Curva document as a [`Model`] for the writer: layers, groups, blocks and objects, every
//! surface as NURBS (spec `docs/specs/3dm-files.md` §2).

use crate::archive::Uuid;
use crate::import::Report;
use crate::model::{
    self, Attributes, Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, BrepVertex, Geometry, Model,
    NurbsCurve, NurbsSurface, P3, Plane, Source,
};
use curva_document::{Object, PrintWidth, Shape, State, Unit};
use curva_kernel::record::{
    Brep as Record, Curve as RecordCurve, Curve2, Loop, Shell, Surface as RecordSurface,
};
use curva_kernel::{Curve, Spline};
use std::collections::BTreeMap;

/// openNURBS' trim kinds.
mod trim_kind {
    pub const BOUNDARY: i32 = 1;
    pub const MATED: i32 = 2;
    pub const SEAM: i32 = 3;
    pub const SINGULAR: i32 = 4;
}

/// The whole model, or only the objects whose ids are in `only`, as a file's model, with a
/// report of what could not be written.
#[must_use]
pub fn export(state: &State, only: Option<&[u64]>) -> (Model, Report) {
    let mut report = Report::default();
    let mut ids = Ids::default();
    let mut model = Model {
        version: 70,
        // Rhino's unit codes; its feet and inches are Curva's.
        unit: match state.unit {
            Unit::Millimetres => 2,
            Unit::Metres => 4,
            Unit::Inches => 8,
            Unit::Feet => 9,
        },
        tolerance: state.unit.tolerance(),
        angle_tolerance: 1f64.to_radians(),
        ..Model::default()
    };

    // Layers: "Parent::Child" names become nested layers again, parents made as needed.
    let mut layer_index: BTreeMap<u64, usize> = BTreeMap::new();
    let mut by_path: BTreeMap<String, usize> = BTreeMap::new();
    for layer in &state.layers {
        let mut parent = Uuid::NIL;
        let parts: Vec<&str> = layer.name.split("::").collect();
        for (depth, part) in parts.iter().enumerate() {
            let path = parts[..=depth].join("::");
            let last = depth + 1 == parts.len();
            let index = match by_path.get(&path) {
                Some(i) => *i,
                None => {
                    model.layers.push(model::Layer {
                        name: (*part).to_owned(),
                        id: ids.next(),
                        parent,
                        color: [0, 0, 0],
                        visible: true,
                        locked: false,
                        print_width: 0.0,
                    });
                    by_path.insert(path, model.layers.len() - 1);
                    model.layers.len() - 1
                }
            };
            if last {
                let l = &mut model.layers[index];
                l.color = colour_of(layer.colour);
                l.visible = layer.visible;
                l.locked = layer.locked;
                l.print_width = layer.print_width.unwrap_or(0.0);
                layer_index.insert(layer.id, index);
            }
            parent = model.layers[index].id;
        }
    }
    if model.layers.is_empty() {
        model.layers.push(model::Layer {
            name: "Default".to_owned(),
            id: ids.next(),
            parent: Uuid::NIL,
            color: [0, 0, 0],
            visible: true,
            locked: false,
            print_width: 0.0,
        });
    }

    let chosen: Vec<&Object> = state
        .objects
        .iter()
        .filter(|o| only.is_none_or(|ids| ids.contains(&o.id)))
        .map(|o| &**o)
        .collect();

    // Blocks: the definitions the chosen objects use, nested ones included.
    let mut needed: Vec<u64> = Vec::new();
    let mut queue: Vec<&Object> = chosen.clone();
    while let Some(o) = queue.pop() {
        if let Shape::Instance(i) = &o.shape
            && !needed.contains(&i.definition)
        {
            needed.push(i.definition);
            if let Some(d) = state.blocks.iter().find(|b| b.id == i.definition) {
                queue.extend(d.objects.iter());
            }
        }
    }
    let block_ids: BTreeMap<u64, Uuid> = state
        .blocks
        .iter()
        .filter(|b| needed.contains(&b.id))
        .map(|b| (b.id, ids.next()))
        .collect();

    let mut groups: BTreeMap<u64, usize> = BTreeMap::new();
    let mut add = |o: &Object, in_block: bool, model: &mut Model, report: &mut Report| {
        let geometry = match geometry_of(&o.shape, &block_ids) {
            Ok(g) => g,
            Err(why) => {
                count(&mut report.skipped, &format!("not written ({why})"));
                return None;
            }
        };
        let group = o.group.map(|g| {
            let n = groups.len();
            *groups.entry(g).or_insert(n)
        });
        let id = ids.next();
        let (print_width, print_width_source) = match o.print_width {
            PrintWidth::ByLayer => (0.0, Source::Layer),
            PrintWidth::Default => (0.0, Source::Object),
            PrintWidth::Millimetres(w) => (w, Source::Object),
        };
        model.objects.push(model::Object {
            geometry,
            attributes: Attributes {
                id,
                layer: layer_index.get(&o.layer).copied().unwrap_or(0),
                name: o.name.clone(),
                print_width,
                print_width_source,
                visible: !o.hidden,
                mode: if in_block {
                    3
                } else if o.hidden {
                    1
                } else if o.locked {
                    2
                } else {
                    0
                },
                groups: group.into_iter().collect(),
                user_text: o
                    .user_text
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                ..Attributes::default()
            },
        });
        if !in_block {
            count(&mut report.brought, kind_of(&o.shape));
        }
        Some(id)
    };

    for b in &state.blocks {
        let Some(id) = block_ids.get(&b.id).copied() else {
            continue;
        };
        let objects = b
            .objects
            .iter()
            .filter_map(|o| add(o, true, &mut model, &mut report))
            .collect();
        model.blocks.push(model::Block {
            name: b.name.clone(),
            id,
            description: String::new(),
            objects,
            linked: None,
        });
    }
    for o in chosen {
        add(o, false, &mut model, &mut report);
    }
    model.groups = (0..groups.len())
        .map(|_| model::Group {
            name: String::new(),
            id: ids.next(),
        })
        .collect();
    if state
        .objects
        .iter()
        .any(|o| o.live.is_some() || o.recipe.is_some())
    {
        report
            .notes
            .push("live booleans and History are saved as plain geometry".to_owned());
    }
    (model, report)
}

/// Fresh ids for one file.
#[derive(Default)]
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> Uuid {
        self.0 += 1;
        Uuid::fresh(self.0)
    }
}

fn count(list: &mut Vec<(String, usize)>, kind: &str) {
    match list.iter_mut().find(|(k, _)| k == kind) {
        Some((_, n)) => *n += 1,
        None => list.push((kind.to_owned(), 1)),
    }
}

fn kind_of(shape: &Shape) -> &'static str {
    match shape {
        Shape::Curve(Curve::Point { .. }) => "points",
        Shape::Curve(_) => "curves",
        Shape::Body(_) => "polysurfaces",
        Shape::Instance(_) => "block instances",
        Shape::Annotation(_) => "texts",
    }
}

/// A layer colour; Curva's standard colour goes back to Rhino's black, as it came in.
fn colour_of(c: [f32; 3]) -> [u8; 3] {
    if c == curva_document::STANDARD_COLOUR {
        [0, 0, 0]
    } else {
        c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

fn geometry_of(shape: &Shape, blocks: &BTreeMap<u64, Uuid>) -> Result<Geometry, String> {
    Ok(match shape {
        Shape::Curve(Curve::Point { at }) => Geometry::Point(*at),
        Shape::Curve(c) => Geometry::Curve(curve(c)?),
        Shape::Body(b) => Geometry::Brep(brep(&b.portable_record().map_err(|e| e.0)?)?),
        Shape::Instance(i) => {
            let block = *blocks
                .get(&i.definition)
                .ok_or("a block that is not defined")?;
            // Curva's placements are rows; openNURBS transforms columns.
            let xform = std::array::from_fn(|i2| std::array::from_fn(|j| i.placement[j][i2]));
            Geometry::Instance { block, xform }
        }
        // Annotation goes into .3dm files with the round trip at the end of Milestone 3.
        Shape::Annotation(_) => {
            return Err("annotation is not written to .3dm files yet".to_owned());
        }
    })
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A Curva curve as the file's: lines, polylines, circles and arcs as themselves, the rest as
/// NURBS.
fn curve(c: &Curve) -> Result<model::Curve, String> {
    Ok(match c {
        Curve::Line { start, end } => model::Curve::Line {
            ends: [*start, *end],
            domain: [0.0, 1.0],
        },
        Curve::Polyline { points, closed } => {
            let mut points = points.clone();
            if *closed && let Some(first) = points.first().copied() {
                points.push(first);
            }
            let params = (0..points.len()).map(|i| i as f64).collect();
            model::Curve::Polyline { points, params }
        }
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => arc(*centre, *radius, *x, *y, std::f64::consts::TAU),
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } => arc(*centre, *radius, *x, *y, *sweep),
        Curve::Compound { segments } => {
            let mut params = vec![0.0];
            let mut out = Vec::with_capacity(segments.len());
            for s in segments {
                let made = curve(s)?;
                let [a, b] = domain_of(&made);
                params.push(params[params.len() - 1] + (b - a));
                out.push(made);
            }
            model::Curve::Poly {
                segments: out,
                params,
            }
        }
        Curve::Point { .. } => return Err("a point as a curve".to_owned()),
        _ => model::Curve::Nurbs(nurbs(&Spline::of(c).map_err(|e| e.0)?)),
    })
}

fn arc(centre: P3, radius: f64, x: P3, y: P3, sweep: f64) -> model::Curve {
    model::Curve::Arc {
        plane: Plane {
            origin: centre,
            x,
            y,
            z: cross(x, y),
        },
        radius,
        angles: [0.0, sweep],
        domain: [0.0, sweep],
    }
}

fn domain_of(c: &model::Curve) -> [f64; 2] {
    match c {
        model::Curve::Line { domain, .. } | model::Curve::Arc { domain, .. } => *domain,
        model::Curve::Polyline { params, .. } | model::Curve::Poly { params, .. } => {
            [params[0], params[params.len() - 1]]
        }
        model::Curve::Nurbs(n) => [n.knots[n.order - 2], n.knots[n.points.len() - 1]],
    }
}

/// openNURBS keeps knots without the two outermost.
fn inner(knots: &[f64]) -> Vec<f64> {
    knots[1..knots.len() - 1].to_vec()
}

fn rational(points: &[[f64; 4]]) -> bool {
    points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12)
}

fn nurbs(s: &Spline) -> NurbsCurve {
    NurbsCurve {
        dimension: 3,
        order: s.degree + 1,
        knots: inner(&s.knots),
        rational: rational(&s.points),
        points: s.points.clone(),
    }
}

/// A full knot vector and homogeneous points as a NURBS curve of `dimension`.
fn nurbs_of(knots: &[f64], points: Vec<[f64; 4]>, dimension: usize) -> NurbsCurve {
    NurbsCurve {
        dimension,
        order: knots.len() - points.len(),
        knots: inner(knots),
        rational: rational(&points),
        points,
    }
}

/// An edge's curve, on its own parameter.
fn edge_curve(c: &RecordCurve, interval: [f64; 2]) -> Result<model::Curve, String> {
    Ok(match c {
        // The record's line runs on t from `start` (0) to `end` (1), and the edge's interval can
        // reach past both: the file's line is the edge's part, on the same t.
        RecordCurve::Line { start, end } => {
            let at = |t: f64| -> P3 { std::array::from_fn(|k| start[k] + t * (end[k] - start[k])) };
            model::Curve::Line {
                ends: [at(interval[0]), at(interval[1])],
                domain: interval,
            }
        }
        RecordCurve::Polyline { points } => model::Curve::Polyline {
            points: points.clone(),
            params: (0..points.len()).map(|i| i as f64).collect(),
        },
        RecordCurve::Circle { frame, radius } => model::Curve::Arc {
            plane: Plane {
                origin: frame.origin,
                x: frame.x_axis,
                y: frame.y_axis,
                z: cross(frame.x_axis, frame.y_axis),
            },
            radius: *radius,
            angles: interval,
            domain: interval,
        },
        RecordCurve::Spline { knots, points } => model::Curve::Nurbs(nurbs_of(
            knots,
            points.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
            3,
        )),
        RecordCurve::RationalSpline { knots, homogeneous } => {
            model::Curve::Nurbs(nurbs_of(knots, homogeneous.clone(), 3))
        }
        RecordCurve::Ellipse { .. } => return Err("an ellipse edge without its NURBS".to_owned()),
        RecordCurve::SurfaceIntersection { .. } => {
            return Err("an intersection edge without its NURBS".to_owned());
        }
    })
}

/// A trim curve as a full knot vector and homogeneous points `[u w, v w, 0, w]`.
fn trim_points(c: &Curve2) -> (Vec<f64>, Vec<[f64; 4]>) {
    match c {
        Curve2::Spline { knots, points } => (
            knots.clone(),
            points.iter().map(|p| [p[0], p[1], 0.0, 1.0]).collect(),
        ),
        Curve2::RationalSpline { knots, homogeneous } => (
            knots.clone(),
            homogeneous
                .iter()
                .map(|p| [p[0], p[1], 0.0, p[2]])
                .collect(),
        ),
    }
}

fn reversed(knots: &[f64], points: &[[f64; 4]]) -> (Vec<f64>, Vec<[f64; 4]>) {
    let (a, b) = (knots[0], knots[knots.len() - 1]);
    (
        knots.iter().rev().map(|k| a + b - k).collect(),
        points.iter().rev().copied().collect(),
    )
}

fn uv(p: [f64; 4]) -> [f64; 2] {
    [p[0] / p[3], p[1] / p[3]]
}

/// Which iso-parametric line, if any, a trim lies on (openNURBS' `ISO`): 1 u constant, 2 v
/// constant, 3 to 6 the west, south, east and north sides of the surface's domain.
fn iso(points: &[[f64; 4]], domain: [[f64; 2]; 2]) -> i32 {
    let size = (domain[0][1] - domain[0][0])
        .abs()
        .max((domain[1][1] - domain[1][0]).abs());
    let tol = 1e-9 * size.max(1.0);
    let uvs: Vec<[f64; 2]> = points.iter().map(|p| uv(*p)).collect();
    for (axis, [low, high, middle]) in [(0, [3, 5, 1]), (1, [4, 6, 2])] {
        let first = uvs[0][axis];
        if uvs.iter().all(|p| (p[axis] - first).abs() <= tol) {
            return if (first - domain[axis][0]).abs() <= tol {
                low
            } else if (first - domain[axis][1]).abs() <= tol {
                high
            } else {
                middle
            };
        }
    }
    0
}

/// A face's surface as a NURBS surface, with its parameter domain.
fn face_surface(
    s: &RecordSurface,
    domain: [[f64; 2]; 2],
) -> Result<(NurbsSurface, [[f64; 2]; 2]), String> {
    Ok(match s {
        RecordSurface::Plane {
            origin,
            u_axis,
            v_axis,
        } => plane_patch(*origin, *u_axis, *v_axis, domain),
        RecordSurface::PlaneThrough {
            origin,
            u_point,
            v_point,
        } => {
            let along = |p: &P3| std::array::from_fn(|k| p[k] - origin[k]);
            plane_patch(*origin, along(u_point), along(v_point), domain)
        }
        RecordSurface::Spline {
            u_knots,
            v_knots,
            points,
        } => {
            let net: Vec<Vec<[f64; 4]>> = points
                .iter()
                .map(|row| row.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect())
                .collect();
            surface_of(u_knots, v_knots, net)
        }
        RecordSurface::RationalSpline {
            u_knots,
            v_knots,
            homogeneous,
        } => surface_of(u_knots, v_knots, homogeneous.clone()),
        _ => return Err("a face of a kind without its NURBS".to_owned()),
    })
}

fn surface_of(u: &[f64], v: &[f64], points: Vec<Vec<[f64; 4]>>) -> (NurbsSurface, [[f64; 2]; 2]) {
    let order = [
        u.len() - points.len(),
        v.len() - points.first().map_or(0, Vec::len),
    ];
    let (du, dv) = (order[0] - 1, order[1] - 1);
    let domain = [[u[du], u[u.len() - 1 - du]], [v[dv], v[v.len() - 1 - dv]]];
    let rational = points.iter().any(|row| rational(row));
    (
        NurbsSurface {
            order,
            knots: [inner(u), inner(v)],
            points,
            rational,
        },
        domain,
    )
}

/// The rectangle `domain` of the plane `origin + u a + v b`, as a bilinear NURBS patch on the
/// same parameters.
fn plane_patch(origin: P3, a: P3, b: P3, domain: [[f64; 2]; 2]) -> (NurbsSurface, [[f64; 2]; 2]) {
    let at = |u: f64, v: f64| -> [f64; 4] {
        let p: P3 = std::array::from_fn(|k| origin[k] + u * a[k] + v * b[k]);
        [p[0], p[1], p[2], 1.0]
    };
    let [[u0, u1], [v0, v1]] = domain;
    (
        NurbsSurface {
            order: [2, 2],
            knots: [vec![u0, u1], vec![v0, v1]],
            points: vec![vec![at(u0, v0), at(u0, v1)], vec![at(u1, v0), at(u1, v1)]],
            rational: false,
        },
        domain,
    )
}

/// The engine's B-rep record as openNURBS' B-rep.
fn brep(r: &Record) -> Result<Brep, String> {
    let index = |i: usize| i32::try_from(i).map_err(|_| "too large".to_owned());
    let mut out = Brep::default();

    // Which way each face's normal goes, from the shells that use it.
    let mut forward = vec![true; r.faces.len()];
    for shell in &r.shells {
        if let Shell::Faces { faces, .. } = shell {
            for f in faces {
                if let Some(slot) = forward.get_mut(f.face as usize) {
                    *slot = f.forward;
                }
            }
        }
    }
    // The faces that use each edge, once per use.
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); r.edges.len()];
    for (fi, f) in r.faces.iter().enumerate() {
        for l in &f.loops {
            if let Loop::Edges { edges } = l {
                for e in edges {
                    if let Some(list) = users.get_mut(e.edge as usize) {
                        list.push(fi);
                    }
                }
            }
        }
    }

    for v in &r.vertices {
        out.vertices.push(BrepVertex {
            point: v.point,
            edges: Vec::new(),
            tolerance: v.tolerance,
        });
    }
    for (ei, e) in r.edges.iter().enumerate() {
        out.curves3
            .push(Some(edge_curve(e.parameters(), e.interval)?));
        let [a, b] = e.vertices;
        out.edges.push(BrepEdge {
            curve: index(ei)?,
            reversed: false,
            proxy_domain: e.interval,
            domain: e.interval,
            vertices: [a as i32, b as i32],
            trims: Vec::new(),
            tolerance: e.tolerance,
        });
        for v in [a, b] {
            if let Some(vertex) = out.vertices.get_mut(v as usize) {
                vertex.edges.push(index(ei)?);
            }
        }
    }

    for (fi, f) in r.faces.iter().enumerate() {
        let (surface, domain) = face_surface(f.parameters(), f.domain)?;
        out.surfaces.push(Some(model::Surface::Nurbs(surface)));
        let face_index = index(fi)?;
        let mut loops = Vec::new();
        for (li, l) in f.loops.iter().enumerate() {
            let Loop::Edges { edges } = l else {
                // A loop of one point (a pole or a collapsed boundary): Rhino has none.
                continue;
            };
            let loop_index = index(out.loops.len())?;
            // Each use as a trim running the loop's way, with its ends in (u, v).
            let mut made: Vec<(BrepTrim, Vec<f64>, Vec<[f64; 4]>)> = Vec::new();
            for e in edges {
                let edge = &r.edges[e.edge as usize];
                let (mut knots, mut points) = trim_points(&e.pcurve);
                if !e.forward {
                    (knots, points) = reversed(&knots, &points);
                }
                let span = [knots[0], knots[knots.len() - 1]];
                let [a, b] = edge.vertices;
                let uses = &users[e.edge as usize];
                let kind = if uses.len() == 1 {
                    trim_kind::BOUNDARY
                } else if uses.iter().filter(|u| **u == fi).count() > 1 {
                    trim_kind::SEAM
                } else {
                    trim_kind::MATED
                };
                made.push((
                    BrepTrim {
                        curve: 0,
                        proxy_domain: span,
                        domain: span,
                        proxy_reversed: false,
                        edge: e.edge as i32,
                        vertices: if e.forward {
                            [a as i32, b as i32]
                        } else {
                            [b as i32, a as i32]
                        },
                        reversed3d: !e.forward,
                        kind,
                        iso: iso(&points, domain),
                        loop_index,
                        tolerance: [0.0, 0.0],
                    },
                    knots,
                    points,
                ));
            }
            // Where one trim ends away from where the next starts, the loop passes through a
            // pole: a singular trim, with no edge, joins them.
            let size = (domain[0][1] - domain[0][0])
                .abs()
                .max((domain[1][1] - domain[1][0]).abs());
            let mut trims = Vec::new();
            let n = made.len();
            for i in 0..n {
                trims.push(made[i].clone());
                let end = uv(made[i].2[made[i].2.len() - 1]);
                let next = uv(made[(i + 1) % n].2[0]);
                if (end[0] - next[0]).hypot(end[1] - next[1]) > 1e-8 * size.max(1.0) {
                    let points = vec![[end[0], end[1], 0.0, 1.0], [next[0], next[1], 0.0, 1.0]];
                    let vertex = made[i].0.vertices[1];
                    trims.push((
                        BrepTrim {
                            curve: 0,
                            proxy_domain: [0.0, 1.0],
                            domain: [0.0, 1.0],
                            proxy_reversed: false,
                            edge: -1,
                            vertices: [vertex, vertex],
                            reversed3d: false,
                            kind: trim_kind::SINGULAR,
                            iso: iso(&points, domain),
                            loop_index,
                            tolerance: [0.0, 0.0],
                        },
                        vec![0.0, 0.0, 1.0, 1.0],
                        points,
                    ));
                }
            }
            let mut trim_ids = Vec::new();
            for (mut t, knots, points) in trims {
                t.curve = index(out.curves2.len())?;
                out.curves2
                    .push(Some(model::Curve::Nurbs(nurbs_of(&knots, points, 2))));
                let ti = index(out.trims.len())?;
                if let Some(edge) = usize::try_from(t.edge)
                    .ok()
                    .and_then(|e| out.edges.get_mut(e))
                {
                    edge.trims.push(ti);
                }
                out.trims.push(t);
                trim_ids.push(ti);
            }
            loops.push(loop_index);
            out.loops.push(BrepLoop {
                trims: trim_ids,
                kind: if li == 0 { 1 } else { 2 },
                face: face_index,
            });
        }
        out.faces.push(BrepFace {
            loops,
            surface: face_index,
            reversed: !forward[fi],
        });
    }
    Ok(out)
}

/// The document as a file's model: [`export`]'s objects, with its named views and construction
/// planes.
#[must_use]
pub fn export_document(doc: &curva_document::Document, only: Option<&[u64]>) -> (Model, Report) {
    let (mut model, report) = export(doc.state(), only);
    model.views = doc.views().iter().map(view_of).collect();
    model.cplanes = doc
        .cplanes()
        .iter()
        .map(|c| model::NamedCPlane {
            name: c.name.clone(),
            plane: Plane {
                origin: c.origin,
                x: c.x,
                y: c.y,
                z: cross(c.x, c.y),
            },
        })
        .collect();
    (model, report)
}

fn unit3(a: P3) -> P3 {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
    a.map(|v| v / length)
}

/// A Curva saved view as a file's: the camera Curva's viewport would have, with a frustum that
/// shows as much at the target.
fn view_of(v: &curva_document::SavedView) -> model::View {
    let (forward, up, eye) = match v.axes {
        Some([forward, _, up]) => (
            forward,
            up,
            std::array::from_fn(|k| v.target[k] - forward[k] * v.distance),
        ),
        None => {
            let (cp, sp) = (v.pitch.cos(), v.pitch.sin());
            let eye: P3 = [
                v.target[0] + v.distance * cp * v.yaw.cos(),
                v.target[1] + v.distance * cp * v.yaw.sin(),
                v.target[2] + v.distance * sp,
            ];
            let forward = unit3(std::array::from_fn(|k| v.target[k] - eye[k]));
            let right = unit3(cross(forward, [0.0, 0.0, 1.0]));
            (forward, cross(right, forward), eye)
        }
    };
    let (near, far) = (0.01 * v.distance, 100.0 * v.distance);
    let half = crate::import::HALF_FIELD.tan() * if v.parallel { v.distance } else { near };
    model::View {
        name: v.name.clone(),
        location: eye,
        direction: forward,
        up,
        target: v.target,
        parallel: v.parallel,
        frustum: [-half, half, -half, half, near, far],
    }
}
