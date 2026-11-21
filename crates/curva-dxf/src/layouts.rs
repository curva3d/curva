//! Layouts to and from DXF and DWG (spec `drawing-output.md` §3): each Curva layout is a DXF
//! layout (a paper space with its paper size), what is drawn on the sheet goes into that paper
//! space in millimetres, and each detail is a viewport entity looking into the model at its
//! scale (AutoCAD DXF Reference, "LAYOUT" and "VIEWPORT"). Coming in, the same the other way.

use crate::{P, p3, unit, v3};
use acadrust::CadDocument;
use acadrust::entities::{EntityType, Viewport};
use acadrust::objects::ObjectType;
use acadrust::types::Vector3;
use curva_document::layouts::Detail;
use curva_kernel::Curve;

/// The DXF layout for a Curva layout, made (or, for the first, the empty `Layout1` a new
/// drawing has, renamed) with its paper size in millimetres. Returns its name.
pub(crate) fn layout_out(
    cad: &mut CadDocument,
    name: &str,
    first: bool,
    size_mm: [f64; 2],
) -> String {
    let mut name = if name.trim().is_empty() {
        "Layout".to_owned()
    } else {
        crate::dxf_name(name)
    };
    let taken = |cad: &CadDocument, n: &str| {
        cad.objects
            .values()
            .any(|o| matches!(o, ObjectType::Layout(l) if l.name.eq_ignore_ascii_case(n)))
    };
    let renamed = first && rename_first(cad, &name);
    if !renamed {
        while taken(cad, &name) {
            name.push('_');
        }
        let _ = cad.add_layout(&name);
    }
    for object in cad.objects.values_mut() {
        if let ObjectType::Layout(l) = object
            && l.name == name
        {
            l.paper_width = size_mm[0];
            l.paper_height = size_mm[1];
            l.min_limits = (0.0, 0.0);
            l.max_limits = (size_mm[0], size_mm[1]);
        }
    }
    // Every layout has its own viewport first, id 1, the size of the sheet; a new drawing's
    // Layout1 comes without one.
    let block = cad.objects.values().find_map(|o| match o {
        ObjectType::Layout(l) if l.name == name => Some(l.block_record),
        _ => None,
    });
    let has_own = block
        .and_then(|handle| cad.block_records.iter().find(|b| b.handle == handle))
        .is_some_and(|record| {
            record
                .entity_handles
                .iter()
                .any(|h| matches!(cad.get_entity(*h), Some(EntityType::Viewport(v)) if v.id == 1))
        });
    if !has_own {
        let mut own = Viewport::new();
        own.id = 1;
        own.center = Vector3::new(size_mm[0] / 2.0, size_mm[1] / 2.0, 0.0);
        own.width = size_mm[0];
        own.height = size_mm[1];
        own.view_center = own.center;
        own.view_height = size_mm[1];
        let _ = cad.add_entity_to_layout(EntityType::Viewport(Box::new(own)), &name);
    }
    name
}

/// Renames the drawing's `Layout1` to `name`, in the layout and in the layout dictionary.
fn rename_first(cad: &mut CadDocument, name: &str) -> bool {
    let mut handle = None;
    for object in cad.objects.values_mut() {
        if let ObjectType::Layout(l) = object
            && l.name == "Layout1"
        {
            name.clone_into(&mut l.name);
            handle = Some(l.handle);
        }
    }
    let Some(handle) = handle else {
        return false;
    };
    for object in cad.objects.values_mut() {
        if let ObjectType::Dictionary(d) = object {
            for entry in &mut d.entries {
                if entry.1 == handle {
                    name.clone_into(&mut entry.0);
                }
            }
        }
    }
    true
}

/// The view direction (from the target towards the camera) of a detail's standard view.
/// A DXF viewport's view direction: from the target towards the eye, against the way it looks.
fn direction_of(projection: &str) -> P {
    match curva_document::layouts::flat_view_axes(projection) {
        Some((looking, _)) => looking.map(|c| -c),
        None => {
            let s = 1.0 / 3f64.sqrt();
            [s, -s, s]
        }
    }
}

/// A detail as a viewport: centred on its frame (`low` to `high`, in millimetres), looking at
/// its target from its standard view, showing as much of the model as its scale fits.
pub(crate) fn viewport_out(detail: &Detail, low: P, high: P, model_mm: f64, id: i16) -> Viewport {
    let mut v = Viewport::new();
    let (width, height) = (high[0] - low[0], high[1] - low[1]);
    v.center = Vector3::new((low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, 0.0);
    v.width = width;
    v.height = height;
    v.id = id;
    v.view_target = v3(detail.target);
    v.view_direction = v3(direction_of(&detail.projection));
    v.view_center = Vector3::new(0.0, 0.0, 0.0);
    match detail.scale {
        Some(scale) => v.view_height = height / scale / model_mm,
        None => {
            v.status.perspective = true;
            v.view_height = detail.distance;
        }
    }
    v.status.locked = detail.locked;
    v
}

/// A viewport as a detail and its frame on the sheet: its standard view by its direction (else
/// a perspective), its target, and its scale from how much of the model its height shows.
pub(crate) fn viewport_in(v: &Viewport, model_mm: f64) -> Option<(Curve, Detail)> {
    if v.width <= 0.0 || v.height <= 0.0 {
        // One of no size.
        return None;
    }
    let c = p3(&v.center);
    let (w, h) = (v.width / 2.0, v.height / 2.0);
    let frame = Curve::polyline(
        vec![
            [c[0] - w, c[1] - h, 0.0],
            [c[0] + w, c[1] - h, 0.0],
            [c[0] + w, c[1] + h, 0.0],
            [c[0] - w, c[1] + h, 0.0],
        ],
        true,
    )
    .ok()?;
    let dir = unit(p3(&v.view_direction)).unwrap_or([0.0, 0.0, 1.0]);
    // The view direction points at the eye: the way it looks is against it.
    let projection = if v.status.perspective {
        "Perspective"
    } else {
        curva_document::layouts::flat_view_looking(dir.map(|c| -c)).unwrap_or("Perspective")
    };
    let parallel = projection != "Perspective";
    Some((
        frame,
        Detail {
            frame: 0,
            projection: projection.to_owned(),
            target: p3(&v.view_target),
            distance: if parallel { 100.0 } else { v.view_height },
            scale: (parallel && v.view_height > 0.0).then(|| v.height / (v.view_height * model_mm)),
            display: if parallel { "Wireframe" } else { "Shaded" }.to_owned(),
            locked: v.status.locked,
            hidden_layers: Vec::new(),
        },
    ))
}

/// One of the drawing's layouts, as Curva reads it.
pub(crate) struct LayoutIn {
    /// Its name.
    pub name: String,
    /// Its sheet's width and height in millimetres, as it lies.
    pub size: [f64; 2],
    /// The block its paper space draws into.
    pub block: String,
    /// Where paper space's origin is on the sheet: AutoCAD measures from the printable area's
    /// corner, inside the plot margins.
    pub offset: [f64; 2],
}

/// The drawing's layouts other than the model, in tab order: each one's name, sheet size in
/// millimetres (turned by its plot rotation; else its limits; else A3), paper space block and
/// origin on the sheet.
pub(crate) fn layouts_in(cad: &CadDocument) -> Vec<LayoutIn> {
    let mut found: Vec<(i16, LayoutIn)> = cad
        .objects
        .values()
        .filter_map(|o| match o {
            ObjectType::Layout(l) if !l.name.eq_ignore_ascii_case("Model") => {
                let block = cad
                    .block_records
                    .iter()
                    .find(|b| b.handle == l.block_record)?
                    .name
                    .clone();
                let turned = l.plot_rotation % 2 == 1;
                let size = if l.paper_width > 0.0 && l.paper_height > 0.0 {
                    if turned {
                        [l.paper_height, l.paper_width]
                    } else {
                        [l.paper_width, l.paper_height]
                    }
                } else if l.max_limits.0 > l.min_limits.0 && l.max_limits.1 > l.min_limits.1 {
                    [
                        l.max_limits.0 - l.min_limits.0,
                        l.max_limits.1 - l.min_limits.1,
                    ]
                } else {
                    [420.0, 297.0]
                };
                // The printable corner on the sheet, by how the sheet lies on the paper.
                let offset = match l.plot_rotation {
                    1 => [l.plot_margin_bottom, l.plot_margin_right],
                    2 => [l.plot_margin_right, l.plot_margin_top],
                    3 => [l.plot_margin_top, l.plot_margin_left],
                    _ => [l.plot_margin_left, l.plot_margin_bottom],
                };
                Some((
                    l.tab_order,
                    LayoutIn {
                        name: l.name.clone(),
                        size,
                        block,
                        offset,
                    },
                ))
            }
            _ => None,
        })
        .collect();
    found.sort_by_key(|(order, _)| *order);
    found.into_iter().map(|(_, l)| l).collect()
}

/// Whether an entity is the viewport of a paper space.
pub(crate) fn is_viewport(entity: &EntityType) -> Option<&Viewport> {
    match entity {
        EntityType::Viewport(v) => Some(v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{Format, export, import, read};
    use curva_document::Document;
    use curva_document::layouts::{Detail, PaperUnit};
    use curva_kernel::Curve;

    #[test]
    fn layouts_go_out_with_their_viewports_and_come_back() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0; 3], [10_000.0, 0.0, 0.0]).unwrap());
        let furniture = doc.layer_at_path("Furniture");
        let sheet = doc.add_layout_sized("Plan".to_owned(), [420.0, 297.0], PaperUnit::Millimetres);
        doc.set_space(Some(sheet));
        let frame = doc.add(
            Curve::polyline(
                vec![
                    [20.0, 20.0, 0.0],
                    [220.0, 20.0, 0.0],
                    [220.0, 170.0, 0.0],
                    [20.0, 170.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.add(Curve::line([250.0, 20.0, 0.0], [400.0, 20.0, 0.0]).unwrap());
        doc.add_detail(
            sheet,
            Detail {
                frame,
                projection: "Top".to_owned(),
                target: [5000.0, 0.0, 0.0],
                distance: 100.0,
                scale: Some(0.02),
                display: "Wireframe".to_owned(),
                locked: true,
                hidden_layers: vec![furniture],
            },
        );
        doc.set_space(None);
        for format in [Format::Dxf, Format::Dwg] {
            let (bytes, report) = export(&doc, None, format).unwrap();
            assert!(report.skipped.is_empty(), "{report:?}");
            if let (Format::Dxf, Ok(path)) = (format, std::env::var("CURVA_DXF_LAYOUT_SAMPLE")) {
                std::fs::write(path, &bytes).unwrap();
            }
            let mut back = Document::default();
            import(&read(&bytes, format).unwrap(), &mut back);
            let layouts = back.layouts();
            assert_eq!(
                layouts.len(),
                1,
                "{format:?}: Layout1 renamed, not added to"
            );
            let plan = &layouts[0];
            assert_eq!(plan.name, "Plan");
            assert_eq!(plan.size, [420.0, 297.0]);
            let d = &plan.details[0];
            assert_eq!(d.projection, "Top");
            assert!(
                (d.scale.unwrap() - 0.02).abs() < 1e-9,
                "{format:?}: {:?}",
                d.scale
            );
            assert!((d.target[0] - 5000.0).abs() < 1e-9);
            assert!(d.locked);
            let hidden: Vec<String> = d
                .hidden_layers
                .iter()
                .filter_map(|id| back.state().layer(*id).map(|l| l.name.clone()))
                .collect();
            assert_eq!(
                hidden,
                ["Furniture"],
                "{format:?}: hidden in the detail only"
            );
            let state = back.state();
            let frame = state.objects.iter().find(|o| o.id == d.frame).unwrap();
            let (low, high) = frame.shape.bounds().unwrap();
            assert!((low[0] - 20.0).abs() < 1e-9 && (high[1] - 170.0).abs() < 1e-9);
            assert_eq!(
                state
                    .objects
                    .iter()
                    .filter(|o| o.space == Some(plan.id) && o.id != d.frame)
                    .count(),
                1,
                "{format:?}: the sheet's line"
            );
            assert_eq!(
                state.objects.iter().filter(|o| o.space.is_none()).count(),
                1
            );
        }
    }
}
