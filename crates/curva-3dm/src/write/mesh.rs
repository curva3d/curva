//! Writing Curva's meshes as Rhino's (`ON_Mesh`, version 3.8, as Rhino 6 to 8 write them).
//!
//! The layout is openNURBS' `ON_Mesh::Write` (`opennurbs_mesh.cpp`). Rhino keeps normals, texture
//! coordinates and colours once per point, so a point whose corners carry different values (a
//! hard edge, a texture seam) is written once for each, as Rhino keeps them. Faces of more than
//! four corners are written as Rhino's n-gons: their triangles, and the outline that groups them.
//! Points are written at double precision as well as Rhino's single, so nothing is rounded.

use crate::archive::{Uuid, Writer};
use curva_mesh::{Mesh, Point};
use std::collections::HashMap;

/// A point with its data, as bits, so equal values compare equal.
type Key = (u32, [u64; 3], [u64; 2], [u32; 3]);

/// A point to write: its vertex, when first met, and its normal, texture coordinate and colour.
type Written = (u32, usize, Point, [f64; 2], [f32; 3]);

/// The mesh as Rhino stores it: points with their data, and faces of four point numbers.
struct Flat {
    points: Vec<Point>,
    normals: Option<Vec<Point>>,
    texture: Option<Vec<[f64; 2]>>,
    colours: Option<Vec<[f32; 3]>>,
    quads: Vec<[u32; 4]>,
    /// Each n-gon's outline and its faces in `quads`.
    ngons: Vec<(Vec<u32>, Vec<u32>)>,
}

fn flatten(mesh: &Mesh) -> Flat {
    let has_n = !mesh.normals().is_none();
    let has_t = !mesh.texture().is_none();
    let has_c = !mesh.colours().is_none();
    let mut flat = Flat {
        points: Vec::new(),
        normals: has_n.then(Vec::new),
        texture: has_t.then(Vec::new),
        colours: has_c.then(Vec::new),
        quads: Vec::new(),
        ngons: Vec::new(),
    };
    let (kinds, corner_kind) = kinds(mesh);
    let number = number_points(&mut flat, mesh, &kinds);
    flat_faces(&mut flat, mesh, |c| number[corner_kind[c]]);
    flat
}

/// One written point per point and set of values, and which one each corner uses.
fn kinds(mesh: &Mesh) -> (Vec<Written>, Vec<usize>) {
    let mut seen: HashMap<Key, usize> = HashMap::new();
    let mut kinds: Vec<Written> = Vec::new();
    let mut corner_kind = Vec::with_capacity(mesh.corners().len());
    for c in 0..mesh.corners().len() {
        let v = mesh.corners()[c];
        let n = mesh
            .normals()
            .at(c, v as usize)
            .copied()
            .unwrap_or_default();
        let t = mesh
            .texture()
            .at(c, v as usize)
            .copied()
            .unwrap_or_default();
        let col = mesh
            .colours()
            .at(c, v as usize)
            .copied()
            .unwrap_or_default();
        let key = (
            v,
            n.map(f64::to_bits),
            t.map(f64::to_bits),
            col.map(f32::to_bits),
        );
        let next = kinds.len();
        let kind = *seen.entry(key).or_insert_with(|| {
            kinds.push((v, next, n, t, col));
            next
        });
        corner_kind.push(kind);
    }
    unused_points(mesh, &mut kinds);
    (kinds, corner_kind)
}

/// Points no face uses, kept too, so a mesh comes back as it went.
fn unused_points(mesh: &Mesh, kinds: &mut Vec<Written>) {
    let mut used = vec![false; mesh.vertex_count()];
    for &v in mesh.corners() {
        used[v as usize] = true;
    }
    for v in (0..mesh.vertex_count()).filter(|&v| !used[v]) {
        let v32 = u32::try_from(v).unwrap_or(u32::MAX);
        let n = mesh
            .normals()
            .at(usize::MAX, v)
            .copied()
            .unwrap_or_default();
        let t = mesh
            .texture()
            .at(usize::MAX, v)
            .copied()
            .unwrap_or_default();
        let col = mesh
            .colours()
            .at(usize::MAX, v)
            .copied()
            .unwrap_or_default();
        let next = kinds.len();
        kinds.push((v32, next, n, t, col));
    }
}

/// The written points in the mesh's own point order, put in `flat`; returns each kind's number.
fn number_points(flat: &mut Flat, mesh: &Mesh, kinds: &[Written]) -> Vec<u32> {
    let mut order: Vec<usize> = (0..kinds.len()).collect();
    order.sort_by_key(|&k| (kinds[k].0, kinds[k].1));
    let mut number = vec![0u32; kinds.len()];
    for (i, &k) in order.iter().enumerate() {
        number[k] = u32::try_from(i).unwrap_or(u32::MAX);
        let (v, _, n, t, col) = kinds[k];
        flat.points.push(mesh.vertices()[v as usize]);
        if let Some(all) = &mut flat.normals {
            all.push(n);
        }
        if let Some(all) = &mut flat.texture {
            all.push(t);
        }
        if let Some(all) = &mut flat.colours {
            all.push(col);
        }
    }
    number
}

/// The faces as four point numbers each, `at` giving a corner's; faces of more than four corners
/// as their triangles and an n-gon.
fn flat_faces(flat: &mut Flat, mesh: &Mesh, at: impl Fn(usize) -> u32) {
    let triangles = mesh.triangles();
    let mut first_triangle = vec![usize::MAX; mesh.face_count()];
    for (t, &f) in triangles.faces.iter().enumerate().rev() {
        first_triangle[f as usize] = t;
    }
    for (f, &first) in first_triangle.iter().enumerate() {
        let range = mesh.face_range(f);
        match range.len() {
            3 => {
                let [a, b, c] = [range.start, range.start + 1, range.start + 2].map(&at);
                flat.quads.push([a, b, c, c]);
            }
            4 => flat
                .quads
                .push(std::array::from_fn(|k| at(range.start + k))),
            _ => {
                let outline: Vec<u32> = range.clone().map(&at).collect();
                let mut faces = Vec::new();
                let mut t = first;
                while t < triangles.faces.len() && triangles.faces[t] as usize == f {
                    let [a, b, c] = triangles.corners[t].map(|k| at(k as usize));
                    faces.push(u32::try_from(flat.quads.len()).unwrap_or(u32::MAX));
                    flat.quads.push([a, b, c, c]);
                    t += 1;
                }
                flat.ngons.push((outline, faces));
            }
        }
    }
}

/// The lowest and highest of some numbers in each of `N` places, or the empty box openNURBS uses.
fn bounds<const N: usize>(values: &[[f64; N]]) -> ([f64; N], [f64; N]) {
    values.iter().fold(
        ([f64::INFINITY; N], [f64::NEG_INFINITY; N]),
        |(lo, hi), v| {
            (
                std::array::from_fn(|i| lo[i].min(v[i])),
                std::array::from_fn(|i| hi[i].max(v[i])),
            )
        },
    )
}

/// A number as single precision, rounded down (`ON_FloatFloor`) or up (`ON_FloatCeil`), so a box
/// of floats still holds every point.
#[allow(clippy::cast_possible_truncation)]
fn float_toward(x: f64, up: bool) -> f32 {
    let f = x as f32;
    let past = if up {
        f64::from(f) < x
    } else {
        f64::from(f) > x
    };
    if !past {
        return f;
    }
    let bits = f.to_bits();
    let step_up = (f > 0.0) == up || f == 0.0;
    f32::from_bits(match (f == 0.0, step_up) {
        (true, true) => 1,
        (true, false) => 0x8000_0001,
        (false, true) => bits + 1,
        (false, false) => bits - 1,
    })
}

/// One `ON_Mesh`, its class data.
pub(crate) fn write_mesh(w: &mut Writer, mesh: &Mesh) {
    let flat = flatten(mesh);
    let (lo, hi) = write_header(w, &flat, mesh.is_closed());
    write_faces(w, &flat);
    write_arrays(w, &flat);
    write_later_versions(w, &flat, (lo, hi));
}

/// The counts, domains, boxes and settings before the faces; returns the points' box.
fn write_header(w: &mut Writer, flat: &Flat, closed: bool) -> ([f64; 3], [f64; 3]) {
    let vcount = flat.points.len();
    w.chunk_version(3, 8);
    w.i32(i32::try_from(vcount).unwrap_or(i32::MAX));
    w.i32(i32::try_from(flat.quads.len()).unwrap_or(i32::MAX));
    // Packed texture domain, surface domain (unit intervals) and surface scale (none).
    w.f64s(&[0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0]);
    let (lo, hi) = bounds(&flat.points);
    if vcount > 0 {
        for c in lo {
            w.f32(float_toward(c, false));
        }
        for c in hi {
            w.f32(float_toward(c, true));
        }
    } else {
        for c in [1.0, 1.0, 1.0, -1.0, -1.0, -1.0] {
            w.f32(c);
        }
    }
    let float_box = |b: Option<([f64; 3], [f64; 3])>| -> [f32; 6] {
        #[allow(clippy::cast_possible_truncation)]
        b.map_or([0.0; 6], |(l, h)| {
            [l[0], l[1], l[2], h[0], h[1], h[2]].map(|c| c as f32)
        })
    };
    for c in float_box(
        flat.normals
            .as_deref()
            .filter(|n| !n.is_empty())
            .map(bounds),
    ) {
        w.f32(c);
    }
    let tbox = flat
        .texture
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(bounds);
    #[allow(clippy::cast_possible_truncation)]
    let tbox = tbox.map_or([0.0; 4], |(l, h)| {
        [l[0], l[1], h[0], h[1]].map(|c| c as f32)
    });
    for c in tbox {
        w.f32(c);
    }
    w.i32(i32::from(closed));
    w.u8(0); // no meshing parameters
    for _ in 0..4 {
        w.u8(0); // no curvature statistics
    }
    (lo, hi)
}

/// The faces, as four point numbers each.
fn write_faces(w: &mut Writer, flat: &Flat) {
    let vcount = flat.points.len();
    // Point numbers in the smallest size that holds them.
    let size = if vcount < 256 {
        1
    } else if vcount < 65536 {
        2
    } else {
        4
    };
    w.i32(size);
    for quad in &flat.quads {
        for &v in quad {
            #[allow(clippy::cast_possible_truncation)]
            match size {
                1 => w.u8(v as u8),
                2 => w.u16(v as u16),
                _ => w.u32(v),
            }
        }
    }
}

/// The points, normals, texture coordinates, curvatures and colours, as compressed arrays.
fn write_arrays(w: &mut Writer, flat: &Flat) {
    let float_bytes = |values: &mut dyn Iterator<Item = f64>| -> Vec<u8> {
        #[allow(clippy::cast_possible_truncation)]
        values.flat_map(|c| (c as f32).to_le_bytes()).collect()
    };
    buffer(w, &float_bytes(&mut flat.points.iter().flatten().copied()));
    buffer(
        w,
        &flat
            .normals
            .as_ref()
            .map_or_else(Vec::new, |n| float_bytes(&mut n.iter().flatten().copied())),
    );
    buffer(
        w,
        &flat
            .texture
            .as_ref()
            .map_or_else(Vec::new, |t| float_bytes(&mut t.iter().flatten().copied())),
    );
    buffer(w, &[]); // no curvatures
    buffer(
        w,
        &flat.colours.as_ref().map_or_else(Vec::new, |c| {
            c.iter()
                .flat_map(|c| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let b = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
                    [b[0], b[1], b[2], 0]
                })
                .collect()
        }),
    );
}

/// What later versions add: the texture settings, the n-gons, and the points at double precision
/// with their box.
fn write_later_versions(w: &mut Writer, flat: &Flat, (lo, hi): ([f64; 3], [f64; 3])) {
    let vcount = flat.points.len();
    w.i32(0); // texture not packed and turned
    w.uuid(Uuid([0; 16])); // no texture mapping
    if vcount > 0 {
        buffer(w, &[]); // no surface parameters
    }
    // The texture mapping tag, version 1.1: none, with no change of place.
    w.begin_anonymous(1);
    w.uuid(Uuid([0; 16]));
    w.u32(0);
    for i in 0..4 {
        for j in 0..4 {
            w.f64(if i == j { 1.0 } else { 0.0 });
        }
    }
    w.u32(0);
    w.end();
    // Manifold, oriented and solid: not known, so Rhino works them out.
    w.u8(0);
    w.u8(0);
    w.u8(0);
    w.bool(!flat.ngons.is_empty());
    if !flat.ngons.is_empty() {
        w.begin_anonymous(0);
        w.u32(u32::try_from(flat.ngons.len()).unwrap_or(u32::MAX));
        for (outline, faces) in &flat.ngons {
            w.u32(u32::try_from(outline.len()).unwrap_or(u32::MAX));
            w.u32(u32::try_from(faces.len()).unwrap_or(u32::MAX));
            for &v in outline {
                w.u32(v);
            }
            for &f in faces {
                w.u32(f);
            }
        }
        w.end();
    }
    w.bool(true);
    w.begin_anonymous(0);
    w.u32(u32::try_from(vcount).unwrap_or(u32::MAX));
    if vcount > 0 {
        let bytes: Vec<u8> = flat
            .points
            .iter()
            .flatten()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        buffer(w, &bytes);
    }
    w.end();
    if vcount > 0 {
        w.f64s(&lo);
        w.f64s(&hi);
    } else {
        w.f64s(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    }
}

/// A buffer as openNURBS' `WriteCompressedBuffer` writes it (see `read::mesh::buffer`), deflated.
fn buffer(w: &mut Writer, data: &[u8]) {
    w.u32(u32::try_from(data.len()).unwrap_or(u32::MAX));
    if data.is_empty() {
        return;
    }
    w.u32(crc32fast::hash(data));
    w.u8(1);
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = encoder.write_all(data);
    let packed = encoder.finish().unwrap_or_default();
    w.begin(crate::archive::tcode::ANONYMOUS_CHUNK);
    w.out.extend_from_slice(&packed);
    w.end();
}
