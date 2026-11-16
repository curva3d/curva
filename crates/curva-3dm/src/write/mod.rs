//! Writing a [`Model`] as a Rhino 7 `.3dm` file, as openNURBS writes one: the start, the
//! tables in their order, and the end mark with the file's size.

use crate::archive::{Uuid, Writer, tcode};
use crate::model::{
    AngularDimension, Attributes, Block, Brep, ClippingPlane, Curve, Detail, DimStyle, Geometry,
    Group, HatchLine, HatchPattern, Layer, Leader, LinearDimension, Linetype, Model, NurbsCurve,
    NurbsSurface, Object, OrdinateDimension, P3, Plane, RadialDimension, Source, Surface, Text,
    View,
};
use crate::read::class;

mod annotation;
mod geometry;
mod views;
use annotation::*;
use geometry::*;
#[allow(clippy::wildcard_imports)]
use views::*;

/// The openNURBS version a Rhino 7 file says wrote it.
const OPENNURBS_VERSION: i64 = 2_382_393_748;

/// The file's bytes.
#[must_use]
pub fn write(model: &Model) -> Vec<u8> {
    let mut w = Writer::default();
    w.out.extend_from_slice(b"3D Geometry File Format       70");
    w.u32(tcode::COMMENTBLOCK);
    let comment = format!(
        " Written by Curva {} (https://github.com/curva3d/curva)\n\u{1a}\0",
        env!("CARGO_PKG_VERSION")
    );
    w.i64(comment.len() as i64);
    w.out.extend_from_slice(comment.as_bytes());

    table(&mut w, tcode::PROPERTIES_TABLE, |w| {
        w.short(tcode::PROPERTIES_OPENNURBS_VERSION, OPENNURBS_VERSION);
        w.begin(tcode::PROPERTIES_APPLICATION);
        w.chunk_version(1, 0);
        w.wstring("Curva");
        w.wstring("https://github.com/curva3d/curva");
        w.wstring(concat!("Curva ", env!("CARGO_PKG_VERSION")));
        w.end();
        if !model.notes.is_empty() {
            // ON_3dmNotes 1.1: plain text, its words, not shown, no window, not locked.
            w.begin(tcode::PROPERTIES_NOTES);
            w.chunk_version(1, 1);
            w.i32(0);
            w.wstring(&model.notes);
            for v in [0, 0, 0, 0, 0] {
                w.i32(v);
            }
            w.bool(false);
            w.end();
        }
    });
    table(&mut w, tcode::SETTINGS_TABLE, |w| {
        w.begin(tcode::SETTINGS_UNITSANDTOLS);
        w.i32(102);
        w.u32(model.unit);
        w.f64(model.tolerance);
        w.f64(model.angle_tolerance);
        w.f64(0.01);
        w.i32(0);
        w.i32(3);
        w.f64(metres_per(model.unit));
        w.wstring("");
        w.end();
        w.begin(tcode::SETTINGS_NAMED_CPLANE_LIST);
        w.count(model.cplanes.len());
        for c in &model.cplanes {
            w.begin(tcode::VIEW_CPLANE);
            write_cplane(w, &c.plane, &c.name);
            w.end();
        }
        w.end();
        w.begin(tcode::SETTINGS_NAMED_VIEW_LIST);
        w.count(model.views.len());
        for (i, v) in model.views.iter().enumerate() {
            w.begin(tcode::VIEW_RECORD);
            write_view(w, v, Uuid::fresh(0x5649_4557 + i as u64), None, &[]);
            w.end();
        }
        w.end();
        if !model.pages.is_empty() {
            write_view_list(w, model);
            write_page_unit(w, model);
        }
    });
    for empty in [
        tcode::BITMAP_TABLE,
        tcode::TEXTURE_MAPPING_TABLE,
        tcode::MATERIAL_TABLE,
    ] {
        table(&mut w, empty, |_| {});
    }
    table(&mut w, tcode::LINETYPE_TABLE, |w| {
        for (index, linetype) in model.linetypes.iter().enumerate() {
            w.begin(tcode::LINETYPE_RECORD);
            class_object(w, class::LINETYPE, |w| write_linetype(w, linetype, index));
            w.end();
        }
    });
    table(&mut w, tcode::LAYER_TABLE, |w| {
        for (index, layer) in model.layers.iter().enumerate() {
            w.begin(tcode::LAYER_RECORD);
            class_object_with(
                w,
                class::LAYER,
                |w| write_layer(w, layer, index),
                |w| {
                    if !layer.hidden_in.is_empty() {
                        write_layer_extensions(w, &layer.hidden_in);
                    }
                },
            );
            w.end();
        }
    });
    table(&mut w, tcode::GROUP_TABLE, |w| {
        for (index, group) in model.groups.iter().enumerate() {
            w.begin(tcode::GROUP_RECORD);
            class_object(w, class::GROUP, |w| write_group(w, group, index));
            w.end();
        }
    });
    table(&mut w, tcode::FONT_TABLE, |_| {});
    table(&mut w, tcode::DIMSTYLE_TABLE, |w| {
        for (index, style) in model.dim_styles.iter().enumerate() {
            w.begin(tcode::DIMSTYLE_RECORD);
            class_object(w, class::DIM_STYLE, |w| write_dim_style(w, style, index));
            w.end();
        }
    });
    table(&mut w, tcode::LIGHT_TABLE, |_| {});
    table(&mut w, tcode::HATCHPATTERN_TABLE, |w| {
        for (index, pattern) in model.hatch_patterns.iter().enumerate() {
            w.begin(tcode::HATCHPATTERN_RECORD);
            class_object(w, class::HATCH_PATTERN, |w| {
                write_hatch_pattern(w, pattern, index);
            });
            w.end();
        }
    });
    table(&mut w, tcode::INSTANCE_DEFINITION_TABLE, |w| {
        for (index, block) in model.blocks.iter().enumerate() {
            w.begin(tcode::INSTANCE_DEFINITION_RECORD);
            class_object(w, class::INSTANCE_DEFINITION, |w| {
                write_block(w, block, index, model.unit);
            });
            w.end();
        }
    });
    table(&mut w, tcode::OBJECT_TABLE, |w| {
        for object in &model.objects {
            write_object(w, object);
        }
    });
    table(&mut w, tcode::HISTORYRECORD_TABLE, |_| {});
    if !model.document_text.is_empty() {
        write_document_text(&mut w, &model.document_text);
    }
    // The end mark holds the size of the whole file, itself included.
    let size = w.out.len() as i64 + 4 + 8 + 8;
    w.u32(tcode::ENDOFFILE);
    w.i64(8);
    w.i64(size);
    w.out
}

fn metres_per(unit: u32) -> f64 {
    match unit {
        2 => 0.001,
        3 => 0.01,
        4 => 1.0,
        8 => 0.0254,
        9 => 0.3048,
        _ => 0.001,
    }
}

/// A table: its chunk, its records, and the end-of-table mark.
fn table(w: &mut Writer, code: u32, records: impl FnOnce(&mut Writer)) {
    w.begin(code);
    records(w);
    w.short(tcode::ENDOFTABLE, 0);
    w.end();
}

/// An object as openNURBS' `WriteObject` writes it: its class id, then its data.
fn class_object(w: &mut Writer, id: Uuid, data: impl FnOnce(&mut Writer)) {
    w.begin(tcode::OPENNURBS_CLASS);
    w.begin(tcode::OPENNURBS_CLASS_UUID);
    w.uuid(id);
    w.end();
    w.begin(tcode::OPENNURBS_CLASS_DATA);
    data(w);
    w.end();
    w.short(tcode::OPENNURBS_CLASS_END, 0);
    w.end();
}

/// An object written with openNURBS' `WriteObject` and its user data after its data.
fn class_object_with(
    w: &mut Writer,
    id: Uuid,
    data: impl FnOnce(&mut Writer),
    user_data: impl FnOnce(&mut Writer),
) {
    w.begin(tcode::OPENNURBS_CLASS);
    w.begin(tcode::OPENNURBS_CLASS_UUID);
    w.uuid(id);
    w.end();
    w.begin(tcode::OPENNURBS_CLASS_DATA);
    data(w);
    w.end();
    user_data(w);
    w.short(tcode::OPENNURBS_CLASS_END, 0);
    w.end();
}

/// One piece of user data as openNURBS writes it (`WriteObjectUserData`): its header (class,
/// item, copy count, transform, owning application) and its data in a chunk of its own.
fn write_user_data(w: &mut Writer, class: Uuid, application: Uuid, data: impl FnOnce(&mut Writer)) {
    w.begin(tcode::OPENNURBS_CLASS_USERDATA);
    w.chunk_version(2, 2);
    w.begin(tcode::OPENNURBS_CLASS_USERDATA_HEADER);
    w.uuid(class);
    w.uuid(class);
    w.i32(1); // copy count
    for row in 0..4 {
        for col in 0..4 {
            w.f64(if row == col { 1.0 } else { 0.0 });
        }
    }
    w.uuid(application);
    w.bool(false);
    w.i32(70);
    w.u32(OPENNURBS_VERSION as u32);
    w.end();
    w.begin(tcode::ANONYMOUS_CHUNK);
    data(w);
    w.end();
    w.end();
}

/// A layer's per-viewport settings (`ON__LayerExtensions`): turned off (2) in each of these
/// details, by viewport id. Each setting is version 1.2: its bits (1 the viewport id, 16 the
/// visibility), the id, and the visibility twice, as openNURBS writes it for older Rhinos.
fn write_layer_extensions(w: &mut Writer, off_in: &[Uuid]) {
    write_user_data(w, class::LAYER_EXTENSIONS, class::OPENNURBS5, |w| {
        w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
        w.count(off_in.len());
        for viewport in off_in {
            w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 2);
            w.u32(1 | 16);
            w.uuid(*viewport);
            w.u8(2);
            w.u8(2);
            w.end();
        }
        w.end();
    });
}

fn color(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 0]
}

/// openNURBS' unset colour.
pub(crate) const UNSET_COLOR: [u8; 4] = [255, 255, 255, 255];

fn write_layer(w: &mut Writer, layer: &Layer, index: usize) {
    // Version 1.15, as Rhino 8 writes, only for a section style, which comes among its items.
    w.chunk_version(1, if layer.section.is_some() { 15 } else { 6 });
    let mode = if !layer.visible {
        1
    } else if layer.locked {
        2
    } else {
        0
    };
    w.i32(mode);
    w.count(index);
    w.i32(0); // IGES level
    w.i32(-1); // material
    w.i32(0);
    w.color(color(layer.color));
    w.u16(0);
    w.u16(0);
    w.f64(0.0);
    w.f64(1.0);
    w.wstring(&layer.name);
    w.bool(layer.visible);
    w.i32(layer.linetype);
    w.color(UNSET_COLOR);
    w.f64(layer.print_width);
    w.bool(layer.locked);
    w.uuid(layer.id);
    w.uuid(layer.parent);
    w.bool(true); // expanded
    if let Some(style) = &layer.section {
        // Rendering attributes with no material mappings, no display material, then the items.
        w.begin_anonymous(0);
        w.i32(0);
        w.end();
        w.uuid(Uuid::NIL);
        w.u8(35);
        crate::sections::write(w, style);
        w.u8(0);
    }
}

/// A linetype as Rhino 8 writes one (`ON_Linetype::Write`, version 2.3): its id, index and
/// name, its segments (length, then 0 drawn or 1 left out), and no further properties.
fn write_linetype(w: &mut Writer, linetype: &Linetype, index: usize) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 2, 3);
    w.begin_versioned(tcode::MODEL_ATTRIBUTES_CHUNK, 1, 0);
    w.u8(0); // model serial number
    w.u8(1);
    w.uuid(linetype.id);
    w.u8(0); // component type
    w.u8(1);
    w.count(index);
    w.u8(1);
    w.wstring(&linetype.name);
    w.end();
    w.count(linetype.segments.len());
    for (length, drawn) in &linetype.segments {
        w.f64(*length);
        w.i32(i32::from(!drawn));
    }
    w.u8(0);
    w.end();
}

fn write_group(w: &mut Writer, group: &Group, index: usize) {
    w.chunk_version(1, 1);
    w.count(index);
    w.wstring(&group.name);
    w.uuid(group.id);
}

fn empty_box(w: &mut Writer) {
    w.f64s(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
}

fn write_block(w: &mut Writer, block: &Block, index: usize, unit: u32) {
    w.begin_anonymous(0);
    w.begin_versioned(tcode::MODEL_ATTRIBUTES_CHUNK, 1, 0);
    w.u8(0); // model serial number
    w.u8(1);
    w.uuid(block.id);
    w.u8(0); // component type
    w.u8(1);
    w.count(index);
    w.u8(1);
    w.wstring(&block.name);
    w.end();
    w.u32(1); // a static (embedded) definition
    w.begin_anonymous(0);
    w.u32(unit);
    w.f64(metres_per(unit));
    w.wstring("");
    w.end();
    w.wstring(&block.description);
    w.wstring("");
    w.wstring("");
    empty_box(w);
    w.bool(true);
    w.count(block.objects.len());
    for id in &block.objects {
        w.uuid(*id);
    }
    w.bool(false); // not linked
    w.end();
}

/// openNURBS' object type of a geometry.
fn object_type(g: &Geometry) -> i64 {
    match g {
        Geometry::Point(_) => 1,
        Geometry::Curve(_) => 4,
        Geometry::Surface(_) => 8,
        Geometry::Brep(_) | Geometry::Extrusion(_) => 0x10,
        Geometry::Instance { .. } => 0x1000,
        Geometry::Dot { .. } => 0x2000,
        Geometry::Hatch(_) => 0x10000,
        Geometry::Text(_)
        | Geometry::Dimension(_)
        | Geometry::Radial(_)
        | Geometry::Angular(_)
        | Geometry::Ordinate(_)
        | Geometry::Leader(_) => 0x200,
        Geometry::Detail(_) => 0x8000,
        Geometry::ClippingPlane(_) => 0x2000_0000,
    }
}

fn write_object(w: &mut Writer, object: &Object) {
    let g = &object.geometry;
    w.begin(tcode::OBJECT_RECORD);
    w.short(tcode::OBJECT_RECORD_TYPE, object_type(g));
    match g {
        Geometry::Point(p) => class_object(w, class::POINT, |w| {
            w.chunk_version(1, 0);
            w.f64s(p);
        }),
        Geometry::Curve(c) => write_curve(w, c),
        Geometry::Surface(s) => write_surface(w, s),
        Geometry::Brep(b) => class_object(w, class::BREP, |w| write_brep(w, b)),
        Geometry::Extrusion(_) => {}
        Geometry::Text(t) => class_object(w, class::TEXT, |w| write_text(w, t)),
        Geometry::Dimension(d) => {
            class_object(w, class::DIM_LINEAR, |w| write_linear_dimension(w, d));
        }
        Geometry::Radial(d) => {
            class_object(w, class::DIM_RADIAL, |w| write_radial_dimension(w, d));
        }
        Geometry::Angular(d) => {
            class_object(w, class::DIM_ANGULAR, |w| write_angular_dimension(w, d));
        }
        Geometry::Ordinate(d) => {
            class_object(w, class::DIM_ORDINATE, |w| write_ordinate_dimension(w, d));
        }
        Geometry::Leader(l) => class_object(w, class::LEADER, |w| write_leader(w, l)),
        Geometry::Detail(d) => class_object(w, class::DETAIL, |w| write_detail(w, d)),
        Geometry::ClippingPlane(c) => {
            class_object(w, class::CLIPPING_PLANE_SURFACE, |w| {
                write_clipping_plane(w, c)
            });
        }
        Geometry::Hatch(h) => class_object(w, class::HATCH, |w| {
            // openNURBS' ON_Hatch, version 1.2: plane, scale, turn, pattern, outlines as 2D
            // curves in the plane, then the base point.
            w.chunk_version(1, 2);
            write_plane(w, &h.plane);
            w.f64(h.scale);
            w.f64(h.rotation);
            w.i32(h.pattern);
            w.count(h.loops.len());
            for (outer, curve) in &h.loops {
                w.chunk_version(1, 1);
                w.i32(i32::from(!outer));
                write_curve2(w, curve);
            }
            w.f64s(&h.base);
        }),
        Geometry::Dot {
            at,
            text,
            secondary,
            height,
            font,
        } => class_object(w, class::TEXT_DOT, |w| {
            // openNURBS' ON_TextDot, version 1.1: the point, height, words, font, display bits
            // (none), then the secondary words.
            w.chunk_version(1, 1);
            w.f64s(at);
            w.i32(*height);
            w.wstring(text);
            w.wstring(font);
            w.i32(0);
            w.wstring(secondary);
        }),
        Geometry::Instance { block, xform } => class_object(w, class::INSTANCE_REF, |w| {
            w.chunk_version(1, 0);
            w.uuid(*block);
            for row in xform {
                w.f64s(row);
            }
            empty_box(w);
        }),
    }
    w.begin(tcode::OBJECT_RECORD_ATTRIBUTES);
    write_attributes(w, &object.attributes);
    w.end();
    if !object.attributes.user_text.is_empty() {
        w.begin(tcode::OBJECT_RECORD_ATTRIBUTES_USERDATA);
        write_user_text(w, &object.attributes.user_text);
        w.short(tcode::OPENNURBS_CLASS_END, 0);
        w.end();
    }
    w.short(tcode::OBJECT_RECORD_END, 0);
    w.end();
}

fn source(s: Source) -> u8 {
    match s {
        Source::Layer => 0,
        Source::Object => 1,
        Source::Parent => 3,
    }
}

fn write_attributes(w: &mut Writer, a: &Attributes) {
    // Version 2.13, as Rhino 8 writes, only for section styles, which older readers skip.
    let sectioned = a.section_source != crate::sections::FROM_LAYER || a.section.is_some();
    w.chunk_version(2, if sectioned { 13 } else { 1 });
    w.uuid(a.id);
    w.count(a.layer);
    if !a.name.is_empty() {
        w.u8(1);
        w.wstring(&a.name);
    }
    if a.linetype >= 0 {
        w.u8(3);
        w.i32(a.linetype);
    }
    if a.color_source == Source::Object {
        w.u8(6);
        w.color(color(a.color));
    }
    if a.print_width_source == Source::Object && a.print_width != 0.0 {
        w.u8(8);
        w.f64(a.print_width);
    }
    if !a.visible {
        w.u8(11);
        w.bool(false);
    }
    if a.mode != 0 {
        w.u8(12);
        w.u8(a.mode);
    }
    if a.color_source != Source::Layer {
        w.u8(13);
        w.u8(source(a.color_source));
    }
    if a.print_width_source != Source::Layer {
        w.u8(15);
        w.u8(source(a.print_width_source));
    }
    if a.linetype_source != Source::Layer {
        w.u8(17);
        w.u8(source(a.linetype_source));
    }
    if !a.groups.is_empty() {
        w.u8(18);
        w.count(a.groups.len());
        for g in &a.groups {
            w.count(*g);
        }
    }
    // On a layout: paper space, and the page it is drawn on.
    if a.page_space {
        w.u8(19);
        w.u8(2); // page space
    }
    if a.viewport != Uuid::NIL {
        w.u8(20);
        w.uuid(a.viewport);
    }
    if a.section_source != crate::sections::FROM_LAYER {
        w.u8(29);
        w.u8(a.section_source);
    }
    if let Some(style) = &a.section {
        w.u8(39);
        crate::sections::write(w, style);
    }
    w.u8(0);
}

/// User text as openNURBS' user string list, owned by openNURBS itself.
fn write_user_text(w: &mut Writer, pairs: &[(String, String)]) {
    w.begin(tcode::OPENNURBS_CLASS_USERDATA);
    w.chunk_version(2, 2);
    w.begin(tcode::OPENNURBS_CLASS_USERDATA_HEADER);
    w.uuid(class::USER_STRING_LIST);
    w.uuid(class::USER_STRING_LIST);
    w.i32(1); // copy count
    for row in 0..4 {
        for col in 0..4 {
            w.f64(if row == col { 1.0 } else { 0.0 });
        }
    }
    w.uuid(class::OPENNURBS4);
    w.bool(false);
    w.i32(70);
    w.u32(OPENNURBS_VERSION as u32);
    w.end();
    w.begin(tcode::ANONYMOUS_CHUNK);
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.count(pairs.len());
    for (key, value) in pairs {
        w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
        w.wstring(key);
        w.wstring(value);
        w.end();
    }
    w.end();
    w.end();
    w.end();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dimension a curve ends with, the last `i32` before its class end mark.
    fn last_dimension(write: fn(&mut Writer, &Curve), c: &Curve) -> i32 {
        let mut w = Writer::default();
        write(&mut w, c);
        // The class wrapper ends with a short chunk (a tcode and an 8-byte value); before it, the
        // data chunk ends with its CRC, and the dimension comes just before that.
        let at = w.out.len() - 12 - 4 - 4;
        i32::from_le_bytes(w.out[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn trim_curves_are_two_dimensional() {
        let line = Curve::Line {
            ends: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            domain: [0.0, 1.0],
        };
        assert_eq!(last_dimension(write_curve2, &line), 2);
        assert_eq!(last_dimension(write_curve, &line), 3);
    }
}
