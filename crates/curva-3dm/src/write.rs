//! Writing a [`Model`] as a Rhino 7 `.3dm` file, as openNURBS writes one: the start, the
//! tables in their order, and the end mark with the file's size.

use crate::archive::{Uuid, Writer, tcode};
use crate::model::{
    Attributes, Block, Brep, Curve, Geometry, Group, Layer, Model, NurbsCurve, NurbsSurface,
    Object, P3, Plane, Source, Surface, View,
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
            write_view(w, v, i);
            w.end();
        }
        w.end();
    });
    for empty in [
        tcode::BITMAP_TABLE,
        tcode::TEXTURE_MAPPING_TABLE,
        tcode::MATERIAL_TABLE,
        tcode::LINETYPE_TABLE,
    ] {
        table(&mut w, empty, |_| {});
    }
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
    for empty in [
        tcode::FONT_TABLE,
        tcode::DIMSTYLE_TABLE,
        tcode::LIGHT_TABLE,
        tcode::HATCHPATTERN_TABLE,
    ] {
        table(&mut w, empty, |_| {});
    }
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
    w.i32(-1); // linetype: continuous
    w.color(UNSET_COLOR);
    w.f64(layer.print_width);
    w.bool(layer.locked);
    w.uuid(layer.id);
    w.uuid(layer.parent);
    w.bool(true); // expanded
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
    if !a.groups.is_empty() {
        w.u8(18);
        w.count(a.groups.len());
        for g in &a.groups {
            w.count(*g);
        }
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
fn write_view(w: &mut Writer, v: &View, index: usize) {
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
    w.uuid(Uuid::fresh(0x5649_4557 + index as u64));
    for locked in [false, false, false, true, true] {
        w.bool(locked);
    }
    w.f64s(&v.target);
    w.bool(true); // the camera frame is valid
    w.f64s(&[1.0, 1.0, 1.0]);
    w.end();
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
