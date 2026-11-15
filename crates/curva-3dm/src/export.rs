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
        document_text: state
            .document_text
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        notes: state.notes.clone(),
        ..Model::default()
    };

    // Layers: each layer's full name (its sublayer tree, spec `sublayers.md` §3, or an older
    // "Parent::Child" name) becomes Rhino's nested layers, parents made as needed.
    let mut layer_index: BTreeMap<u64, usize> = BTreeMap::new();
    let mut by_path: BTreeMap<String, usize> = BTreeMap::new();
    for layer in &state.layers {
        let mut parent = Uuid::NIL;
        let full = state.layer_path(layer.id);
        let parts: Vec<&str> = full.split("::").collect();
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
                        linetype: -1,
                    });
                    by_path.insert(path, model.layers.len() - 1);
                    model.layers.len() - 1
                }
            };
            if last {
                let linetype = linetype_in(&mut model, &mut ids, state, layer.linetype);
                let l = &mut model.layers[index];
                l.linetype = linetype;
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
            linetype: -1,
        });
    }

    // Each layout's page id, which objects on it and fields naming it carry.
    let page_ids: BTreeMap<u64, Uuid> = state.layouts.iter().map(|l| (l.id, ids.next())).collect();
    // Scale bars and tables, which Rhino has not, go out as the plain drawing they make: lines,
    // solid hatches for their filled blocks, and notes, grouped.
    let fields = curva_document::fields::FieldContext::now(None);
    let mut expanded: Vec<Object> = Vec::new();
    let mut next_group = state.next_group;
    for o in state
        .objects
        .iter()
        .filter(|o| only.is_none_or(|ids| ids.contains(&o.id)))
    {
        match plain_parts(o, state, next_group, expanded.len()) {
            Some(parts) => {
                next_group += 1;
                expanded.extend(parts);
            }
            None => expanded.push(with_rhino_fields(o, state, &fields, &page_ids)),
        }
    }
    let chosen: Vec<&Object> = expanded.iter().collect();

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

    // Annotation styles: each Curva style the annotation uses, under its own name with its font,
    // sizes (in the model's unit) and decimals. A note of another height or alignment than its
    // style's gets a style of its own beside it ("ISO 3.5 7 Centre"), so no note needs to
    // override its style.
    let mut style_ids: StyleIds = BTreeMap::new();
    let wanted: Vec<StyleKey> = chosen
        .iter()
        .map(|o| &o.shape)
        .chain(
            state
                .blocks
                .iter()
                .flat_map(|b| b.objects.iter().map(|o| &o.shape)),
        )
        .filter_map(|s| match s {
            Shape::Annotation(curva_document::annotation::Annotation::Text(t)) => Some(note_key(t)),
            Shape::Annotation(
                a @ (curva_document::annotation::Annotation::Dimension(_)
                | curva_document::annotation::Annotation::Radial(_)
                | curva_document::annotation::Annotation::Angle(_)
                | curva_document::annotation::Annotation::Leader(_)),
            ) => Some(dimension_key(state, a.style())),
            _ => None,
        })
        .collect();
    for key in wanted {
        if style_ids.contains_key(&key) {
            continue;
        }
        let (style_id, height_bits, align) = key;
        let height = f64::from_bits(height_bits);
        let style = annotation_style(state, style_id);
        let scale = style.scale_to(state.unit, None);
        let own = (height - style.height_in(state.unit)).abs() < 1e-9 && align == 0;
        let id = ids.next();
        style_ids.insert(key, id);
        model.dim_styles.push(model::DimStyle {
            id,
            name: if own {
                style.name.clone()
            } else {
                format!(
                    "{} {} {}",
                    style.name,
                    format!("{height:.4}")
                        .trim_end_matches('0')
                        .trim_end_matches('.'),
                    ["Left", "Centre", "Right"][align as usize]
                )
            },
            text_height: height,
            h_align: align,
            v_align: 3,
            arrow_size: style.arrow_size() * scale,
            extension_extension: style.extension_extension() * scale,
            extension_offset: style.extension_offset() * scale,
            text_gap: style.text_gap() * scale,
            length_resolution: i32::try_from(style.linear_resolution()).unwrap_or(2),
            font: style.font.clone(),
        });
    }

    // Hatch patterns: those the hatches use, in Rhino's model units at scale one.
    let mut pattern_index: BTreeMap<String, i32> = BTreeMap::new();
    let per_mm = 1.0 / state.unit.millimetres();
    let all_patterns = state.patterns();
    let hatch_patterns = chosen
        .iter()
        .map(|o| &o.shape)
        .chain(
            state
                .blocks
                .iter()
                .flat_map(|b| b.objects.iter().map(|o| &o.shape)),
        )
        .filter_map(|s| match s {
            Shape::Annotation(curva_document::annotation::Annotation::Hatch(h)) => {
                Some(h.pattern.to_lowercase())
            }
            _ => None,
        });
    for name in hatch_patterns {
        if pattern_index.contains_key(&name) {
            continue;
        }
        let Some(own) = all_patterns.iter().find(|p| p.name.to_lowercase() == name) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        pattern_index.insert(name, model.hatch_patterns.len() as i32);
        model.hatch_patterns.push(model::HatchPattern {
            id: ids.next(),
            name: own.name.clone(),
            description: own.description.clone(),
            solid: own.is_solid(),
            lines: own
                .lines
                .iter()
                .map(|l| model::HatchLine {
                    angle: l.angle.to_radians(),
                    base: [l.origin[0] * per_mm, l.origin[1] * per_mm],
                    offset: [l.shift * per_mm, l.spacing * per_mm],
                    dashes: l.dashes.iter().map(|d| d * per_mm).collect(),
                })
                .collect(),
        });
    }

    // Layouts: Rhino's pages, in millimetres.
    for (index, layout) in state.layouts.iter().enumerate() {
        let mm = layout.unit.millimetres();
        model.pages.push(model::Page {
            id: page_ids[&layout.id],
            name: if layout.name.is_empty() {
                format!("Layout {}", index + 1)
            } else {
                layout.name.clone()
            },
            size_mm: [layout.size[0] * mm, layout.size[1] * mm],
            user_text: layout
                .user_text
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        });
    }
    if !state.layouts.is_empty() {
        model.page_unit = Some(2);
    }

    let mut groups: BTreeMap<u64, usize> = BTreeMap::new();
    let mut add = |o: &Object, in_block: bool, model: &mut Model, report: &mut Report| {
        let sheet = o
            .space
            .and_then(|id| state.layouts.iter().find(|l| l.id == id));
        let made = match sheet {
            // On a layout: drawn in millimetres on its page, a detail's frame as the detail.
            Some(layout) => {
                sheet_geometry(o, layout, &block_ids, &pattern_index, &style_ids, state)
            }
            None => geometry_of(&o.shape, &block_ids, &pattern_index, &style_ids, state),
        };
        let geometry = match made {
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
        let (linetype, linetype_source) = if o.linetype.is_by_layer() {
            (-1, Source::Layer)
        } else {
            (
                linetype_in(model, &mut ids, state, o.linetype),
                Source::Object,
            )
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
                page_space: sheet.is_some(),
                viewport: sheet.map_or(Uuid::NIL, |l| page_ids[&l.id]),
                user_text: o
                    .user_text
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                linetype,
                linetype_source,
                color: o.colour.map_or([0, 0, 0], |c| {
                    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
                }),
                color_source: if o.colour.is_some() {
                    Source::Object
                } else {
                    Source::Layer
                },
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

/// The linetype table's index for a linetype (Curva's own or the document's), adding it to the
/// table the first time; -1 for a continuous line, which Rhino leaves unnamed.
fn linetype_in(
    model: &mut Model,
    ids: &mut Ids,
    state: &State,
    linetype: curva_document::Linetype,
) -> i32 {
    let pattern = state.linetype_pattern(linetype);
    if linetype.is_by_layer() || pattern.is_empty() {
        return -1;
    }
    let name = state.linetype_name(linetype);
    let index = match model.linetypes.iter().position(|l| l.name == name) {
        Some(i) => i,
        None => {
            model.linetypes.push(model::Linetype {
                id: ids.next(),
                name,
                segments: pattern
                    .iter()
                    .enumerate()
                    .map(|(k, length)| (*length, k % 2 == 0))
                    .collect(),
            });
            model.linetypes.len() - 1
        }
    };
    i32::try_from(index).unwrap_or(-1)
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

fn geometry_of(
    shape: &Shape,
    blocks: &BTreeMap<u64, Uuid>,
    patterns: &BTreeMap<String, i32>,
    styles: &StyleIds,
    state: &State,
) -> Result<Geometry, String> {
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
        Shape::Annotation(curva_document::annotation::Annotation::Dot(dot)) => Geometry::Dot {
            at: dot.at,
            text: dot.content.clone(),
            secondary: dot.secondary.clone(),
            #[allow(clippy::cast_possible_truncation)]
            height: dot.height.round() as i32,
            font: dot.font.clone().unwrap_or_default(),
        },
        Shape::Annotation(curva_document::annotation::Annotation::Hatch(h)) => {
            Geometry::Hatch(hatch(h, patterns)?)
        }
        Shape::Annotation(curva_document::annotation::Annotation::Text(t)) => {
            let style = *styles.get(&note_key(t)).ok_or("a note without its style")?;
            // Rhino aligns these notes by the top of their first line, one text height above
            // the baseline Curva places them by.
            let origin: P3 = std::array::from_fn(|k| t.origin[k] + t.y[k] * t.height);
            Geometry::Text(model::Text {
                rich: t.content.clone(),
                plane: Plane {
                    origin,
                    x: t.x,
                    y: t.y,
                    z: cross(t.x, t.y),
                },
                style,
                own: None,
                rotation: 0.0,
            })
        }
        Shape::Annotation(curva_document::annotation::Annotation::Dimension(d)) => {
            Geometry::Dimension(linear_dimension(d, styles, state)?)
        }
        Shape::Annotation(curva_document::annotation::Annotation::Radial(d)) => {
            let style = *styles
                .get(&dimension_key(state, d.style))
                .ok_or("a dimension without its style")?;
            let plane = plane_at(d.centre, d.x, d.y);
            Geometry::Radial(model::RadialDimension {
                radius_point: local(&plane, d.on_curve),
                text_point: local(&plane, d.text_at),
                plane,
                style,
                own: None,
                diameter: d.diameter,
                text: d.text.clone().unwrap_or_else(|| "<>".to_owned()),
            })
        }
        Shape::Annotation(curva_document::annotation::Annotation::Angle(d)) => {
            let style = *styles
                .get(&dimension_key(state, d.style))
                .ok_or("a dimension without its style")?;
            let plane = plane_at(d.apex, d.x, d.y);
            let side = |p: P3| {
                let q = local(&plane, p);
                let length = (q[0] * q[0] + q[1] * q[1]).sqrt();
                ([q[0] / length.max(1e-12), q[1] / length.max(1e-12)], length)
            };
            let (first, near) = side(d.first);
            let (second, far) = side(d.second);
            Geometry::Angular(model::AngularDimension {
                arc_point: local(&plane, d.arc_at),
                plane,
                style,
                own: None,
                text: d.text.clone().unwrap_or_else(|| "<>".to_owned()),
                sides: [first, second],
                offsets: [near, far],
            })
        }
        Shape::Annotation(curva_document::annotation::Annotation::Leader(l)) => {
            let style = *styles
                .get(&dimension_key(state, l.style))
                .ok_or("a leader without its style")?;
            let start = *l.points.first().ok_or("a leader without points")?;
            let plane = plane_at(start, l.x, l.y);
            Geometry::Leader(model::Leader {
                points: l.points.iter().map(|p| local(&plane, *p)).collect(),
                plane,
                style,
                own: None,
                rich: l.content.clone(),
            })
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

/// A hatch as Rhino holds one: its plane at the hatch's base point, its outlines as 2D curves
/// in that plane, and its pattern by its place in the file's table.
fn hatch(
    h: &curva_document::annotation::Hatch,
    patterns: &BTreeMap<String, i32>,
) -> Result<model::Hatch, String> {
    let z = cross(h.x, h.y);
    let o = h.base;
    // From the world into the plane: each axis's coordinate, measured from the base point.
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let into: [[f64; 4]; 4] = [
        [h.x[0], h.y[0], z[0], 0.0],
        [h.x[1], h.y[1], z[1], 0.0],
        [h.x[2], h.y[2], z[2], 0.0],
        [-dot(o, h.x), -dot(o, h.y), -dot(o, z), 1.0],
    ];
    let loops = h
        .loops
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let flat = c.mapped_affine(into).map_err(|e| e.0)?;
            Ok((i == 0, flat2(curve(&flat)?)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let pattern = *patterns
        .get(&h.pattern.to_lowercase())
        .ok_or("a hatch pattern Curva does not have")?;
    Ok(model::Hatch {
        plane: Plane {
            origin: o,
            x: h.x,
            y: h.y,
            z,
        },
        scale: h.scale,
        rotation: h.rotation,
        pattern,
        loops,
        base: [0.0; 2],
    })
}

/// A curve in a plane's coordinates written as a 2D one, as hatch outlines are.
fn flat2(c: model::Curve) -> model::Curve {
    match c {
        model::Curve::Nurbs(mut n) => {
            n.dimension = 2;
            model::Curve::Nurbs(n)
        }
        model::Curve::Poly { segments, params } => model::Curve::Poly {
            segments: segments.into_iter().map(flat2).collect(),
            params,
        },
        other => other,
    }
}

/// A note's horizontal alignment as Rhino 5 numbered it: 0 left, 1 centre, 2 right.
fn align_code(align: curva_document::annotation::TextAlign) -> u32 {
    match align {
        curva_document::annotation::TextAlign::Left => 0,
        curva_document::annotation::TextAlign::Centre => 1,
        curva_document::annotation::TextAlign::Right => 2,
    }
}

/// The annotation style with this id: the document's own, or the standard ones for its unit;
/// the first when there is none of that id.
fn annotation_style(state: &State, style: u64) -> curva_document::annotation::AnnotationStyle {
    let styles = if state.annotation_styles.is_empty() {
        curva_document::annotation::standard_styles(matches!(state.unit, Unit::Feet | Unit::Inches))
    } else {
        state.annotation_styles.clone()
    };
    styles
        .iter()
        .find(|s| s.id == style)
        .or_else(|| styles.first())
        .cloned()
        .unwrap_or_else(|| curva_document::annotation::standard_styles(false).remove(0))
}

/// The Rhino style a note or dimension takes: by Curva style, text height (in the model's
/// unit, as bits) and alignment code.
type StyleKey = (u64, u64, u32);
/// The Rhino styles written, by [`StyleKey`].
type StyleIds = BTreeMap<StyleKey, Uuid>;

/// A note's Rhino style: its Curva style with its own height and alignment.
fn note_key(t: &curva_document::annotation::Text) -> StyleKey {
    (t.style, t.height.to_bits(), align_code(t.align))
}

/// A dimension's or leader's Rhino style: its Curva style as it is.
fn dimension_key(state: &State, style: u64) -> StyleKey {
    let held = annotation_style(state, style);
    (held.id, held.height_in(state.unit).to_bits(), 0)
}

/// A linear dimension as Rhino holds one: its plane at the first point, its X the direction it
/// measures along (rotated) or from point to point (aligned), the other points in that plane.
fn linear_dimension(
    d: &curva_document::annotation::Dimension,
    styles: &StyleIds,
    state: &State,
) -> Result<model::LinearDimension, String> {
    use curva_document::annotation::DimensionKind;
    let normal = cross(d.x, d.y);
    let unit = |v: P3| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (l > 1e-12).then(|| v.map(|c| c / l))
    };
    let (along, rotated) = match d.kind {
        DimensionKind::Horizontal => (d.x, true),
        DimensionKind::Vertical => (d.y, true),
        DimensionKind::Rotated(a) => (
            std::array::from_fn(|k| d.x[k] * a.cos() + d.y[k] * a.sin()),
            true,
        ),
        DimensionKind::Aligned => {
            let v: P3 = std::array::from_fn(|k| d.second[k] - d.first[k]);
            let off: f64 = (0..3).map(|k| v[k] * normal[k]).sum();
            let flat: P3 = std::array::from_fn(|k| v[k] - normal[k] * off);
            (unit(flat).ok_or("a dimension between one point")?, false)
        }
    };
    let up = cross(normal, along);
    let local = |p: P3| {
        let v: P3 = std::array::from_fn(|k| p[k] - d.first[k]);
        [
            (0..3).map(|k| v[k] * along[k]).sum::<f64>(),
            (0..3).map(|k| v[k] * up[k]).sum::<f64>(),
        ]
    };
    let style = *styles
        .get(&dimension_key(state, d.style))
        .ok_or("a dimension without its style")?;
    Ok(model::LinearDimension {
        plane: Plane {
            origin: d.first,
            x: along,
            y: up,
            z: normal,
        },
        style,
        own: None,
        rotated,
        text: d.text.clone().unwrap_or_else(|| "<>".to_owned()),
        second: local(d.second),
        line: local(d.line),
    })
}

/// A plane at `origin` with the directions `x` and `y`.
fn plane_at(origin: P3, x: P3, y: P3) -> Plane {
    Plane {
        origin,
        x,
        y,
        z: cross(x, y),
    }
}

/// A point in a plane's coordinates.
fn local(plane: &Plane, p: P3) -> [f64; 2] {
    let v: P3 = std::array::from_fn(|k| p[k] - plane.origin[k]);
    [
        (0..3).map(|k| v[k] * plane.x[k]).sum(),
        (0..3).map(|k| v[k] * plane.y[k]).sum(),
    ]
}

/// An object on a layout, as Rhino keeps it: in millimetres on its page; a detail's frame as the
/// detail itself, with its camera and scale.
fn sheet_geometry(
    o: &Object,
    layout: &curva_document::layouts::Layout,
    blocks: &BTreeMap<u64, Uuid>,
    patterns: &BTreeMap<String, i32>,
    styles: &StyleIds,
    state: &State,
) -> Result<Geometry, String> {
    let mm = layout.unit.millimetres();
    let to_mm = [
        [mm, 0.0, 0.0, 0.0],
        [0.0, mm, 0.0, 0.0],
        [0.0, 0.0, mm, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // A millimetre sheet is drawn as it is, so its lines stay lines.
    let shape = if (mm - 1.0).abs() < 1e-12 {
        o.shape.clone()
    } else {
        curva_document::blocks::shape_placed(&o.shape, &to_mm).ok_or("scaling failed")?
    };
    let Some(detail) = layout.details.iter().find(|d| d.frame == o.id) else {
        return geometry_of(&shape, blocks, patterns, styles, state);
    };
    let Shape::Curve(frame) = &shape else {
        return Err("a detail without its frame".to_owned());
    };
    let boundary = outline(frame)?;
    let (low, high) = shape.bounds().ok_or("a detail without a size")?;
    let model_mm = state.unit.millimetres();
    Ok(Geometry::Detail(model::Detail {
        view: detail_camera(detail, [high[0] - low[0], high[1] - low[1]], model_mm),
        boundary,
        ratio: detail.scale.map_or(0.0, |s| 1.0 / s),
    }))
}

/// The camera a detail looks through, in model units: its standard view's direction at its
/// target, the frustum the frame's size covers at its scale (or a 50 mm lens for a perspective).
fn detail_camera(
    detail: &curva_document::layouts::Detail,
    frame_mm: [f64; 2],
    model_mm: f64,
) -> model::View {
    let (direction, up): (P3, P3) = match detail.projection.as_str() {
        "Top" => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        "Front" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        "Right" => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        _ => {
            let s = 1.0 / 3f64.sqrt();
            ([-s, s, -s], [0.0, 0.0, 1.0])
        }
    };
    let up = {
        let along: f64 = (0..3).map(|k| up[k] * direction[k]).sum();
        let v: P3 = std::array::from_fn(|k| up[k] - direction[k] * along);
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
        v.map(|c| c / l)
    };
    let target = detail.target;
    match detail.scale {
        Some(scale) => {
            let [w, h] = frame_mm.map(|v| v / scale / model_mm);
            let depth = 1000.0 * w.max(h).max(1.0);
            model::View {
                name: String::new(),
                location: std::array::from_fn(|k| target[k] - direction[k] * depth),
                direction,
                up,
                target,
                parallel: true,
                frustum: [
                    -w / 2.0,
                    w / 2.0,
                    -h / 2.0,
                    h / 2.0,
                    depth * 1e-3,
                    depth * 2.0,
                ],
            }
        }
        None => {
            let distance = detail.distance.max(1e-6);
            let near = distance / 100.0;
            // A 50 mm lens on 36 mm film across the frame's longer side.
            let half = near * 18.0 / 50.0;
            let aspect = frame_mm[1] / frame_mm[0].max(1e-12);
            model::View {
                name: String::new(),
                location: std::array::from_fn(|k| target[k] - direction[k] * distance),
                direction,
                up,
                target,
                parallel: false,
                frustum: [
                    -half,
                    half,
                    -half * aspect,
                    half * aspect,
                    near,
                    distance * 100.0,
                ],
            }
        }
    }
}

/// A detail's outline as Rhino keeps it: a 2D NURBS curve, of degree one for a polyline
/// (openNURBS' knot vector, without the end knots NURBS books add).
fn outline(frame: &Curve) -> Result<model::NurbsCurve, String> {
    if let Curve::Polyline { points, closed } = frame {
        let mut corners: Vec<[f64; 4]> = points.iter().map(|p| [p[0], p[1], 0.0, 1.0]).collect();
        if *closed && let Some(first) = corners.first().copied() {
            corners.push(first);
        }
        #[allow(clippy::cast_precision_loss)]
        let knots = (0..corners.len()).map(|i| i as f64).collect();
        return Ok(model::NurbsCurve {
            dimension: 2,
            order: 2,
            knots,
            points: corners,
            rational: false,
        });
    }
    let nurbs = frame.to_nurbs().map_err(|e| e.0)?;
    let model::Curve::Nurbs(mut boundary) = curve(&nurbs)? else {
        return Err("a detail frame that is not a NURBS curve".to_owned());
    };
    boundary.dimension = 2;
    Ok(boundary)
}

/// A scale bar or table as the plain objects it draws, for a file that has neither: its lines,
/// its filled blocks as solid hatches, and its words as notes, on its layer and sheet and in a
/// group of their own. `None` for any other object.
fn plain_parts(o: &Object, state: &State, group: u64, before: usize) -> Option<Vec<Object>> {
    use curva_document::annotation::{Annotation, Hatch};
    let (parts, place) = match &o.shape {
        Shape::Annotation(Annotation::ScaleBar(bar)) => (
            curva_document::sheet::scale_bar_parts(bar, state, o.space),
            &bar.at,
        ),
        Shape::Annotation(Annotation::Table(table)) => {
            (curva_document::sheet::table_parts(table, state), &table.at)
        }
        _ => return None,
    };
    let shapes = parts
        .lines
        .into_iter()
        .map(Shape::Curve)
        .chain(parts.fills.into_iter().map(|loops| {
            Shape::Annotation(Annotation::Hatch(Hatch {
                loops,
                pattern: "Solid".to_owned(),
                scale: 1.0,
                rotation: 0.0,
                base: place.origin,
                x: place.x,
                y: place.y,
            }))
        }))
        .chain(
            parts
                .texts
                .into_iter()
                .map(|t| Shape::Annotation(Annotation::Text(t))),
        );
    Some(
        shapes
            .enumerate()
            .map(|(i, shape)| Object {
                // Ids of their own, far from the model's, as these objects exist only in the file.
                id: u64::MAX - (before + i) as u64,
                shape,
                group: Some(group),
                ..o.clone()
            })
            .collect(),
    )
}

/// The fields Rhino works out itself, by the same names (Rhino 8's `information/text_fields.htm`).
/// Others read what only Curva holds (objects by Curva's numbers), so they go out worked out.
/// Document text, layout user text and notes travel in the file, so Rhino reads them itself.
const RHINO_FIELDS: [&str; 12] = [
    "DocumentText",
    "LayoutUserText",
    "Notes",
    "PageName",
    "PageNumber",
    "NumPages",
    "PageWidth",
    "PageHeight",
    "PaperName",
    "Date",
    "FileName",
    "ModelUnits",
];

/// Words with their fields as Rhino will show them: its own fields kept, the rest worked out.
fn rhino_words(
    words: &str,
    state: &State,
    space: Option<u64>,
    fields: &curva_document::fields::FieldContext,
    page_ids: &BTreeMap<u64, Uuid>,
) -> String {
    let mut out = String::new();
    let mut rest = words;
    while let Some(start) = rest.find("%<") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(">%") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let field = &rest[start..start + end + 2];
        let name: String = field[2..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if name == "LayoutUserText" {
            match layout_by_page_id(field, page_ids) {
                Some(rhinos) => out.push_str(&rhinos),
                None => out.push_str(&curva_document::fields::resolve(
                    field, state, space, fields,
                )),
            }
        } else if RHINO_FIELDS.contains(&name.as_str()) {
            out.push_str(field);
        } else {
            out.push_str(&curva_document::fields::resolve(
                field, state, space, fields,
            ));
        }
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// The two parts of a field's brackets, `"a","b"`, as written; `None` for any other form.
pub(crate) fn field_args(field: &str) -> Option<Vec<String>> {
    let inner = field.get(field.find('(')? + 1..field.rfind(')')?)?;
    let mut args = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        args.push(part.strip_prefix('"')?.strip_suffix('"')?.to_owned());
    }
    Some(args)
}

/// A `LayoutUserText` field as Rhino reads it: with one part as it is, with two the layout's
/// number swapped for its page's id. `None` when it names no layout going out.
fn layout_by_page_id(field: &str, page_ids: &BTreeMap<u64, Uuid>) -> Option<String> {
    match field_args(field)?.as_slice() {
        [_] => Some(field.to_owned()),
        [layout, key] => {
            let id = page_ids.get(&layout.trim().parse().ok()?)?;
            Some(format!("%<LayoutUserText(\"{}\",\"{key}\")>%", id.text()))
        }
        _ => None,
    }
}

/// An object with its notes' and leaders' fields as Rhino will show them.
fn with_rhino_fields(
    o: &Object,
    state: &State,
    fields: &curva_document::fields::FieldContext,
    page_ids: &BTreeMap<u64, Uuid>,
) -> Object {
    use curva_document::annotation::Annotation;
    let mut out = o.clone();
    match &mut out.shape {
        Shape::Annotation(Annotation::Text(t)) => {
            t.content = rhino_words(&t.content, state, o.space, fields, page_ids);
        }
        Shape::Annotation(Annotation::Leader(l)) => {
            l.content = rhino_words(&l.content, state, o.space, fields, page_ids);
        }
        _ => {}
    }
    out
}
