//! A [`Model`] read from a file brought into a Curva document.

use crate::model::{self, Brep, Geometry, Model, P3};
use crate::notes::note_in;
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

mod annotation;
mod breps;
mod curves;
mod surfaces;
mod views;
use annotation::*;
pub(crate) use breps::*;
#[allow(clippy::wildcard_imports)]
use curves::*;
use surfaces::*;
use views::*;

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
    // The file's hatch patterns by index, which section styles name them by.
    let pattern_names: Vec<String> = model
        .hatch_patterns
        .iter()
        .map(|p| p.name.clone())
        .collect();
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

    // Document user text joins the model's (the file's value where both have a key); its notes
    // fill notes the model has not got.
    for (key, value) in &model.document_text {
        state.document_text.insert(key.clone(), value.clone());
    }
    if state.notes.is_empty() {
        state.notes.clone_from(&model.notes);
    }

    // Layers: the file's tree as sublayers (spec `sublayers.md` §3), parents first, each merged
    // with a layer of the same full name already there.
    // Linetypes: the file's table as Curva's own or the document's.
    let linetypes: Vec<Linetype> = model
        .linetypes
        .iter()
        .map(|l| linetype_in(state, l))
        .collect();
    let linetype_of = |index: i32| {
        usize::try_from(index)
            .ok()
            .and_then(|i| linetypes.get(i).copied())
            .unwrap_or(Linetype::Continuous)
    };
    // Annotation styles: each the file's annotation uses, as Curva's, by name.
    let used: Vec<model::DimStyle> = model
        .dim_styles
        .iter()
        .filter(|s| {
            model
                .objects
                .iter()
                .any(|o| style_named(&o.geometry) == Some(s.id))
        })
        .cloned()
        .collect();
    let style_ids = styles_in(state, &used, file_mm);
    let full_names = layer_names(&model.layers);
    let nil = crate::archive::Uuid::NIL;
    let mut placed: Vec<Option<u64>> = vec![None; model.layers.len()];
    // Each pass places the layers whose parent is placed; a tree n deep takes n passes.
    for _ in 0..=model.layers.len() {
        let mut placed_any = false;
        for (index, layer) in model.layers.iter().enumerate() {
            if placed[index].is_some() {
                continue;
            }
            let parent = if layer.parent == nil {
                None
            } else {
                match model.layers.iter().position(|l| l.id == layer.parent) {
                    // A parent the file does not have: the layer goes at the top.
                    None => None,
                    Some(p) => match placed[p] {
                        Some(id) => Some(id),
                        None => continue,
                    },
                }
            };
            let full = &full_names[index];
            let existing = state
                .layers
                .iter()
                .find(|l| state.layer_path(l.id) == *full)
                .map(|l| l.id);
            let id = existing.unwrap_or_else(|| {
                let id = state.next_layer;
                state.next_layer += 1;
                state.layers.push(Layer {
                    id,
                    name: layer.name.clone(),
                    colour: colour_of(layer.color),
                    visible: layer.visible,
                    locked: layer.locked,
                    print_width: (layer.print_width > 0.0)
                        .then(|| PrintWidth::clamped(layer.print_width)),
                    linetype: linetype_of(layer.linetype),
                    parent,
                    collapsed: false,
                    section: layer
                        .section
                        .as_ref()
                        .map(|s| crate::sections::from_rhino(s, &pattern_names)),
                });
                id
            });
            placed[index] = Some(id);
            placed_any = true;
        }
        if !placed_any {
            break;
        }
    }
    let fallback = state.current_layer;
    let layer_ids: Vec<u64> = placed.iter().map(|id| id.unwrap_or(fallback)).collect();
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

    // Layouts: one for each of the file's pages, in millimetres; what is drawn on a page comes
    // from the file's page unit.
    let page_mm = millimetres_per(model.page_unit.unwrap_or(model.unit)).unwrap_or(file_mm);
    let mut page_ids: BTreeMap<crate::archive::Uuid, u64> = BTreeMap::new();
    // Each detail by its viewport id: its layout and frame, for the layers turned off in it.
    let mut detail_by_viewport: BTreeMap<crate::archive::Uuid, (u64, u64)> = BTreeMap::new();
    // Clipping planes as they come in: their rectangle, viewports and whether they cut.
    let mut clips: Vec<(
        u64,
        Vec<crate::archive::Uuid>,
        bool,
        Option<crate::sections::SectionStyle>,
    )> = Vec::new();
    let mut object_numbers: BTreeMap<crate::archive::Uuid, u64> = BTreeMap::new();
    for page in &model.pages {
        let id = state.next_layout;
        state.next_layout += 1;
        page_ids.insert(page.id, id);
        state.layouts.push(curva_document::layouts::Layout {
            id,
            name: page.name.clone(),
            size: page.size_mm,
            unit: curva_document::layouts::PaperUnit::Millimetres,
            details: Vec::new(),
            user_text: page.user_text.iter().cloned().collect(),
            revisions: Vec::new(),
        });
    }

    for object in &model.objects {
        let a = &object.attributes;
        let space = if a.page_space {
            match page_ids.get(&a.viewport) {
                Some(layout) => Some(*layout),
                None => {
                    Report::count(
                        &mut report.skipped,
                        "objects on a layout the file does not list",
                    );
                    continue;
                }
            }
        } else {
            None
        };
        if let Geometry::Detail(d) = &object.geometry {
            match (space, detail_in(d, page_mm, scale)) {
                (Some(layout), Ok((frame, mut detail))) => {
                    let id = state.next_id;
                    state.next_id += 1;
                    detail.frame = id;
                    state.objects.push(std::sync::Arc::new(Object {
                        space: Some(layout),
                        id,
                        shape: Shape::Curve(frame),
                        layer: layer_of(a.layer).unwrap_or(state.current_layer),
                        hidden: false,
                        name: a.name.clone(),
                        recipe: None,
                        print_width: PrintWidth::ByLayer,
                        locked: false,
                        group: None,
                        live: None,
                        input_of: None,
                        colour: None,
                        linetype: Linetype::ByLayer,
                        user_text: BTreeMap::new(),
                        section: None,
                    }));
                    if let Some(sheet) = state.layouts.iter_mut().find(|l| l.id == layout) {
                        detail_by_viewport.insert(a.id, (layout, id));
                        sheet.details.push(detail);
                    }
                    Report::count(&mut report.brought, "details");
                }
                (_, Err(why)) => {
                    Report::count(&mut report.skipped, &format!("unreadable ({why})"));
                }
                (None, _) => Report::count(&mut report.skipped, "details outside a layout"),
            }
            continue;
        }
        let scale = if space.is_some() { page_mm } else { scale };
        let (shape, kind) = match shape_of(
            &object.geometry,
            &block_ids,
            &model.dim_styles,
            &model.hatch_patterns,
            state,
            file_mm,
        ) {
            Ok(made) => made,
            Err(why) => {
                Report::count(&mut report.skipped, &format!("unreadable ({why})"));
                continue;
            }
        };
        let mut shape = match scaled(shape, scale) {
            Ok(shape) => shape,
            Err(why) => {
                Report::count(&mut report.skipped, &format!("unreadable ({why})"));
                continue;
            }
        };
        // Rhino 5's leaders (all a Rhino 5 file has) end at their words, with no landing.
        if model.version < 60
            && let Shape::Annotation(curva_document::annotation::Annotation::Leader(l)) = &mut shape
        {
            l.landing = false;
        }
        let shape = with_layout_numbers(shape, &page_ids);
        let shape = match style_named(&object.geometry).and_then(|s| style_ids.get(&s)) {
            Some(style) => with_style(shape, *style),
            None => shape,
        };
        let id = state.next_id;
        state.next_id += 1;
        let made = Object {
            space,
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
            colour: (a.color_source == model::Source::Object)
                .then(|| a.color.map(|c| f32::from(c) / 255.0)),
            linetype: if a.linetype_source == model::Source::Object {
                linetype_of(a.linetype)
            } else {
                Linetype::ByLayer
            },
            user_text: a.user_text.iter().cloned().collect(),
            // A clipping plane's own style is the plane's, below.
            section: (a.section_source == crate::sections::FROM_OBJECT
                && !matches!(object.geometry, Geometry::ClippingPlane(_)))
            .then_some(a.section.as_ref())
            .flatten()
            .map(|s| crate::sections::from_rhino(s, &pattern_names)),
        };
        if let Some(block) = in_blocks.get(&a.id) {
            definitions.entry(*block).or_default().push(made);
        } else if a.mode == 3 {
            // Part of a definition the file does not list: left out.
            Report::count(&mut report.skipped, "loose block parts");
        } else {
            Report::count(&mut report.brought, kind);
            if let Geometry::ClippingPlane(c) = &object.geometry {
                clips.push((made.id, c.viewports.clone(), c.enabled, a.section.clone()));
            }
            object_numbers.insert(a.id, made.id);
            state.objects.push(std::sync::Arc::new(made));
        }
    }
    // Fields measuring an object name it by the file's object id: Curva's name it by number.
    for object in &mut state.objects {
        let Shape::Annotation(annotation) = &object.shape else {
            continue;
        };
        let words = match annotation {
            curva_document::annotation::Annotation::Text(t) => &t.content,
            curva_document::annotation::Annotation::Leader(l) => &l.content,
            _ => continue,
        };
        let swapped = object_numbers_in(words, &object_numbers);
        if swapped != *words {
            let object = std::sync::Arc::make_mut(object);
            match &mut object.shape {
                Shape::Annotation(curva_document::annotation::Annotation::Text(t)) => {
                    t.content = swapped;
                }
                Shape::Annotation(curva_document::annotation::Annotation::Leader(l)) => {
                    l.content = swapped;
                }
                _ => {}
            }
        }
    }
    // Clipping planes: the views they cut, by the file's viewport ids: the model's standard
    // views as Curva's viewports in Rhino's layout (Top, Perspective, Front, Right), details by
    // their frames. One that is off cuts nothing.
    for (frame, viewports, enabled, style) in clips {
        use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
        let views = if enabled {
            viewports
                .iter()
                .filter_map(|id| {
                    if let Some((name, _)) = model.model_views.iter().find(|(_, v)| v == id) {
                        ["Top", "Perspective", "Front", "Right"]
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(name))
                            .map(ClipView::Viewport)
                    } else {
                        detail_by_viewport
                            .get(id)
                            .map(|&(_, detail)| ClipView::Detail(detail))
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        // Its style, else Rhino's default (the viewport's fill), which is Curva's standard one.
        let style = style.map(|s| crate::sections::from_rhino(&s, &pattern_names));
        state.clipping_planes.push(ClippingPlane {
            frame,
            flipped: false,
            views,
            section: style.as_ref().map_or(SectionFill::default(), |s| s.fill),
            hatch: style.and_then(|s| s.hatch),
        });
    }
    // Layers turned off in particular details.
    for (index, layer) in model.layers.iter().enumerate() {
        let Some(curva_layer) = placed[index] else {
            continue;
        };
        for viewport in &layer.hidden_in {
            let Some(&(layout, frame)) = detail_by_viewport.get(viewport) else {
                continue;
            };
            if let Some(detail) = state
                .layouts
                .iter_mut()
                .filter(|l| l.id == layout)
                .flat_map(|l| l.details.iter_mut())
                .find(|d| d.frame == frame)
                && !detail.hidden_layers.contains(&curva_layer)
            {
                detail.hidden_layers.push(curva_layer);
            }
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
    styles: &[model::DimStyle],
    patterns: &[model::HatchPattern],
    state: &mut State,
    file_mm: f64,
) -> Result<(Shape, &'static str), String> {
    let text = |e: KernelError| e.0;
    Ok(match geometry {
        Geometry::Point(p) => (Shape::Curve(Curve::Point { at: *p }), "points"),
        Geometry::Radial(d) => (
            Shape::Annotation(curva_document::annotation::Annotation::Radial(radial_in(d))),
            "dimensions",
        ),
        Geometry::Ordinate(d) => (
            Shape::Annotation(curva_document::annotation::Annotation::Ordinate(
                ordinate_in(d),
            )),
            "dimensions",
        ),
        Geometry::Angular(d) => (
            Shape::Annotation(curva_document::annotation::Annotation::Angle(angular_in(d))),
            "dimensions",
        ),
        Geometry::Detail(_) => return Err("details come in with their layout".to_owned()),
        Geometry::ClippingPlane(c) => (
            Shape::Curve(clipping_rectangle(c).map_err(text)?),
            "clipping planes",
        ),
        Geometry::Leader(l) => (
            Shape::Annotation(curva_document::annotation::Annotation::Leader(leader_in(l))),
            "leaders",
        ),
        Geometry::Dimension(d) => (
            Shape::Annotation(curva_document::annotation::Annotation::Dimension(
                dimension_in(d),
            )),
            "dimensions",
        ),
        Geometry::Text(t) => (
            Shape::Annotation(curva_document::annotation::Annotation::Text(note_in(
                t, styles,
            ))),
            "texts",
        ),
        Geometry::Hatch(h) => (
            Shape::Annotation(curva_document::annotation::Annotation::Hatch(hatch_in(
                h, patterns, state, file_mm,
            )?)),
            "hatches",
        ),
        Geometry::Dot {
            at,
            text,
            secondary,
            height,
            font,
        } => (
            Shape::Annotation(curva_document::annotation::Annotation::Dot(
                curva_document::annotation::Dot {
                    at: *at,
                    content: text.clone(),
                    secondary: secondary.clone(),
                    height: f64::from(*height),
                    font: (!font.trim().is_empty()).then(|| font.clone()),
                },
            )),
            "dots",
        ),
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
