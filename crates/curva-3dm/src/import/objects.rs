//! The file's tables and objects brought into Curva's model, step by step: the document's
//! settings, linetypes, annotation styles, layers, groups, blocks and pages first, then each
//! object, then what the objects name (fields, clipping planes' views, details' layers, block
//! definitions).

#[allow(clippy::wildcard_imports)]
use super::*;
use crate::archive::Uuid;

/// What the objects look up as they come in: the file's tables as Curva's, by the file's
/// indices and ids.
struct Lookups {
    /// The file's hatch patterns by index, which section styles name them by.
    pattern_names: Vec<String>,
    /// Millimetres per unit of the file's model.
    file_mm: f64,
    /// The file's model unit in Curva's.
    scale: f64,
    /// Millimetres per unit of what is drawn on the file's pages.
    page_mm: f64,
    /// The file's linetypes as Curva's, by index.
    linetypes: Vec<Linetype>,
    /// The annotation styles the file's annotation uses, as Curva's, by the file's id.
    style_ids: BTreeMap<Uuid, u64>,
    /// Each of the file's layers as Curva's, by index, where placed.
    placed: Vec<Option<u64>>,
    /// Each of the file's layers as Curva's, the current layer where not placed.
    layer_ids: Vec<u64>,
    /// A Curva group for each of the file's, by index.
    group_ids: Vec<u64>,
    /// A Curva block definition for each of the file's, by its id.
    block_ids: BTreeMap<Uuid, u64>,
    /// The block each of the file's block parts belongs to, by the part's id.
    in_blocks: BTreeMap<Uuid, Uuid>,
    /// A Curva layout for each of the file's pages, by the page's id.
    page_ids: BTreeMap<Uuid, u64>,
}

impl Lookups {
    /// The linetype at `index` in the file's table, continuous for none.
    fn linetype_of(&self, index: i32) -> Linetype {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.linetypes.get(i).copied())
            .unwrap_or(Linetype::Continuous)
    }

    /// The Curva layer for the file's layer at `index`.
    fn layer_of(&self, index: usize) -> Option<u64> {
        self.layer_ids.get(index).copied()
    }
}

/// What the objects bring as they come in, for the steps after them.
#[derive(Default)]
struct Arrivals {
    /// Block definitions' objects, by the file's block id.
    definitions: BTreeMap<Uuid, Vec<Object>>,
    /// Each detail by its viewport id: its layout and frame, for the layers turned off in it.
    detail_by_viewport: BTreeMap<Uuid, (u64, u64)>,
    /// Clipping planes as they come in: their rectangle, viewports, whether they cut, and
    /// their style.
    clips: Vec<(u64, Vec<Uuid>, bool, Option<crate::sections::SectionStyle>)>,
    /// Each object's Curva number, by the file's id.
    object_numbers: BTreeMap<Uuid, u64>,
}

pub(super) fn bring_in(model: &Model, state: &mut State, report: &mut Report) {
    let look = lookups(model, state, report);
    let mut came = Arrivals::default();
    for object in &model.objects {
        object_in(model, state, report, &look, &mut came, object);
    }
    object_numbers_in_fields(state, &came.object_numbers);
    clipping_planes_in(model, state, &look, &came);
    detail_layers_in(model, state, &look, &came);
    blocks_in(model, state, report, &look, came.definitions);
}

/// The file's tables into Curva's model, as the objects will name them: the unit, document
/// text and notes, linetypes, annotation styles, layers, groups, block definitions and pages.
fn lookups(model: &Model, state: &mut State, report: &mut Report) -> Lookups {
    let pattern_names: Vec<String> = model
        .hatch_patterns
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let (file_mm, scale) = settings_in(model, state, report);
    // Linetypes: the file's table as Curva's own or the document's.
    let linetypes: Vec<Linetype> = model
        .linetypes
        .iter()
        .map(|l| linetype_in(state, l))
        .collect();
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
    let mut look = Lookups {
        pattern_names,
        file_mm,
        scale,
        page_mm: millimetres_per(model.page_unit.unwrap_or(model.unit)).unwrap_or(file_mm),
        linetypes,
        style_ids,
        placed: Vec::new(),
        layer_ids: Vec::new(),
        group_ids: Vec::new(),
        block_ids: BTreeMap::new(),
        in_blocks: BTreeMap::new(),
        page_ids: BTreeMap::new(),
    };
    look.placed = layers_in(model, state, &look);
    let fallback = state.current_layer;
    look.layer_ids = look
        .placed
        .iter()
        .map(|id| id.unwrap_or(fallback))
        .collect();
    // Groups: a new Curva group for each of the file's.
    look.group_ids = model
        .groups
        .iter()
        .map(|_| {
            let id = state.next_group;
            state.next_group += 1;
            id
        })
        .collect();
    // Block definitions first, so instances can name them.
    for block in &model.blocks {
        let id = state.next_block;
        state.next_block += 1;
        look.block_ids.insert(block.id, id);
    }
    look.in_blocks = model
        .blocks
        .iter()
        .flat_map(|b| b.objects.iter().map(move |o| (*o, b.id)))
        .collect();
    look.page_ids = pages_in(model, state);
    look
}

/// The file's unit (noted when it is converted or unknown), what it left out, its document
/// text and notes into the model; millimetres per file unit, and the file's unit in Curva's.
fn settings_in(model: &Model, state: &mut State, report: &mut Report) -> (f64, f64) {
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
    (file_mm, scale)
}

/// Layers: the file's tree as sublayers (spec `sublayers.md` §3), parents first, each merged
/// with a layer of the same full name already there; each of the file's layers as Curva's, by
/// index.
fn layers_in(model: &Model, state: &mut State, look: &Lookups) -> Vec<Option<u64>> {
    let full_names = layer_names(&model.layers);
    let nil = Uuid::NIL;
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
            let id = existing.unwrap_or_else(|| layer_in(state, look, layer, parent));
            placed[index] = Some(id);
            placed_any = true;
        }
        if !placed_any {
            break;
        }
    }
    placed
}

/// One of the file's layers as a new Curva layer under `parent`, returning its id.
fn layer_in(state: &mut State, look: &Lookups, layer: &model::Layer, parent: Option<u64>) -> u64 {
    let id = state.next_layer;
    state.next_layer += 1;
    state.layers.push(Layer {
        id,
        name: layer.name.clone(),
        colour: colour_of(layer.color),
        visible: layer.visible,
        locked: layer.locked,
        print_width: (layer.print_width > 0.0).then(|| PrintWidth::clamped(layer.print_width)),
        linetype: look.linetype_of(layer.linetype),
        parent,
        collapsed: false,
        section: layer
            .section
            .as_ref()
            .map(|s| crate::sections::from_rhino(s, &look.pattern_names)),
    });
    id
}

/// Layouts: one for each of the file's pages, in millimetres; each page's layout by its id.
fn pages_in(model: &Model, state: &mut State) -> BTreeMap<Uuid, u64> {
    let mut page_ids = BTreeMap::new();
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
    page_ids
}

/// One of the file's objects into the model, a block definition, or the report.
fn object_in(
    model: &Model,
    state: &mut State,
    report: &mut Report,
    look: &Lookups,
    came: &mut Arrivals,
    object: &model::Object,
) {
    let a = &object.attributes;
    let space = if a.page_space {
        match look.page_ids.get(&a.viewport) {
            Some(layout) => Some(*layout),
            None => {
                Report::count(
                    &mut report.skipped,
                    "objects on a layout the file does not list",
                );
                return;
            }
        }
    } else {
        None
    };
    if let Geometry::Detail(d) = &object.geometry {
        detail_object_in(state, report, look, came, a, d, space);
        return;
    }
    // Meshes come in once the document holds them (#266); until then they are left out.
    if let Geometry::Mesh(_) = &object.geometry {
        Report::count(&mut report.skipped, "mesh");
        return;
    }
    if let Geometry::SubD(_) = &object.geometry {
        Report::count(&mut report.skipped, "SubD");
        return;
    }
    let (shape, kind) = match shape_in(model, state, look, object, space) {
        Ok(made) => made,
        Err(why) => {
            Report::count(&mut report.skipped, &format!("unreadable ({why})"));
            return;
        }
    };
    let id = state.next_id;
    state.next_id += 1;
    let made = object_from(look, object, id, shape, space, state.current_layer);
    if let Some(block) = look.in_blocks.get(&a.id) {
        came.definitions.entry(*block).or_default().push(made);
    } else if a.mode == 3 {
        // Part of a definition the file does not list: left out.
        Report::count(&mut report.skipped, "loose block parts");
    } else {
        Report::count(&mut report.brought, kind);
        if let Geometry::ClippingPlane(c) = &object.geometry {
            came.clips
                .push((made.id, c.viewports.clone(), c.enabled, a.section.clone()));
        }
        came.object_numbers.insert(a.id, made.id);
        state.objects.push(std::sync::Arc::new(made));
    }
}

/// A detail on a page: its frame as a curve on the page's layout, and the detail itself.
fn detail_object_in(
    state: &mut State,
    report: &mut Report,
    look: &Lookups,
    came: &mut Arrivals,
    a: &model::Attributes,
    d: &model::Detail,
    space: Option<u64>,
) {
    match (space, detail_in(d, look.page_mm, look.scale)) {
        (Some(layout), Ok((frame, mut detail))) => {
            let id = state.next_id;
            state.next_id += 1;
            detail.frame = id;
            state.objects.push(std::sync::Arc::new(Object {
                space: Some(layout),
                id,
                shape: Shape::Curve(frame),
                layer: look.layer_of(a.layer).unwrap_or(state.current_layer),
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
                ties: Vec::new(),
            }));
            if let Some(sheet) = state.layouts.iter_mut().find(|l| l.id == layout) {
                came.detail_by_viewport.insert(a.id, (layout, id));
                sheet.details.push(detail);
            }
            Report::count(&mut report.brought, "details");
        }
        (_, Err(why)) => {
            Report::count(&mut report.skipped, &format!("unreadable ({why})"));
        }
        (None, _) => Report::count(&mut report.skipped, "details outside a layout"),
    }
}

/// An object's shape in Curva's units, with its kind for the report: on a page in the page's
/// unit; fields naming layouts by number; its annotation style as Curva's.
fn shape_in(
    model: &Model,
    state: &mut State,
    look: &Lookups,
    object: &model::Object,
    space: Option<u64>,
) -> Result<(Shape, &'static str), String> {
    let scale = if space.is_some() {
        look.page_mm
    } else {
        look.scale
    };
    let (shape, kind) = shape_of(
        &object.geometry,
        &look.block_ids,
        &model.dim_styles,
        &model.hatch_patterns,
        state,
        look.file_mm,
    )?;
    let mut shape = scaled(shape, scale)?;
    // Rhino 5's leaders (all a Rhino 5 file has) end at their words, with no landing.
    if model.version < 60
        && let Shape::Annotation(curva_document::annotation::Annotation::Leader(l)) = &mut shape
    {
        l.landing = false;
    }
    let shape = with_layout_numbers(shape, &look.page_ids);
    let shape = match style_named(&object.geometry).and_then(|s| look.style_ids.get(&s)) {
        Some(style) => with_style(shape, *style),
        None => shape,
    };
    Ok((shape, kind))
}

/// The Curva object numbered `id` for one of the file's, with its shape and attributes.
fn object_from(
    look: &Lookups,
    object: &model::Object,
    id: u64,
    shape: Shape,
    space: Option<u64>,
    current_layer: u64,
) -> Object {
    let a = &object.attributes;
    Object {
        space,
        id,
        shape,
        layer: look.layer_of(a.layer).unwrap_or(current_layer),
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
        group: a
            .groups
            .first()
            .and_then(|g| look.group_ids.get(*g))
            .copied(),
        live: None,
        input_of: None,
        colour: (a.color_source == model::Source::Object)
            .then(|| a.color.map(|c| f32::from(c) / 255.0)),
        linetype: if a.linetype_source == model::Source::Object {
            look.linetype_of(a.linetype)
        } else {
            Linetype::ByLayer
        },
        user_text: a.user_text.iter().cloned().collect(),
        // A clipping plane's own style is the plane's, below.
        section: (a.section_source == crate::sections::FROM_OBJECT
            && !matches!(object.geometry, Geometry::ClippingPlane(_)))
        .then_some(a.section.as_ref())
        .flatten()
        .map(|s| crate::sections::from_rhino(s, &look.pattern_names)),
        ties: Vec::new(),
    }
}

/// Fields measuring an object name it by the file's object id: Curva's name it by number.
fn object_numbers_in_fields(state: &mut State, object_numbers: &BTreeMap<Uuid, u64>) {
    use curva_document::annotation::Annotation;
    for object in &mut state.objects {
        let Shape::Annotation(annotation) = &object.shape else {
            continue;
        };
        let words = match annotation {
            Annotation::Text(t) => &t.content,
            Annotation::Leader(l) => &l.content,
            _ => continue,
        };
        let swapped = object_numbers_in(words, object_numbers);
        if swapped != *words {
            let object = std::sync::Arc::make_mut(object);
            match &mut object.shape {
                Shape::Annotation(Annotation::Text(t)) => t.content = swapped,
                Shape::Annotation(Annotation::Leader(l)) => l.content = swapped,
                _ => {}
            }
        }
    }
}

/// Clipping planes: the views they cut, by the file's viewport ids: the model's standard views
/// as Curva's viewports in Rhino's layout (Top, Perspective, Front, Right), details by their
/// frames. One that is off cuts nothing. Its style, else Rhino's default (the viewport's fill),
/// which is Curva's standard one.
fn clipping_planes_in(model: &Model, state: &mut State, look: &Lookups, came: &Arrivals) {
    use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
    for (frame, viewports, enabled, style) in &came.clips {
        let views = if *enabled {
            viewports
                .iter()
                .filter_map(|id| {
                    if let Some((name, _)) = model.model_views.iter().find(|(_, v)| v == id) {
                        ["Top", "Perspective", "Front", "Right"]
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(name))
                            .map(ClipView::Viewport)
                    } else {
                        came.detail_by_viewport
                            .get(id)
                            .map(|&(_, detail)| ClipView::Detail(detail))
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let style = style
            .as_ref()
            .map(|s| crate::sections::from_rhino(s, &look.pattern_names));
        state.clipping_planes.push(ClippingPlane {
            frame: *frame,
            flipped: false,
            views,
            section: style.as_ref().map_or(SectionFill::default(), |s| s.fill),
            hatch: style.and_then(|s| s.hatch),
        });
    }
}

/// Layers turned off in particular details.
fn detail_layers_in(model: &Model, state: &mut State, look: &Lookups, came: &Arrivals) {
    for (index, layer) in model.layers.iter().enumerate() {
        let Some(curva_layer) = look.placed[index] else {
            continue;
        };
        for viewport in &layer.hidden_in {
            let Some(&(layout, frame)) = came.detail_by_viewport.get(viewport) else {
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
}

/// The block definitions with their objects, renamed where the model has one of that name; a
/// linked block whose file was not read is noted.
fn blocks_in(
    model: &Model,
    state: &mut State,
    report: &mut Report,
    look: &Lookups,
    mut definitions: BTreeMap<Uuid, Vec<Object>>,
) {
    for block in &model.blocks {
        let Some(id) = look.block_ids.get(&block.id).copied() else {
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
