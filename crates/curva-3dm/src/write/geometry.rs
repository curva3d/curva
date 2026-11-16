//! Curves, surfaces and polysurfaces as openNURBS reads them.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn write_curve(w: &mut Writer, c: &Curve) {
    write_curve_in(w, c, 3);
}

/// A trim curve, in a face's parameters.
pub(super) fn write_curve2(w: &mut Writer, c: &Curve) {
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

pub(super) fn write_nurbs_curve(w: &mut Writer, n: &NurbsCurve) {
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

pub(super) fn write_surface(w: &mut Writer, s: &Surface) {
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

pub(super) fn write_brep(w: &mut Writer, b: &Brep) {
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
