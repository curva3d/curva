//! Reading Rhino's meshes (`ON_Mesh`, versions 3.0 to 3.8) into Curva's.
//!
//! The layout is openNURBS' `ON_Mesh::Read` (`opennurbs_mesh.cpp`): counts and boxes, the faces as
//! four point numbers each (a triangle repeats its third), then the points, normals, texture
//! coordinates, curvatures and colours as compressed arrays, then, in later versions, the texture
//! mapping tag, n-gons, and the points again at double precision. Rhino stores normals, texture
//! coordinates and colours once per point and keeps a hard edge as two points at one place; Curva
//! keeps those points as they are, which it reads as one edge (see `curva_mesh::Mesh::places`).
//! An n-gon, kept by Rhino as a group of triangles with its outline, comes in as one face.

use crate::archive::{Reader, Result, bad};
use curva_mesh::{Attribute, Mesh, Point};

/// One `ON_Mesh`, its class data.
///
/// # Errors
/// When the record is damaged or of a version Curva does not read (1.x, from Rhino 1 and 2).
pub(crate) fn mesh(r: &mut Reader) -> Result<Mesh> {
    let (minor, vcount, quads) = header_and_faces(r)?;
    let arrays = arrays(r, vcount)?;
    let (ngons, doubles) = later_versions(r, minor, vcount)?;
    // Version 3.8's double-precision box after this is worked out again too.
    let points = chosen_points(&arrays.points, doubles, vcount);
    let faces = faces(&quads, &ngons, vcount);
    let mesh = Mesh::new(points, &faces).map_err(|e| bad(e.to_string()))?;
    with_attributes(mesh, &arrays, vcount)
}

/// An n-gon: its outline and the triangles it is kept as.
type Ngon = (Vec<u32>, Vec<u32>);

/// The version's minor number, the point count and the faces as four point numbers each, after
/// the counts, boxes and settings that are worked out again.
fn header_and_faces(r: &mut Reader) -> Result<(u32, usize, Vec<[u32; 4]>)> {
    let (major, minor) = r.chunk_version()?;
    if major != 3 {
        return Err(bad(format!("a mesh of version {major}.{minor}")));
    }
    let vcount = usize::try_from(r.i32()?).map_err(|_| bad("a negative point count"))?;
    let fcount = usize::try_from(r.i32()?).map_err(|_| bad("a negative face count"))?;
    // Texture and surface domains (4 intervals), surface scale, and the point, normal and texture
    // boxes: all worked out again from the arrays.
    r.skip(4 * 16 + 2 * 8 + 6 * 4 + 6 * 4 + 4 * 4)?;
    let _closed = r.i32()?;
    // Meshing parameters, then four curvature statistics, each a flag and a chunk.
    for _ in 0..5 {
        if r.u8()? != 0 {
            r.chunk()?;
        }
    }
    let size = r.i32()?;
    let mut quads = Vec::with_capacity(fcount.min(r.left() / 4));
    for _ in 0..fcount {
        let mut vi = [0u32; 4];
        for v in &mut vi {
            *v = match size {
                1 => u32::from(r.u8()?),
                2 => u32::from(r.u16()?),
                4 => r.u32()?,
                _ => return Err(bad("a mesh face of unknown size")),
            };
        }
        quads.push(vi);
    }
    Ok((u32::from(minor), vcount, quads))
}

/// The per-point arrays, as stored.
struct Arrays {
    points: Vec<f32>,
    normals: Vec<f32>,
    texture: Vec<f32>,
    colours: Vec<u8>,
}

/// The points, normals, texture coordinates, curvatures and colours, as compressed arrays.
fn arrays(r: &mut Reader, vcount: usize) -> Result<Arrays> {
    let floats = |bytes: &[u8], each: usize| -> Result<Vec<f32>> {
        if !bytes.is_empty() && bytes.len() != vcount * each * 4 {
            return Err(bad("a mesh array of the wrong size"));
        }
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect())
    };
    let points_f = floats(&buffer(r)?, 3)?;
    let normals_f = floats(&buffer(r)?, 3)?;
    let texture_f = floats(&buffer(r)?, 2)?;
    let _curvatures = buffer(r)?;
    let colours_raw = buffer(r)?;
    if !colours_raw.is_empty() && colours_raw.len() != vcount * 4 {
        return Err(bad("a mesh colour array of the wrong size"));
    }
    if points_f.len() != vcount * 3 {
        return Err(bad("a mesh without its points"));
    }
    Ok(Arrays {
        points: points_f,
        normals: normals_f,
        texture: texture_f,
        colours: colours_raw,
    })
}

/// What later versions add: the n-gons and the points at double precision.
fn later_versions(r: &mut Reader, minor: u32, vcount: usize) -> Result<(Vec<Ngon>, Vec<Point>)> {
    let mut ngons: Vec<Ngon> = Vec::new();
    let mut doubles: Vec<Point> = Vec::new();
    if minor >= 2 {
        let _packed_rotate = r.i32()?;
    }
    if minor >= 3 {
        let _mapping = r.uuid()?;
        if vcount > 0 {
            let _surface_parameters = buffer(r)?;
        }
    }
    if minor >= 4 {
        r.chunk()?; // the texture mapping tag
    }
    if minor >= 5 {
        r.skip(3)?; // manifold, oriented and solid flags, all worked out again
    }
    if minor >= 6 && r.bool()? {
        ngons = read_ngons(r)?;
    }
    if minor >= 7 && r.bool()? {
        doubles = read_doubles(r, vcount)?;
    }
    Ok((ngons, doubles))
}

/// The n-gons, each its outline and its triangles.
fn read_ngons(r: &mut Reader) -> Result<Vec<Ngon>> {
    let mut ngons = Vec::new();
    {
        let (_, mut body) = r.anonymous()?;
        let count = body.u32()?;
        for _ in 0..count {
            let vn = body.u32()? as usize;
            if vn == 0 {
                continue;
            }
            let fn_ = body.u32()? as usize;
            if vn.max(fn_) > body.left() / 4 {
                return Err(bad("an n-gon longer than its record"));
            }
            let outline = (0..vn).map(|_| body.u32()).collect::<Result<Vec<_>>>()?;
            let faces = (0..fn_).map(|_| body.u32()).collect::<Result<Vec<_>>>()?;
            ngons.push((outline, faces));
        }
    }
    Ok(ngons)
}

/// The points at double precision, when there is one for each point.
fn read_doubles(r: &mut Reader, vcount: usize) -> Result<Vec<Point>> {
    let mut doubles = Vec::new();
    {
        let (_, mut body) = r.anonymous()?;
        let count = body.u32()? as usize;
        if count > 0 {
            let bytes = buffer(&mut body)?;
            if count == vcount && bytes.len() == count * 24 {
                doubles = bytes
                    .as_chunks::<24>()
                    .0
                    .iter()
                    .map(|b| {
                        std::array::from_fn(|i| {
                            let mut d = [0u8; 8];
                            d.copy_from_slice(&b[i * 8..i * 8 + 8]);
                            f64::from_le_bytes(d)
                        })
                    })
                    .collect();
            }
        }
    }
    Ok(doubles)
}

/// The double-precision points where Rhino kept them and they agree with the single-precision
/// ones (as openNURBS checks); otherwise the single-precision points as the decimals they show.
fn chosen_points(points_f: &[f32], doubles: Vec<Point>, vcount: usize) -> Vec<Point> {
    let singles: Vec<Point> = points_f
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| p.map(widen))
        .collect();
    let agree = doubles.len() == vcount
        && doubles
            .iter()
            .zip(points_f.as_chunks::<3>().0.iter())
            .all(|(d, f)| {
                let m = f.iter().fold(0.0_f64, |m, c| m.max(f64::from(c.abs())));
                #[allow(clippy::cast_possible_truncation)]
                let off = (0..3).fold(0.0_f64, |o, i| o.max(f64::from((d[i] as f32 - f[i]).abs())));
                off <= m * 1e-6
            });
    if agree { doubles } else { singles }
}

/// Faces, with each n-gon's triangles replaced by its outline at its first triangle.
fn faces(quads: &[[u32; 4]], ngons: &[Ngon], vcount: usize) -> Vec<Vec<u32>> {
    let mut in_ngon: Vec<Option<usize>> = vec![None; quads.len()];
    for (n, (outline, faces)) in ngons.iter().enumerate() {
        if outline.len() >= 3 && faces.iter().all(|&f| (f as usize) < quads.len()) {
            for &f in faces {
                in_ngon[f as usize] = Some(n);
            }
        }
    }
    let mut placed = vec![false; ngons.len()];
    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(quads.len());
    for (f, vi) in quads.iter().enumerate() {
        let face: Vec<u32> = match in_ngon[f] {
            Some(n) if placed[n] => continue,
            Some(n) => {
                placed[n] = true;
                ngons[n].0.clone()
            }
            None if vi[2] == vi[3] => vi[..3].to_vec(),
            None => vi.to_vec(),
        };
        // Faces Rhino keeps but Curva cannot (a corner twice, a point missing) are left out.
        if face.len() >= 3
            && face.iter().all(|&v| (v as usize) < vcount)
            && (1..face.len()).all(|i| !face[..i].contains(&face[i]))
        {
            faces.push(face);
        }
    }
    faces
}

/// The mesh with the normals, texture coordinates and colours Rhino kept for each point.
fn with_attributes(mut mesh: Mesh, arrays: &Arrays, vcount: usize) -> Result<Mesh> {
    if arrays.normals.len() == vcount * 3 && vcount > 0 {
        let normals = arrays
            .normals
            .as_chunks::<3>()
            .0
            .iter()
            .map(|n| n.map(widen))
            .collect();
        mesh = mesh
            .with_normals(Attribute::PerVertex(normals))
            .map_err(|e| bad(e.to_string()))?;
    }
    if arrays.texture.len() == vcount * 2 && vcount > 0 {
        let texture = arrays
            .texture
            .as_chunks::<2>()
            .0
            .iter()
            .map(|t| t.map(widen))
            .collect();
        mesh = mesh
            .with_texture(Attribute::PerVertex(texture))
            .map_err(|e| bad(e.to_string()))?;
    }
    if arrays.colours.len() == vcount * 4 && vcount > 0 {
        // `ON_Color` is red, green, blue and transparency, a byte each.
        let colours = arrays
            .colours
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| [c[0], c[1], c[2]].map(|b| f32::from(b) / 255.0))
            .collect();
        mesh = mesh
            .with_colours(Attribute::PerVertex(colours))
            .map_err(|e| bad(e.to_string()))?;
    }
    Ok(mesh)
}

/// A single-precision number as the decimal it was written from, as Curva's STL reader reads
/// them, so a mesh drawn in round numbers keeps them.
fn widen(x: f32) -> f64 {
    x.to_string().parse().unwrap_or(f64::from(x))
}

/// A buffer as openNURBS' `WriteCompressedBuffer` writes it: its size in four bytes (`WriteSize`,
/// four bytes in every archive version), then, unless empty, its CRC, how it is stored, and the
/// bytes, stored or deflated in an anonymous chunk.
fn buffer(r: &mut Reader) -> Result<Vec<u8>> {
    let size = r.u32()? as usize;
    if size == 0 {
        return Ok(Vec::new());
    }
    let _crc = r.u32()?;
    match r.u8()? {
        0 => Ok(r.bytes(size)?.to_vec()),
        1 => {
            let mut chunk = r.chunk()?;
            let packed = chunk.body.bytes(chunk.body.left())?;
            let mut out = Vec::with_capacity(size.min(packed.len() * 16));
            use std::io::Read;
            flate2::read::ZlibDecoder::new(packed)
                .take(size as u64)
                .read_to_end(&mut out)
                .map_err(|e| bad(format!("a damaged mesh array: {e}")))?;
            if out.len() != size {
                return Err(bad("a mesh array of the wrong size"));
            }
            Ok(out)
        }
        _ => Err(bad("a mesh array stored in an unknown way")),
    }
}
