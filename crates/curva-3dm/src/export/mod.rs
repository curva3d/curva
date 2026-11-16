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

mod annotation;
mod geometry;
mod sheets;
mod views;
use annotation::*;
#[allow(clippy::wildcard_imports)]
use geometry::*;
pub(crate) use sheets::*;
use views::*;

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

    // The model's standard views' ids, which clipping planes name the views they cut by.
    model.model_views = ["Top", "Front", "Right", "Perspective"]
        .iter()
        .map(|name| ((*name).to_owned(), ids.next()))
        .collect();
    // Each detail's viewport id, by its frame: layers name the details they are off in by it.
    let detail_ids: BTreeMap<u64, Uuid> = state
        .layouts
        .iter()
        .flat_map(|l| l.details.iter())
        .map(|d| (d.frame, ids.next()))
        .collect();

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
                        hidden_in: Vec::new(),
                        section: None,
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
                l.hidden_in = state
                    .layouts
                    .iter()
                    .flat_map(|s| s.details.iter())
                    .filter(|d| d.hidden_layers.contains(&layer.id))
                    .filter_map(|d| detail_ids.get(&d.frame).copied())
                    .collect();
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
            hidden_in: Vec::new(),
            section: None,
        });
    }

    // Each layout's page id, which objects on it and fields naming it carry.
    let page_ids: BTreeMap<u64, Uuid> = state.layouts.iter().map(|l| (l.id, ids.next())).collect();
    // Scale bars and tables, which Rhino has not, go out as the plain drawing they make: lines,
    // solid hatches for their filled blocks, and notes, grouped.
    let fields = curva_document::fields::FieldContext::now(None);
    // Each object's id, settled first so that fields measuring an object can name it.
    let mut object_ids: BTreeMap<u64, Uuid> = state
        .objects
        .iter()
        .filter(|o| only.is_none_or(|ids| ids.contains(&o.id)))
        .map(|o| (o.id, ids.next()))
        .collect();
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
            None => expanded.push(with_rhino_fields(o, state, &fields, &page_ids, &object_ids)),
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
    // style's overrides those in its style, as Rhino's notes do.
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
            Shape::Annotation(
                a @ (curva_document::annotation::Annotation::Text(_)
                | curva_document::annotation::Annotation::Dimension(_)
                | curva_document::annotation::Annotation::Radial(_)
                | curva_document::annotation::Annotation::Angle(_)
                | curva_document::annotation::Annotation::Ordinate(_)
                | curva_document::annotation::Annotation::Leader(_)),
            ) => Some(dimension_key(state, a.style())),
            _ => None,
        })
        .collect();
    for key in wanted {
        if style_ids.contains_key(&key) {
            continue;
        }
        let id = ids.next();
        style_ids.insert(key, id);
        let style = annotation_style(state, key.0);
        model.dim_styles.push(model::DimStyle {
            id,
            name: style.name.clone(),
            ..rhino_style(state, key.0, f64::from_bits(key.1), 0)
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
        })
        .chain(crate::sections::patterns_used(
            state,
            chosen.iter().copied(),
        ));
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

    // Section styles on layers, now that the patterns they name have their places.
    for layer in &state.layers {
        if let (Some(style), Some(&index)) = (&layer.section, layer_index.get(&layer.id)) {
            model.layers[index].section = Some(crate::sections::to_rhino(style, &pattern_index));
        }
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
            Some(layout) => sheet_geometry(
                o,
                layout,
                &block_ids,
                &pattern_index,
                &style_ids,
                state,
                &detail_ids,
            ),
            None => match state.clipping_planes.iter().find(|c| c.frame == o.id) {
                Some(clip) => clipping_plane(o, clip, &model.model_views, &detail_ids),
                None => geometry_of(&o.shape, &block_ids, &pattern_index, &style_ids, state),
            },
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
        // A detail is written under its viewport's id, as Rhino names details: layers turned off
        // in one name it by that.
        // Other objects keep the id settled for them, taken once (a scale bar's or table's
        // parts share its number).
        let id = match &geometry {
            Geometry::Detail(d) if d.id != Uuid::NIL => d.id,
            _ if in_block => ids.next(),
            _ => object_ids.remove(&o.id).unwrap_or_else(|| ids.next()),
        };
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
        // A clipping plane's style is its object's own, with the source left at its layer, as
        // Rhino 8 writes it; another object's own style says it comes from the object.
        let plane = state.clipping_planes.iter().find(|c| c.frame == o.id);
        let (section_source, section) = match (plane, &o.section) {
            (Some(c), _) => (
                crate::sections::FROM_LAYER,
                Some(curva_document::section::SectionStyle {
                    fill: c.section,
                    hatch: c.hatch.clone(),
                }),
            ),
            (None, Some(own)) => (crate::sections::FROM_OBJECT, Some(own.clone())),
            (None, None) => (crate::sections::FROM_LAYER, None),
        };
        let section = section.map(|s| crate::sections::to_rhino(&s, &pattern_index));
        model.objects.push(model::Object {
            geometry,
            attributes: Attributes {
                section_source,
                section,
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
            let key = dimension_key(state, t.style);
            let style = *styles.get(&key).ok_or("a note without its style")?;
            // A note of another height or alignment than its style's overrides them in it.
            let align = align_code(t.align);
            let own = ((t.height - f64::from_bits(key.1)).abs() > 1e-9 || align != 0)
                .then(|| rhino_style(state, key.0, t.height, align));
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
                own,
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
        Shape::Annotation(curva_document::annotation::Annotation::Ordinate(d)) => {
            let style = *styles
                .get(&dimension_key(state, d.style))
                .ok_or("a dimension without its style")?;
            let plane = plane_at(d.base, d.x, d.y);
            Geometry::Ordinate(model::OrdinateDimension {
                point: local(&plane, d.point),
                leader: local(&plane, d.leader),
                plane,
                style,
                own: None,
                text: d.text.clone().unwrap_or_else(|| "<>".to_owned()),
                direction: match d.axis {
                    curva_document::annotation::OrdinateAxis::X => 1,
                    curva_document::annotation::OrdinateAxis::Y => 2,
                },
                kinks: d.kinks.unwrap_or([model::UNSET_VALUE; 2]),
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
