//! Writing a [`Model`] as a Rhino 7 `.3dm` file, as openNURBS writes one: the start, the
//! tables in their order, and the end mark with the file's size.

use crate::archive::{Uuid, Writer, tcode};
use crate::model::{
    AngularDimension, Attributes, Block, Brep, Curve, Detail, DimStyle, Geometry, Group, HatchLine,
    HatchPattern, Layer, Leader, LinearDimension, Linetype, Model, NurbsCurve, NurbsSurface,
    Object, P3, Plane, RadialDimension, Source, Surface, Text, View,
};
use crate::read::class;

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
            class_object(w, class::LAYER, |w| write_layer(w, layer, index));
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

fn color(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 0]
}

/// openNURBS' unset colour.
const UNSET_COLOR: [u8; 4] = [255, 255, 255, 255];

fn write_layer(w: &mut Writer, layer: &Layer, index: usize) {
    w.chunk_version(1, 6);
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
        | Geometry::Leader(_) => 0x200,
        Geometry::Detail(_) => 0x8000,
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
        Geometry::Leader(l) => class_object(w, class::LEADER, |w| write_leader(w, l)),
        Geometry::Detail(d) => class_object(w, class::DETAIL, |w| write_detail(w, d)),
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
    w.chunk_version(2, 1);
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

fn write_plane(w: &mut Writer, p: &Plane) {
    w.f64s(&p.origin);
    w.f64s(&p.x);
    w.f64s(&p.y);
    w.f64s(&p.z);
    w.f64s(&p.equation());
}

/// A construction plane as openNURBS writes one, with Rhino's default grid.
fn write_cplane(w: &mut Writer, plane: &Plane, name: &str) {
    w.chunk_version(1, 1);
    write_plane(w, plane);
    w.f64(1.0); // grid spacing
    w.f64(1.0); // snap spacing
    w.i32(70); // grid lines
    w.i32(5); // every fifth line thick
    w.wstring(name);
    w.bool(false); // not drawn into the depth buffer
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: P3) -> P3 {
    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-300);
    a.map(|v| v / length)
}

/// A named view: its viewport (camera and frustum), the construction plane its kind of view
/// has, its target and its name.
/// A view: its camera, construction plane, target and name, and for a layout or a detail its
/// kind (1 a page, 2 a view nested in a page) and paper size in millimetres.
fn write_view(
    w: &mut Writer,
    v: &View,
    id: Uuid,
    kind: Option<(i32, [f64; 2])>,
    user_text: &[(String, String)],
) {
    let direction = unit(v.direction);
    let x = unit(cross(direction, v.up));
    let y = cross(x, direction);
    w.begin(tcode::VIEW_VIEWPORT);
    w.chunk_version(1, 5);
    w.i32(1); // camera valid
    w.i32(1); // frustum valid
    w.i32(1); // port valid
    w.i32(if v.parallel { 1 } else { 2 });
    w.f64s(&v.location);
    w.f64s(&direction);
    w.f64s(&y);
    w.f64s(&x);
    w.f64s(&y);
    w.f64s(&direction.map(|c| -c));
    w.f64s(&v.frustum);
    // A square port, as the frustum is: left, right, bottom, top (pixels run down), near, far.
    for p in [0, 1000, 1000, 0, 0, 255] {
        w.i32(p);
    }
    w.uuid(id);
    for locked in [false, false, false, true, true] {
        w.bool(locked);
    }
    w.f64s(&v.target);
    w.bool(true); // the camera frame is valid
    w.f64s(&[1.0, 1.0, 1.0]);
    w.end();
    // A layout's user text, in its viewport's user data.
    if !user_text.is_empty() {
        w.begin(tcode::VIEW_VIEWPORT_USERDATA);
        write_user_text(w, user_text);
        w.short(tcode::OPENNURBS_CLASS_END, 0);
        w.end();
    }
    // A flat view looking along an axis draws on that axis' plane, as Rhino's standard views do.
    let along = |axis: usize, sign: f64| v.parallel && direction[axis] * sign > 1.0 - 1e-9;
    let (px, py) = if along(1, 1.0) {
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
    } else if along(0, -1.0) {
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
    } else {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    };
    let plane = Plane {
        origin: [0.0; 3],
        x: px,
        y: py,
        z: cross(px, py),
    };
    w.begin(tcode::VIEW_CPLANE);
    write_cplane(w, &plane, "");
    w.end();
    w.begin(tcode::VIEW_TARGET);
    w.f64s(&v.target);
    w.end();
    w.begin(tcode::VIEW_NAME);
    w.wstring(&v.name);
    w.end();
    if let Some((kind, size)) = kind {
        // Version 1.1 of the view's attributes: kind, paper size, then an unused parent id
        // and nested position.
        w.begin(tcode::VIEW_ATTRIBUTES);
        w.chunk_version(1, 1);
        w.i32(kind);
        w.f64s(&size);
        w.uuid(Uuid::NIL);
        w.f64s(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
        w.end();
    }
    w.short(tcode::ENDOFTABLE, 0);
}

fn write_curve(w: &mut Writer, c: &Curve) {
    write_curve_in(w, c, 3);
}

/// A trim curve, in a face's parameters.
fn write_curve2(w: &mut Writer, c: &Curve) {
    write_curve_in(w, c, 2);
}

/// A curve of dimension `dim` (2 for trims, 3 otherwise; NURBS curves carry their own).
fn write_curve_in(w: &mut Writer, c: &Curve, dim: i32) {
    match c {
        Curve::Line { ends, domain } => class_object(w, class::LINE_CURVE, |w| {
            w.chunk_version(1, 0);
            w.f64s(&ends[0]);
            w.f64s(&ends[1]);
            w.f64s(domain);
            w.i32(dim);
        }),
        Curve::Polyline { points, params } => class_object(w, class::POLYLINE_CURVE, |w| {
            w.chunk_version(1, 0);
            w.count(points.len());
            for p in points {
                w.f64s(p);
            }
            w.count(params.len());
            w.f64s(params);
            w.i32(dim);
        }),
        Curve::Arc {
            plane,
            radius,
            angles,
            domain,
        } => class_object(w, class::ARC_CURVE, |w| {
            w.chunk_version(1, 0);
            write_plane(w, plane);
            w.f64(*radius);
            for a in [0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI] {
                let p: P3 = std::array::from_fn(|k| {
                    plane.origin[k] + radius * (a.cos() * plane.x[k] + a.sin() * plane.y[k])
                });
                w.f64s(&p);
            }
            w.f64s(angles);
            w.f64s(domain);
            w.i32(dim);
        }),
        Curve::Nurbs(n) => class_object(w, class::NURBS_CURVE, |w| write_nurbs_curve(w, n)),
        Curve::Poly { segments, params } => class_object(w, class::POLY_CURVE, |w| {
            w.chunk_version(1, 0);
            w.count(segments.len());
            w.i32(0);
            w.i32(0);
            empty_box(w);
            w.count(params.len());
            w.f64s(params);
            for s in segments {
                write_curve_in(w, s, dim);
            }
        }),
    }
}

fn write_nurbs_curve(w: &mut Writer, n: &NurbsCurve) {
    w.chunk_version(1, 1);
    w.count(n.dimension);
    w.i32(i32::from(n.rational));
    w.count(n.order);
    w.count(n.points.len());
    w.i32(0);
    w.i32(0);
    empty_box(w);
    w.count(n.knots.len());
    w.f64s(&n.knots);
    w.count(n.points.len());
    for p in &n.points {
        w.f64s(&p[..n.dimension]);
        if n.rational {
            w.f64(p[3]);
        }
    }
    w.bool(false); // not SubD-friendly
}

fn write_nurbs_surface(w: &mut Writer, s: &NurbsSurface) {
    w.chunk_version(1, 0);
    w.i32(3);
    w.i32(i32::from(s.rational));
    w.count(s.order[0]);
    w.count(s.order[1]);
    w.count(s.points.len());
    w.count(s.points.first().map_or(0, Vec::len));
    w.i32(0);
    w.i32(0);
    empty_box(w);
    for k in &s.knots {
        w.count(k.len());
        w.f64s(k);
    }
    w.count(s.points.len() * s.points.first().map_or(0, Vec::len));
    for row in &s.points {
        for p in row {
            w.f64s(&p[..3]);
            if s.rational {
                w.f64(p[3]);
            }
        }
    }
}

fn write_surface(w: &mut Writer, s: &Surface) {
    match s {
        Surface::Nurbs(n) => class_object(w, class::NURBS_SURFACE, |w| write_nurbs_surface(w, n)),
        Surface::Plane {
            plane,
            domain,
            extents,
        } => class_object(w, class::PLANE_SURFACE, |w| {
            w.chunk_version(1, 1);
            write_plane(w, plane);
            w.f64s(&domain[0]);
            w.f64s(&domain[1]);
            w.f64s(&extents[0]);
            w.f64s(&extents[1]);
        }),
        // Curva writes every other surface as NURBS, so these do not come here.
        Surface::Revolution { .. } | Surface::Sum { .. } => {}
    }
}

/// One of a B-rep's arrays of curves or surfaces.
fn optional_array<T>(w: &mut Writer, items: &[Option<T>], each: impl Fn(&mut Writer, &T)) {
    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 0);
    w.count(items.len());
    for item in items {
        match item {
            Some(item) => {
                w.i32(1);
                each(w, item);
            }
            None => w.i32(0),
        }
    }
    w.end();
}

fn ints(w: &mut Writer, list: &[i32]) {
    w.count(list.len());
    for i in list {
        w.i32(*i);
    }
}

fn write_brep(w: &mut Writer, b: &Brep) {
    w.chunk_version(3, 3);
    optional_array(w, &b.curves2, write_curve2);
    optional_array(w, &b.curves3, write_curve);
    optional_array(w, &b.surfaces, write_surface);

    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 0);
    w.count(b.vertices.len());
    for (i, v) in b.vertices.iter().enumerate() {
        w.count(i);
        w.f64s(&v.point);
        ints(w, &v.edges);
        w.f64(v.tolerance);
    }
    w.end();

    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 0);
    w.count(b.edges.len());
    for (i, e) in b.edges.iter().enumerate() {
        w.count(i);
        w.i32(e.curve);
        w.i32(i32::from(e.reversed));
        w.f64s(&e.proxy_domain);
        w.i32(e.vertices[0]);
        w.i32(e.vertices[1]);
        ints(w, &e.trims);
        w.f64(e.tolerance);
        w.f64s(&e.domain);
    }
    w.end();

    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 0);
    w.count(b.trims.len());
    for (i, t) in b.trims.iter().enumerate() {
        w.count(i);
        w.i32(t.curve);
        w.f64s(&t.proxy_domain);
        w.i32(t.edge);
        w.i32(t.vertices[0]);
        w.i32(t.vertices[1]);
        w.i32(i32::from(t.reversed3d));
        w.i32(t.kind);
        w.i32(t.iso);
        w.i32(t.loop_index);
        w.f64s(&t.tolerance);
        w.f64s(&t.domain);
        let mut flags = [0u8; 8];
        flags[0] = u8::from(t.proxy_reversed);
        w.out.extend_from_slice(&flags);
        w.out.extend_from_slice(&[0u8; 24]);
        w.f64(0.0);
        w.f64(0.0);
    }
    w.end();

    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 0);
    w.count(b.loops.len());
    for (i, l) in b.loops.iter().enumerate() {
        w.count(i);
        ints(w, &l.trims);
        w.i32(l.kind);
        w.i32(l.face);
    }
    w.end();

    w.begin(tcode::ANONYMOUS_CHUNK);
    w.chunk_version(1, 2);
    w.count(b.faces.len());
    for (i, f) in b.faces.iter().enumerate() {
        w.count(i);
        ints(w, &f.loops);
        w.i32(f.surface);
        w.i32(i32::from(f.reversed));
        w.i32(0); // material channel
    }
    for i in 0..b.faces.len() {
        w.uuid(Uuid::fresh(i as u64));
    }
    w.bool(false); // no per-face colours
    w.end();

    // The bounding box, from the vertices; Rhino works out its own.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in &b.vertices {
        for k in 0..3 {
            lo[k] = lo[k].min(v.point[k]);
            hi[k] = hi[k].max(v.point[k]);
        }
    }
    if b.vertices.is_empty() {
        lo = [1.0, 0.0, 0.0];
        hi = [-1.0, 0.0, 0.0];
    }
    w.f64s(&lo);
    w.f64s(&hi);
    // No render or analysis meshes.
    for _ in 0..2 {
        w.begin(tcode::ANONYMOUS_CHUNK);
        for _ in 0..b.faces.len() {
            w.u8(0);
        }
        w.end();
    }
    w.i32(0); // solidity unknown: Rhino works it out
    w.begin_anonymous(1);
    w.bool(false); // no region topology
    w.end();
}

/// A hatch pattern as Rhino 6 and later write it (`ON_HatchPattern::Write`): its id, index and
/// name, fill type, description, then its lines, each in a chunk of its own.
fn write_hatch_pattern(w: &mut Writer, pattern: &HatchPattern, index: usize) {
    w.begin_anonymous(0);
    w.begin_anonymous(0);
    w.u32(0x01 | 0x04 | 0x08); // id, index and name follow
    w.uuid(pattern.id);
    w.count(index);
    w.wstring(&pattern.name);
    w.end();
    w.u32(u32::from(!pattern.solid));
    w.wstring(&pattern.description);
    w.begin(tcode::ANONYMOUS_CHUNK);
    let lines: &[HatchLine] = if pattern.solid { &[] } else { &pattern.lines };
    w.count(lines.len());
    for line in lines {
        w.begin_anonymous(0);
        w.f64(line.angle);
        w.f64s(&line.base);
        w.f64s(&line.offset);
        w.count(line.dashes.len());
        w.f64s(&line.dashes);
        w.end();
    }
    w.end();
    w.end();
}

/// A length as openNURBS writes one (`ON_LengthValue::Write`): a plain number, no unit.
fn write_length_value(w: &mut Writer, value: f64) {
    w.begin_anonymous(1);
    w.f64(value);
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0); // ON_UnitSystem: none
    w.u32(0);
    w.f64(1.0);
    w.wstring("");
    w.end();
    w.u32(0); // angle unit: none
    w.i32(0); // locale
    w.wstring("1");
    w.u32(0); // exact decimal
    w.end();
}

/// An annotation style as Rhino 6 and later read it (`ON_DimStyle::Read`), written to version
/// 1.1: everything up to the notes' alignment and the style's scale, with Rhino's usual values
/// for what Curva does not set; Rhino fills in the fields of later versions with its defaults.
fn write_dim_style(w: &mut Writer, style: &DimStyle, index: usize) {
    // Version 1.2 adds the font; without one, 1.1 leaves Rhino its default.
    w.begin_versioned(
        tcode::ANONYMOUS_CHUNK,
        1,
        if style.font.is_some() { 2 } else { 1 },
    );
    w.begin_versioned(tcode::MODEL_ATTRIBUTES_CHUNK, 1, 0);
    w.u8(0); // model serial number
    w.u8(1);
    w.uuid(style.id);
    w.u8(0); // component type
    w.u8(1);
    w.count(index);
    w.u8(1);
    w.wstring(&style.name);
    w.end();
    let h = style.text_height;
    // Extension line extension and offset, arrow and leader arrow sizes, centre mark, text gap,
    // text height.
    w.f64s(&[
        style.extension_extension,
        style.extension_offset,
        style.arrow_size,
        style.arrow_size,
        h,
        style.text_gap,
        h,
    ]);
    w.i32(0); // text alignment (obsolete)
    w.i32(0); // angle format: decimal degrees
    w.i32(0); // length format: decimal
    w.i32(0); // angle resolution
    w.i32(style.length_resolution);
    w.i32(-2_147_483_647); // ignored text style index
    w.f64(1.0); // length factor
    w.bool(false); // alternate units
    w.f64(1.0);
    w.i32(0);
    w.i32(2);
    for _ in 0..4 {
        w.wstring(""); // prefixes and suffixes
    }
    w.f64(0.0); // dimension line extension
    w.bool(false);
    w.bool(false);
    w.uuid(Uuid::NIL); // no parent style
    w.i32(0);
    w.bool(false); // no overridden fields
    w.i32(0); // tolerance format
    w.i32(4);
    w.f64s(&[0.0, 0.0, 1.0, 3.0 * h]);
    w.bool(false); // no text mask
    w.i32(0);
    w.color([255, 255, 255, 0]);
    w.f64(1.0); // dimension scale
    w.i32(0);
    w.uuid(Uuid::NIL);
    for _ in 0..2 {
        for _ in 0..4 {
            w.u8(0); // colour sources: by layer
        }
        for _ in 0..4 {
            w.color([0, 0, 0, 0]);
        }
    }
    w.u8(0); // plot weight sources
    w.u8(0);
    w.f64s(&[0.0, 0.0]);
    w.f64(1.0); // fixed extension length
    w.bool(false);
    w.f64(0.0); // text rotation
    w.i32(4);
    w.f64(1.0);
    w.bool(false);
    w.bool(false);
    w.i32(0);
    w.i32(0);
    w.f64(0.7); // stack height
    w.i32(0);
    w.f64s(&[0.0, 0.0, 0.0]); // roundings
    for _ in 0..4 {
        w.i32(0); // zero suppression
    }
    w.bool(false);
    w.i32(1); // arrows: solid triangles
    w.i32(1);
    w.i32(1);
    for _ in 0..3 {
        w.uuid(Uuid::NIL);
    }
    // Version 1.1: leaders, then the notes' alignment in Rhino 5's numbering.
    w.i32(1);
    w.u32(style.v_align);
    w.u32(style.v_align);
    w.i32(0);
    w.i32(1);
    w.f64(0.0);
    w.bool(true);
    w.f64(h);
    w.u32(style.h_align);
    w.u32(style.h_align);
    w.bool(true); // draw forward
    w.bool(false);
    // The style's scale, one to one (`ON_ScaleValue::Write`).
    w.begin_anonymous(1);
    w.f64(1.0);
    w.f64(1.0);
    w.i32(0);
    w.u32(0);
    w.u32(0);
    w.wstring("1:1");
    write_length_value(w, 1.0);
    write_length_value(w, 1.0);
    w.u32(1); // ratio format
    w.end();
    w.u32(0); // unit system: none (the model's)
    if let Some(font) = &style.font {
        write_font(w, font);
    }
    w.end();
}

/// A font as openNURBS writes one (`ON_Font::Write`, version 1.0, the least Rhino reads): its
/// characteristics (normal weight, upright, medium stretch, as `FontCharacteristicsAsUnsigned`
/// makes 429), its Windows name as a UTF-8 string chunk, and no PostScript name. Rhino finds
/// the installed font by the name.
fn write_font(w: &mut Writer, family: &str) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.u32(429);
    w.begin(tcode::UTF8_STRING_CHUNK);
    w.u8(1); // uncompressed
    for byte in family.as_bytes() {
        w.u8(*byte);
    }
    w.end();
    w.wstring("");
    w.end();
}

/// The part every annotation shares, as Rhino 6 and later write it
/// (`ON_Annotation::Internal_WriteAnnotation`, content version 4): its words, style, plane,
/// kind (openNURBS' annotation type), no overrides, the horizontal direction and text scaling.
fn write_annotation_core(
    w: &mut Writer,
    rich: &str,
    rotation: f64,
    style: Uuid,
    plane: &Plane,
    kind: u32,
) {
    w.begin_anonymous(4);
    w.begin_anonymous(0);
    w.wstring(&rich.replace('\n', "\r\n"));
    write_plane(
        w,
        &Plane {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
    );
    w.f64(0.0); // no wrapping width
    w.f64(rotation);
    w.u32(0);
    w.u32(0);
    w.f64(1.0); // obsolete height
    w.bool(false); // not wrapped
    w.end();
    w.uuid(style);
    write_plane(w, plane);
    w.u32(kind);
    w.begin_anonymous(1);
    w.bool(false); // no overrides
    w.end();
    w.f64s(&[1.0, 0.0]); // horizontal direction
    w.bool(true); // text scales with the model
    w.end();
}

/// A note (`ON_Text::Write`).
fn write_text(w: &mut Writer, t: &Text) {
    w.begin_anonymous(0);
    write_annotation_core(w, &t.rich, t.rotation, t.style, &t.plane, 9);
    w.end();
}

/// A linear dimension (`ON_DimLinear::Write` around `ON_Dimension::Internal_WriteDimension`,
/// content version 1): the annotation, its typed text, its text at the usual place, arrows as
/// they are, measured in the model, then its second point and dimension line point.
fn write_linear_dimension(w: &mut Writer, d: &LinearDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(w, "", 0.0, d.style, &d.plane, if d.rotated { 5 } else { 1 });
    w.wstring(&d.text);
    w.f64(0.0); // obsolete text rotation
    w.bool(true); // text at its usual place
    w.f64s(&[0.0, 0.0]);
    w.bool(false); // arrows not flipped
    w.bool(false);
    w.u32(0); // arrow fit: automatic
    w.uuid(Uuid::NIL); // not measured through a detail
    w.f64(1.0); // distance scale
    w.u32(0); // text fit: automatic
    w.end();
    w.f64s(&d.second);
    w.f64s(&d.line);
    w.end();
}

/// The part dimensions share after the annotation (`ON_Dimension::Internal_WriteDimension`,
/// content version 1): typed text at its usual place, arrows as they are, measured in the model.
fn write_dimension_core(w: &mut Writer, text: &str) {
    w.wstring(text);
    w.f64(0.0); // obsolete text rotation
    w.bool(true); // text at its usual place
    w.f64s(&[0.0, 0.0]);
    w.bool(false); // arrows not flipped
    w.bool(false);
    w.u32(0); // arrow fit: automatic
    w.uuid(Uuid::NIL); // not measured through a detail
    w.f64(1.0); // distance scale
    w.u32(0); // text fit: automatic
}

/// A radius or diameter dimension (`ON_DimRadial::Write`).
fn write_radial_dimension(w: &mut Writer, d: &RadialDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(
        w,
        "",
        0.0,
        d.style,
        &d.plane,
        if d.diameter { 3 } else { 4 },
    );
    write_dimension_core(w, &d.text);
    w.end();
    w.f64s(&d.radius_point);
    w.f64s(&d.text_point);
    w.end();
}

/// An angle dimension (`ON_DimAngular::Write`).
fn write_angular_dimension(w: &mut Writer, d: &AngularDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(w, "", 0.0, d.style, &d.plane, 2);
    write_dimension_core(w, &d.text);
    w.end();
    w.f64s(&d.sides[0]);
    w.f64s(&d.sides[1]);
    w.f64s(&d.offsets);
    w.f64s(&d.arc_point);
    w.end();
}

/// A leader (`ON_Leader::Write`, version 1): the annotation with its words, then its points.
fn write_leader(w: &mut Writer, l: &Leader) {
    w.begin_anonymous(1);
    write_annotation_core(w, &l.rich, 0.0, l.style, &l.plane, 10);
    w.count(l.points.len());
    for p in &l.points {
        w.f64s(p);
    }
    w.end();
}

/// A detail view (`ON_DetailView::Write`, version 1.1): its camera as a view nested in its
/// layout, its outline, and its scale.
fn write_detail(w: &mut Writer, d: &Detail) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 1);
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    write_view(w, &d.view, Uuid::NIL, Some((2, [0.0, 0.0])), &[]);
    w.end();
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    write_nurbs_curve(w, &d.boundary);
    w.end();
    w.f64(d.ratio);
    w.end();
}

/// The settings' view list: Rhino's four standard model views, then a page view for each
/// layout, at its paper size, looking down on it.
fn write_view_list(w: &mut Writer, model: &Model) {
    let span = 1000.0;
    let standard = |name: &str, direction: P3, up: P3, parallel: bool| View {
        name: name.to_owned(),
        location: direction.map(|c| -c * span * 10.0),
        direction,
        up,
        target: [0.0; 3],
        parallel,
        frustum: if parallel {
            [-span, span, -span, span, 1.0, span * 100.0]
        } else {
            [-0.36, 0.36, -0.36, 0.36, 1.0, span * 100.0]
        },
    };
    let s = 1.0 / 3f64.sqrt();
    let views = [
        standard("Top", [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], true),
        standard("Front", [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], true),
        standard("Right", [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], true),
        standard("Perspective", [-s, s, -s], [-s, s, 2.0 * s], false),
    ];
    w.begin(tcode::SETTINGS_VIEW_LIST);
    w.count(views.len() + model.pages.len());
    for (i, v) in views.iter().enumerate() {
        w.begin(tcode::VIEW_RECORD);
        write_view(
            w,
            v,
            Uuid::fresh(0x5354_4456 + i as u64),
            Some((0, [0.0, 0.0])),
            &[],
        );
        w.end();
    }
    for page in &model.pages {
        let [width, height] = page.size_mm;
        let centre = [width / 2.0, height / 2.0, 0.0];
        let view = View {
            name: page.name.clone(),
            location: [centre[0], centre[1], 100.0],
            direction: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            target: centre,
            parallel: true,
            frustum: [
                -width / 2.0,
                width / 2.0,
                -height / 2.0,
                height / 2.0,
                1.0,
                200.0,
            ],
        };
        w.begin(tcode::VIEW_RECORD);
        write_view(w, &view, page.id, Some((1, page.size_mm)), &page.user_text);
        w.end();
    }
    w.end();
}

/// The settings' attributes, to version 1.1: the usual linetype scale, plot colour and current
/// linetype, then the unit layouts are drawn in.
fn write_page_unit(w: &mut Writer, model: &Model) {
    let unit = model.page_unit.unwrap_or(model.unit);
    w.begin(tcode::SETTINGS_ATTRIBUTES);
    w.chunk_version(1, 1);
    w.f64(1.0); // linetype display scale
    w.color([0, 0, 0, 0]); // plot colour
    w.i32(0); // plot colour from the layer
    w.i32(-1); // no current linetype
    w.i32(0); // linetype from the layer
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.i32(102);
    w.u32(unit);
    w.f64(0.001);
    w.f64(1f64.to_radians());
    w.f64(0.01);
    w.i32(0);
    w.i32(3);
    w.f64(metres_per(unit));
    w.wstring("");
    w.end();
    w.end();
}

/// The document user text as openNURBS keeps it (`ONX_Model`'s step 17): a user table for the
/// plug-in `ON_DocumentUserStringList`, whose record is that object, with the keys and values as
/// its user data.
fn write_document_text(w: &mut Writer, pairs: &[(String, String)]) {
    w.begin(tcode::USER_TABLE);
    w.begin(tcode::USER_TABLE_UUID);
    w.uuid(class::DOCUMENT_USER_STRINGS);
    w.begin_versioned(tcode::USER_TABLE_RECORD_HEADER, 1, 0);
    w.bool(false); // not kept for a plug-in that is missing
    w.i32(70);
    w.u32(OPENNURBS_VERSION as u32);
    w.end();
    w.end();
    w.begin(tcode::USER_RECORD);
    w.begin(tcode::OPENNURBS_CLASS);
    w.begin(tcode::OPENNURBS_CLASS_UUID);
    w.uuid(class::DOCUMENT_USER_STRINGS);
    w.end();
    w.begin(tcode::OPENNURBS_CLASS_DATA);
    w.u8(1);
    w.end();
    write_user_text(w, pairs);
    w.short(tcode::OPENNURBS_CLASS_END, 0);
    w.end();
    w.end();
    w.short(tcode::ENDOFTABLE, 0);
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
