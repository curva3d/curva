//! Curves, surfaces, polysurfaces and extrusions as openNURBS writes them.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A curve object written with `WriteObject`.
pub(super) fn curve(r: &mut Reader<'_>) -> Result<Curve> {
    let (id, mut data) = class_object(r)?;
    curve_of(id, &mut data)?.ok_or_else(|| bad(format!("a curve of an unknown kind {:02x?}", id.0)))
}

pub(super) fn curve_of(id: Uuid, r: &mut Reader<'_>) -> Result<Option<Curve>> {
    Ok(Some(match id {
        class::LINE_CURVE => {
            r.chunk_version()?;
            let ends = [point(r)?, point(r)?];
            let domain = interval(r)?;
            Curve::Line { ends, domain }
        }
        class::POLYLINE_CURVE => {
            r.chunk_version()?;
            let n = r.count(24)?;
            let points = (0..n).map(|_| point(r)).collect::<Result<Vec<_>>>()?;
            let m = r.count(8)?;
            let params = (0..m).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
            Curve::Polyline { points, params }
        }
        class::ARC_CURVE => {
            r.chunk_version()?;
            let plane = plane(r)?;
            let radius = r.f64()?;
            r.skip(72)?;
            let angles = interval(r)?;
            let domain = interval(r)?;
            Curve::Arc {
                plane,
                radius,
                angles,
                domain,
            }
        }
        class::NURBS_CURVE => Curve::Nurbs(nurbs_curve(r)?),
        class::POLY_CURVE => {
            r.chunk_version()?;
            let count = r.count(1)?;
            r.skip(8)?;
            bounding_box(r)?;
            let m = r.count(8)?;
            let params = (0..m).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
            let segments = (0..count).map(|_| curve(r)).collect::<Result<Vec<_>>>()?;
            Curve::Poly { segments, params }
        }
        _ => return Ok(None),
    }))
}

pub(super) fn nurbs_curve(r: &mut Reader<'_>) -> Result<NurbsCurve> {
    r.chunk_version()?;
    let dimension = usize::try_from(r.i32()?).map_err(|_| bad("a bad dimension"))?;
    let rational = r.i32()? != 0;
    let order = usize::try_from(r.i32()?).map_err(|_| bad("a bad order"))?;
    let count = usize::try_from(r.i32()?).map_err(|_| bad("a bad count"))?;
    if !(1..=3).contains(&dimension) || !(2..=20).contains(&order) || count < order {
        return Err(bad("a NURBS curve that cannot be"));
    }
    r.skip(8)?;
    bounding_box(r)?;
    let knot_count = r.count(8)?;
    let knots = (0..knot_count)
        .map(|_| r.f64())
        .collect::<Result<Vec<_>>>()?;
    let cv_count = r.count(8)?;
    let size = dimension + usize::from(rational);
    let mut points = Vec::with_capacity(cv_count);
    for _ in 0..cv_count {
        let mut p = [0.0, 0.0, 0.0, 1.0];
        for v in p.iter_mut().take(dimension) {
            *v = r.f64()?;
        }
        if rational {
            p[3] = r.f64()?;
        }
        points.push(p);
    }
    let _ = size;
    if knots.len() != count + order - 2 || points.len() != count {
        return Err(bad("a NURBS curve with the wrong number of knots"));
    }
    Ok(NurbsCurve {
        dimension,
        order,
        knots,
        points,
        rational,
    })
}

/// A surface object written with `WriteObject`.
fn surface(r: &mut Reader<'_>) -> Result<Surface> {
    let (id, mut data) = class_object(r)?;
    surface_of(id, &mut data)?.ok_or_else(|| bad("a surface of an unknown kind"))
}

/// A NURBS surface: its orders, knots and control points, checked to fit together.
fn nurbs_surface_in(r: &mut Reader<'_>) -> Result<Surface> {
    r.chunk_version()?;
    let _dimension = r.i32()?;
    let rational = r.i32()? != 0;
    let order = [r.i32()?, r.i32()?].map(|o| usize::try_from(o).unwrap_or(0));
    let count = [r.i32()?, r.i32()?].map(|c| usize::try_from(c).unwrap_or(0));
    if order.iter().any(|o| !(2..=20).contains(o)) || count[0] < order[0] || count[1] < order[1] {
        return Err(bad("a NURBS surface that cannot be"));
    }
    r.skip(8)?;
    bounding_box(r)?;
    let mut knots = [Vec::new(), Vec::new()];
    for k in &mut knots {
        let n = r.count(8)?;
        *k = (0..n).map(|_| r.f64()).collect::<Result<Vec<_>>>()?;
    }
    let n = r.count(8)?;
    if n != count[0] * count[1] {
        return Err(bad("a NURBS surface with the wrong number of points"));
    }
    let mut points = Vec::with_capacity(count[0]);
    for _ in 0..count[0] {
        let mut row = Vec::with_capacity(count[1]);
        for _ in 0..count[1] {
            let mut p = [0.0, 0.0, 0.0, 1.0];
            for v in p.iter_mut().take(3) {
                *v = r.f64()?;
            }
            if rational {
                p[3] = r.f64()?;
            }
            row.push(p);
        }
        points.push(row);
    }
    for d in 0..2 {
        if knots[d].len() != count[d] + order[d] - 2 {
            return Err(bad("a NURBS surface with the wrong number of knots"));
        }
    }
    Ok(Surface::Nurbs(NurbsSurface {
        order,
        knots,
        points,
        rational,
    }))
}

pub(super) fn surface_of(id: Uuid, r: &mut Reader<'_>) -> Result<Option<Surface>> {
    Ok(Some(match id {
        class::NURBS_SURFACE => nurbs_surface_in(r)?,
        class::PLANE_SURFACE => {
            let (_, minor) = r.chunk_version()?;
            let plane = plane(r)?;
            let domain = [interval(r)?, interval(r)?];
            let extents = if minor >= 1 {
                [interval(r)?, interval(r)?]
            } else {
                domain
            };
            Surface::Plane {
                plane,
                domain,
                extents,
            }
        }
        class::REV_SURFACE => {
            r.chunk_version()?;
            let axis = [point(r)?, point(r)?];
            let angles = interval(r)?;
            let domain = interval(r)?;
            bounding_box(r)?;
            let transposed = r.i32()? != 0;
            if r.u8()? == 0 {
                return Err(bad("a surface of revolution without its curve"));
            }
            let curve = Box::new(curve(r)?);
            Surface::Revolution {
                curve,
                axis,
                angles,
                domain,
                transposed,
            }
        }
        class::SUM_SURFACE => {
            r.chunk_version()?;
            let base = point(r)?;
            bounding_box(r)?;
            let a = curve(r)?;
            let b = curve(r)?;
            Surface::Sum {
                curves: [Box::new(a), Box::new(b)],
                base,
            }
        }
        _ => return Ok(None),
    }))
}

/// One of a B-rep's arrays of curves or surfaces: each is there or not.
fn optional_array<T>(
    r: &mut Reader<'_>,
    mut each: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<Vec<Option<T>>> {
    let mut chunk = r.expect(tcode::ANONYMOUS_CHUNK)?;
    let body = &mut chunk.body;
    body.chunk_version()?;
    let n = body.count(4)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if body.i32()? != 0 {
            out.push(Some(each(body)?));
        } else {
            out.push(None);
        }
    }
    Ok(out)
}

/// One of a B-rep's arrays of topology.
fn topology<'a, T>(
    r: &mut Reader<'a>,
    mut each: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut chunk = r.expect(tcode::ANONYMOUS_CHUNK)?;
    let body = &mut chunk.body;
    body.chunk_version()?;
    let n = body.count(4)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(each(body)?);
    }
    Ok(out)
}

fn ints(r: &mut Reader<'_>) -> Result<Vec<i32>> {
    let n = r.count(4)?;
    (0..n).map(|_| r.i32()).collect()
}

pub(super) fn brep(r: &mut Reader<'_>) -> Result<Brep> {
    let (major, _) = r.chunk_version()?;
    if major != 3 {
        return Err(bad("a B-rep from before Rhino 3"));
    }
    let curves2 = optional_array(r, curve)?;
    let curves3 = optional_array(r, curve)?;
    let surfaces = optional_array(r, surface)?;
    let after_3 = r.version >= 3;
    let vertices = topology(r, |r| brep_vertex(r))?;
    let edges = topology(r, |r| brep_edge(r, after_3))?;
    let trims = topology(r, |r| brep_trim(r, after_3))?;
    let loops = topology(r, |r| brep_loop(r))?;
    let faces = topology(r, |r| brep_face(r))?;
    Ok(Brep {
        curves2,
        curves3,
        surfaces,
        vertices,
        edges,
        trims,
        loops,
        faces,
    })
}

/// A B-rep vertex: its point, its edges and its tolerance.
fn brep_vertex(r: &mut Reader<'_>) -> Result<BrepVertex> {
    let _index = r.i32()?;
    let point = point(r)?;
    let edges = ints(r)?;
    let tolerance = r.f64()?;
    Ok(BrepVertex {
        point,
        edges,
        tolerance,
    })
}

/// A B-rep edge: its curve, which way it runs, its ends, its trims; its domain from Rhino 3 on.
fn brep_edge(r: &mut Reader<'_>, after_3: bool) -> Result<BrepEdge> {
    let _index = r.i32()?;
    let curve = r.i32()?;
    let reversed = r.i32()? != 0;
    let proxy_domain = interval(r)?;
    let vertices = [r.i32()?, r.i32()?];
    let trims = ints(r)?;
    let tolerance = r.f64()?;
    let domain = if after_3 { interval(r)? } else { proxy_domain };
    Ok(BrepEdge {
        curve,
        reversed,
        proxy_domain,
        domain,
        vertices,
        trims,
        tolerance,
    })
}

/// A B-rep trim: its curve, its edge, its ends, its kind and loop; its domain and whether it runs back from Rhino 3 on.
fn brep_trim(r: &mut Reader<'_>, after_3: bool) -> Result<BrepTrim> {
    let _index = r.i32()?;
    let curve = r.i32()?;
    let proxy_domain = interval(r)?;
    let edge = r.i32()?;
    let vertices = [r.i32()?, r.i32()?];
    let reversed3d = r.i32()? != 0;
    let kind = r.i32()?;
    let iso = r.i32()?;
    let loop_index = r.i32()?;
    let tolerance = r.f64s::<2>()?;
    let (domain, proxy_reversed) = if after_3 {
        let domain = interval(r)?;
        let flags = r.bytes(8)?;
        r.skip(24)?;
        (domain, flags[0] == 1)
    } else {
        r.skip(48)?;
        (proxy_domain, false)
    };
    r.skip(16)?;
    Ok(BrepTrim {
        curve,
        proxy_domain,
        domain,
        proxy_reversed,
        edge,
        vertices,
        reversed3d,
        kind,
        iso,
        loop_index,
        tolerance,
    })
}

/// A B-rep loop: its trims, its kind and its face.
fn brep_loop(r: &mut Reader<'_>) -> Result<BrepLoop> {
    let _index = r.i32()?;
    let trims = ints(r)?;
    let kind = r.i32()?;
    let face = r.i32()?;
    Ok(BrepLoop { trims, kind, face })
}

/// A B-rep face: its loops, its surface and whether it is turned over.
fn brep_face(r: &mut Reader<'_>) -> Result<BrepFace> {
    let _index = r.i32()?;
    let loops = ints(r)?;
    let surface = r.i32()?;
    let reversed = r.i32()? != 0;
    let _material = r.i32()?;
    Ok(BrepFace {
        loops,
        surface,
        reversed,
    })
}

pub(super) fn extrusion(r: &mut Reader<'_>) -> Result<Extrusion> {
    let (_, minor, mut body) = r.versioned(tcode::ANONYMOUS_CHUNK)?;
    let profile = curve(&mut body)?;
    let line = [point(&mut body)?, point(&mut body)?];
    let t = interval(&mut body)?;
    let up = point(&mut body)?;
    body.skip(2 + 48)?;
    let _path_domain = interval(&mut body)?;
    let _transposed = body.bool()?;
    let mut count = 1;
    let mut caps = [false, false];
    if minor >= 1 {
        count = body.i32()?;
        if minor >= 2 {
            caps = [body.bool()?, body.bool()?];
        }
    }
    let profiles = match profile {
        Curve::Poly { segments, .. } if count > 1 => segments,
        other => vec![other],
    };
    if minor < 2 {
        let closed = true;
        caps = [closed, closed];
    }
    let at = |s: f64| -> P3 { std::array::from_fn(|k| line[0][k] + s * (line[1][k] - line[0][k])) };
    Ok(Extrusion {
        path: [at(t[0]), at(t[1])],
        up,
        profiles,
        caps,
    })
}
