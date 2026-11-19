//! Writing Curva's SubDs as Rhino's (`ON_SubD`), the layout `read/subd.rs` reads: the cage as
//! level 0, version 1.0 of the SubD record (no symmetry or texture packing), components in archive
//! version 70.
//!
//! Each point lists its edges and faces in order around it; each edge its two points and faces;
//! each face its edges, flagged where an edge runs backwards along it. A smooth edge ending at a
//! crease, corner or dart has its sector coefficient there marked as not yet worked out
//! (openNURBS' `UnsetSectorCoefficient`), which openNURBS accepts on a cage and works out itself;
//! everywhere else the coefficient is not used and is written as 0.

use crate::archive::Writer;
use curva_mesh::{Attribute, SubD, VertexTag};
use std::collections::HashMap;

/// openNURBS' mark for a sector coefficient still to be worked out.
const UNSET: f64 = -8883.0;
/// The byte that ends a component's additions.
const END: u8 = 255;

/// One `ON_SubD`, its class data.
pub(crate) fn write_subd(w: &mut Writer, subd: &SubD) {
    let cage = subd.cage();
    if cage.face_count() == 0 {
        w.u8(0);
        return;
    }
    w.u8(1);
    let (edges, edge_number, edge_faces) = edge_table(subd);
    let ids = Ids {
        nv: cage.vertex_count(),
        ne: edges.len(),
    };
    let nf = cage.face_count();
    w.begin_anonymous(0);
    w.u32(1); // one level: the cage
    w.u32(count(ids.nv));
    w.u32(count(ids.ne));
    w.u32(count(nf));
    w.f64s(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]); // the unused box
    w.begin_anonymous(1);
    w.u16(0);
    w.u8(4);
    w.u8(4);
    w.u8(4);
    let (lo, hi) = cage.bounds().unwrap_or(([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]));
    w.f64s(&lo);
    w.f64s(&hi);
    for p in [ids.vertex(0), ids.edge(0), ids.face(0), ids.face(nf)] {
        w.u32(p);
    }
    let around = rings(subd, &edges, &edge_number, &edge_faces);
    write_vertices(w, subd, &edges, &around, &ids);
    write_edges(w, subd, &edges, &edge_faces, &around, &ids);
    write_faces(w, subd, &edge_number, &ids);
    w.u8(0); // no saved surface mesh
    w.end();
    w.end();
}

/// Edges with their number by points, and the faces using each and which way.
type EdgeTable = (
    Vec<(u32, u32)>,
    HashMap<(u32, u32), usize>,
    Vec<Vec<(usize, bool)>>,
);

/// A count as written.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The archive numbers of points, edges and faces: points first, then edges, then faces.
struct Ids {
    nv: usize,
    ne: usize,
}

impl Ids {
    fn vertex(&self, v: usize) -> u32 {
        count(1 + v)
    }
    fn edge(&self, e: usize) -> u32 {
        count(1 + self.nv + e)
    }
    fn face(&self, f: usize) -> u32 {
        count(1 + self.nv + self.ne + f)
    }
}

/// Edges in a fixed order, lower point first, with the faces using each and which way.
fn edge_table(subd: &SubD) -> EdgeTable {
    let cage = subd.cage();
    let mut edges: Vec<(u32, u32)> = Vec::new();
    let mut edge_number: HashMap<(u32, u32), usize> = HashMap::new();
    let mut edge_faces: Vec<Vec<(usize, bool)>> = Vec::new();
    for (f, face) in cage.faces().enumerate() {
        for k in 0..face.len() {
            let (a, b) = (face[k], face[(k + 1) % face.len()]);
            let key = (a.min(b), a.max(b));
            let e = *edge_number.entry(key).or_insert_with(|| {
                edges.push(key);
                edge_faces.push(Vec::new());
                edges.len() - 1
            });
            edge_faces[e].push((f, a > b));
        }
    }
    (edges, edge_number, edge_faces)
}

/// Each point: its tag, place, and the edges and faces around it.
fn write_vertices(
    w: &mut Writer,
    subd: &SubD,
    edges: &[(u32, u32)],
    around: &[(Vec<usize>, Vec<usize>)],
    ids: &Ids,
) {
    for (v, p) in subd.cage().vertices().iter().enumerate() {
        base(w, ids.vertex(v), count(1 + v));
        let tag = match subd.vertex_tag(count(v)) {
            VertexTag::Smooth => 1,
            VertexTag::Crease => 2,
            VertexTag::Corner => 3,
            VertexTag::Dart => 4,
        };
        w.u8(tag);
        w.f64s(p);
        let (ring_edges, ring_faces) = &around[v];
        w.u16(u16::try_from(ring_edges.len()).unwrap_or(u16::MAX));
        w.u16(u16::try_from(ring_faces.len()).unwrap_or(u16::MAX));
        w.u8(0); // no saved limit points
        w.u16(u16::try_from(ring_edges.len()).unwrap_or(u16::MAX));
        for &e in ring_edges {
            w.u32(ids.edge(e));
            // Which end of the edge this point is.
            w.u8(u8::from(edges[e].1 == count(v)));
        }
        w.u16(u16::try_from(ring_faces.len()).unwrap_or(u16::MAX));
        for &f in ring_faces {
            w.u32(ids.face(f));
            w.u8(0);
        }
        w.u8(END);
    }
}

/// Each edge: crease or smooth, its sector coefficients and sharpness, its points and faces.
fn write_edges(
    w: &mut Writer,
    subd: &SubD,
    edges: &[(u32, u32)],
    edge_faces: &[Vec<(usize, bool)>],
    around: &[(Vec<usize>, Vec<usize>)],
    ids: &Ids,
) {
    for (e, &(a, b)) in edges.iter().enumerate() {
        base(w, ids.edge(e), count(1 + e));
        let sharpness = subd.sharpness(a, b);
        let crease = sharpness.is_crease();
        w.u8(if crease { 2 } else { 1 });
        w.u16(u16::try_from(edge_faces[e].len()).unwrap_or(u16::MAX));
        for end in [a, b] {
            let tagged = subd.vertex_tag(end) != VertexTag::Smooth;
            w.f64(if !crease && tagged {
                coefficient(subd, edges, &around[end as usize], end, e)
            } else {
                0.0
            });
        }
        w.f64(if crease { 0.0 } else { sharpness.value() });
        w.u16(2);
        for end in [a, b] {
            w.u32(ids.vertex(end as usize));
            w.u8(0);
        }
        w.u16(u16::try_from(edge_faces[e].len()).unwrap_or(u16::MAX));
        for &(f, backwards) in &edge_faces[e] {
            w.u32(ids.face(f));
            w.u8(u8::from(backwards));
        }
        w.u8(END);
    }
}

/// Each face: its edges and which way, its colour and its texture coordinates.
fn write_faces(w: &mut Writer, subd: &SubD, edge_number: &HashMap<(u32, u32), usize>, ids: &Ids) {
    let cage = subd.cage();
    let texture = match cage.texture() {
        Attribute::None => None,
        t => Some(t),
    };
    for f in 0..cage.face_count() {
        base(w, ids.face(f), count(1 + f));
        w.u32(count(1 + f)); // its level-0 face: itself
        w.u32(0);
        let face = cage.face(f);
        w.u16(u16::try_from(face.len()).unwrap_or(u16::MAX));
        w.u16(u16::try_from(face.len()).unwrap_or(u16::MAX));
        for k in 0..face.len() {
            let (a, b) = (face[k], face[(k + 1) % face.len()]);
            w.u32(ids.edge(edge_number[&(a.min(b), a.max(b))]));
            w.u8(u8::from(a > b));
        }
        w.u8(0); // no texture packing
        w.u8(0); // no material channel
        match subd.face_colours().and_then(|c| c[f]) {
            Some(c) => {
                w.u8(4);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let b = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
                w.color([b[0], b[1], b[2], 0]);
            }
            None => w.u8(0),
        }
        w.u8(0); // no pack id
        match texture {
            Some(t) => write_face_texture(w, cage, f, t),
            None => w.u8(0),
        }
        w.u8(END);
    }
}

/// A face's texture coordinates, as points in runs of ten.
fn write_face_texture(w: &mut Writer, cage: &curva_mesh::Mesh, f: usize, t: &Attribute<[f64; 2]>) {
    w.u8(4);
    let range = cage.face_range(f);
    let points: Vec<[f64; 3]> = range
        .map(|c| {
            let uv = t
                .at(c, cage.corners()[c] as usize)
                .copied()
                .unwrap_or_default();
            [uv[0], uv[1], 0.0]
        })
        .collect();
    w.u32(count(points.len() / 10));
    for run in points.chunks(10) {
        w.u8(u8::try_from(run.len() * 24).unwrap_or(u8::MAX));
        for p in run {
            w.f64s(p);
        }
    }
}

/// The start every component shares: archive number, id, level 0, and no additions.
fn base(w: &mut Writer, archive: u32, id: u32) {
    w.u32(archive);
    w.u32(id);
    w.u16(0);
    w.u8(0); // the unused 24-byte addition
    w.u8(0); // no group
    w.u8(0); // no symmetry
    w.u8(END);
}

/// For each point, its edges and faces in order around it, face `i` between edges `i` and
/// `i + 1`; on an open border the order starts at a border edge.
fn rings(
    subd: &SubD,
    edges: &[(u32, u32)],
    edge_number: &HashMap<(u32, u32), usize>,
    edge_faces: &[Vec<(usize, bool)>],
) -> Vec<(Vec<usize>, Vec<usize>)> {
    let cage = subd.cage();
    // For each point and each face at it, the edge coming in and the edge going out there.
    let mut at: Vec<Vec<(usize, usize, usize)>> = vec![Vec::new(); cage.vertex_count()];
    for (f, face) in cage.faces().enumerate() {
        let n = face.len();
        for k in 0..n {
            let (prev, here, next) = (face[(k + n - 1) % n], face[k], face[(k + 1) % n]);
            let e = |a: u32, b: u32| edge_number[&(a.min(b), a.max(b))];
            at[here as usize].push((f, e(prev, here), e(here, next)));
        }
    }
    at.iter()
        .map(|list| {
            if list.is_empty() {
                return (Vec::new(), Vec::new());
            }
            // Start where the incoming edge is a border edge, if any.
            let start = list
                .iter()
                .position(|&(_, incoming, _)| edge_faces[incoming].len() == 1)
                .unwrap_or(0);
            let mut ring_edges = vec![list[start].1];
            let mut ring_faces = Vec::new();
            let mut current = start;
            let mut seen = vec![false; list.len()];
            loop {
                seen[current] = true;
                let (f, _, outgoing) = list[current];
                ring_faces.push(f);
                // The next face shares the outgoing edge, as its incoming one.
                let next = list
                    .iter()
                    .enumerate()
                    .position(|(i, &(g, incoming, _))| !seen[i] && g != f && incoming == outgoing);
                if !ring_edges.contains(&outgoing) {
                    ring_edges.push(outgoing);
                }
                match next {
                    Some(i) => current = i,
                    None => break,
                }
            }
            // Faces the walk did not reach (a point where the surface pinches) follow after.
            for (i, &(f, incoming, outgoing)) in list.iter().enumerate() {
                if !seen[i] {
                    ring_faces.push(f);
                    for e in [incoming, outgoing] {
                        if !ring_edges.contains(&e) {
                            ring_edges.push(e);
                        }
                    }
                }
            }
            let _ = edges;
            (ring_edges, ring_faces)
        })
        .collect()
}

/// The sector coefficient at point `v` of the smooth edge `e`, as openNURBS works it out
/// (`opennurbs_subd_sector.cpp`): ½ + cos θ / 3, kept between ⅙ and ⅚, where θ is π shared among
/// the faces of the edge's sector at a crease, 2π among all the faces at a dart, and at a corner
/// the angle between the sector's two creases, rounded to whole 5° steps, shared among its faces.
fn coefficient(
    subd: &SubD,
    edges: &[(u32, u32)],
    (ring_edges, ring_faces): &(Vec<usize>, Vec<usize>),
    v: u32,
    e: usize,
) -> f64 {
    let creased = |i: usize| {
        let (a, b) = edges[ring_edges[i]];
        subd.sharpness(a, b).is_crease()
    };
    let closed = ring_edges.len() == ring_faces.len();
    let Some(j) = ring_edges.iter().position(|&x| x == e) else {
        return UNSET;
    };
    let n = ring_edges.len();
    // Walk both ways from the edge to the creases (or the open border) that bound its sector.
    let mut faces = 0;
    let mut left = j;
    let mut right = j;
    loop {
        if !closed && left == 0 {
            break;
        }
        let next = (left + n - 1) % n;
        faces += 1;
        left = next;
        if creased(left) || (closed && left == j) {
            break;
        }
    }
    loop {
        if !closed && right + 1 >= n {
            break;
        }
        faces += 1;
        right = (right + 1) % n;
        if creased(right) || (closed && right == j) {
            break;
        }
    }
    if closed && left == j {
        faces = ring_faces.len();
    }
    let faces = faces.max(1) as f64;
    let theta = match subd.vertex_tag(v) {
        VertexTag::Dart => std::f64::consts::TAU / ring_faces.len().max(2) as f64,
        VertexTag::Corner => {
            corner_angle(subd, edges, ring_edges[left], ring_edges[right], v) / faces
        }
        _ => std::f64::consts::PI / faces,
    };
    from_theta(theta)
}

/// The angle at `v` between two edges, as openNURBS rounds a corner's: to whole 5° steps, from 5°
/// to 355°.
fn corner_angle(subd: &SubD, edges: &[(u32, u32)], e0: usize, e1: usize, v: u32) -> f64 {
    let p = |i: u32| subd.cage().vertices()[i as usize];
    let other = |e: usize| {
        let (a, b) = edges[e];
        if a == v { b } else { a }
    };
    if e0 == e1 {
        return std::f64::consts::TAU - 5f64.to_radians();
    }
    let (c, a, b) = (p(v), p(other(e0)), p(other(e1)));
    let unit = |q: [f64; 3]| {
        let d = [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        d.map(|x| x / l)
    };
    let (a, b) = (unit(a), unit(b));
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sin = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let angle = sin.atan2(dot);
    let step = 5f64.to_radians();
    let index = (angle / step).round().clamp(1.0, 71.0);
    index * step
}

/// openNURBS' `SectorCoefficientFromTheta`.
fn from_theta(theta: f64) -> f64 {
    if !(theta > 0.0 && theta <= std::f64::consts::PI) {
        return UNSET;
    }
    let mut cos = theta.cos();
    if cos.abs() <= 1e-6 {
        cos = 0.0;
    } else if (cos.abs() - 0.5).abs() <= 1e-6 {
        cos = 0.5f64.copysign(cos);
    } else if cos.abs() + 1e-6 >= 1.0 {
        cos = 1f64.copysign(cos);
    }
    (0.5 + cos / 3.0).clamp(1.0 / 6.0, 5.0 / 6.0)
}
