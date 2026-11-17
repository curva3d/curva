//! PLY, the Stanford polygon format (Turk, "The PLY Polygon File Format", 1994), text and binary.
//!
//! A PLY file holds one mesh: a header naming its elements and their properties, then the data.
//! Curva reads points with their normals, colours and texture coordinates, and faces of any number
//! of corners; other elements (Rhino writes a `material`) are left out and counted. It writes points
//! at double precision so nothing is rounded, with normals, colours and texture coordinates when
//! the mesh has them. PLY stores these once per point, so where a mesh keeps them per corner (a hard
//! edge, a texture seam) the point is written once for each different value, as Rhino does.

use crate::{Contents, Named, ReadError};
use curva_mesh::{Attribute, Mesh, Point};
use std::collections::HashMap;
use std::fmt::Write as _;

/// How the data after the header is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Text,
    Little,
    Big,
}

/// A number type a property can have, with the size of its binary form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Kind {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

/// A property of an element: one number, or a list of numbers led by its length.
#[derive(Clone, Debug)]
struct Property {
    name: String,
    kind: Kind,
    /// The type of a list's length, for list properties.
    count: Option<Kind>,
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: usize,
    properties: Vec<Property>,
}

/// Reads a PLY file, text or binary.
///
/// # Errors
/// When it is not PLY, its header is broken, or its data runs short or does not parse.
pub fn read(data: &[u8]) -> Result<Contents, ReadError> {
    let (format, elements, body) = header(data)?;
    let mut values = Values::new(format, body);
    let mut points: Vec<Point> = Vec::new();
    let mut normals: Vec<Point> = Vec::new();
    let mut colours: Vec<[f32; 3]> = Vec::new();
    let mut texture: Vec<[f64; 2]> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let mut contents = Contents::default();
    let mut bad_faces = 0;
    for element in &elements {
        match element.name.as_str() {
            "vertex" => {
                let find = |names: &[&str]| {
                    names
                        .iter()
                        .find_map(|n| element.properties.iter().position(|p| p.name == *n))
                };
                let xyz = [find(&["x"]), find(&["y"]), find(&["z"])];
                let [Some(x), Some(y), Some(z)] = xyz else {
                    return Err(ReadError::new("the points have no x, y and z"));
                };
                let normal = [find(&["nx"]), find(&["ny"]), find(&["nz"])];
                let colour = [
                    find(&["red", "r", "diffuse_red"]),
                    find(&["green", "g", "diffuse_green"]),
                    find(&["blue", "b", "diffuse_blue"]),
                ];
                let uv = [
                    find(&["s", "u", "texture_u", "texture_s"]),
                    find(&["t", "v", "texture_v", "texture_t"]),
                ];
                for _ in 0..element.count {
                    let row = values.row(element)?;
                    let number = |i: usize| row[i][0];
                    let point = [number(x), number(y), number(z)];
                    if point.iter().any(|c| !c.is_finite()) {
                        return Err(ReadError::new(format!(
                            "point {} is not a finite point",
                            points.len()
                        )));
                    }
                    points.push(point);
                    if let [Some(a), Some(b), Some(c)] = normal {
                        normals.push([number(a), number(b), number(c)]);
                    }
                    if let [Some(r), Some(g), Some(b)] = colour {
                        colours.push([r, g, b].map(|i| {
                            let p = &element.properties[i];
                            #[allow(clippy::cast_possible_truncation)]
                            let v = number(i) as f32;
                            if p.kind.is_float() { v } else { v / 255.0 }
                        }));
                    }
                    if let [Some(s), Some(t)] = uv {
                        texture.push([number(s), number(t)]);
                    }
                }
            }
            "face" => {
                let Some(list) = element
                    .properties
                    .iter()
                    .position(|p| p.name == "vertex_indices" || p.name == "vertex_index")
                else {
                    return Err(ReadError::new("the faces have no list of points"));
                };
                for _ in 0..element.count {
                    let row = values.row(element)?;
                    let face: Option<Vec<u32>> = row[list]
                        .iter()
                        .map(|&v| {
                            (v >= 0.0 && v.fract() == 0.0 && v <= f64::from(u32::MAX)).then(|| {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let i = v as u32;
                                i
                            })
                        })
                        .collect();
                    match face {
                        Some(face) if usable(&face, points.len()) => faces.push(face),
                        _ => bad_faces += 1,
                    }
                }
            }
            other => {
                for _ in 0..element.count {
                    values.row(element)?;
                }
                contents.skip(other, element.count);
            }
        }
    }
    contents.skip("degenerate face", bad_faces);
    let mut mesh = Mesh::new(points, &faces).map_err(|e| ReadError::new(e.to_string()))?;
    let n = mesh.vertex_count();
    if normals.len() == n && n > 0 {
        mesh = mesh
            .with_normals(Attribute::PerVertex(normals))
            .map_err(|e| ReadError::new(e.to_string()))?;
    }
    if colours.len() == n && n > 0 {
        mesh = mesh
            .with_colours(Attribute::PerVertex(colours))
            .map_err(|e| ReadError::new(e.to_string()))?;
    }
    if texture.len() == n && n > 0 {
        mesh = mesh
            .with_texture(Attribute::PerVertex(texture))
            .map_err(|e| ReadError::new(e.to_string()))?;
    }
    contents.meshes.push(Named {
        name: String::new(),
        mesh,
    });
    Ok(contents)
}

/// Whether a face can be kept: three or more corners, all real points, none twice.
fn usable(face: &[u32], points: usize) -> bool {
    face.len() >= 3
        && face.iter().all(|&v| (v as usize) < points)
        && (1..face.len()).all(|i| !face[..i].contains(&face[i]))
}

/// The header: the data's format, the elements, and the bytes after `end_header`.
fn header(data: &[u8]) -> Result<(Format, Vec<Element>, &[u8]), ReadError> {
    let mut rest = data;
    let mut line_number = 0;
    let mut next_line = |rest: &mut &[u8]| -> Option<(usize, String)> {
        if rest.is_empty() {
            return None;
        }
        let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        let line = String::from_utf8_lossy(&rest[..end]).trim().to_owned();
        *rest = &rest[(end + 1).min(rest.len())..];
        line_number += 1;
        Some((line_number, line))
    };
    match next_line(&mut rest) {
        Some((_, magic)) if magic == "ply" => {}
        _ => return Err(ReadError::new("not a PLY file")),
    }
    let mut format = None;
    let mut elements: Vec<Element> = Vec::new();
    loop {
        let Some((at, line)) = next_line(&mut rest) else {
            return Err(ReadError::new("the header has no end_header"));
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["format", kind, _version] => {
                format = Some(match *kind {
                    "ascii" => Format::Text,
                    "binary_little_endian" => Format::Little,
                    "binary_big_endian" => Format::Big,
                    _ => return Err(ReadError::at(at, format!("unknown format {kind}"))),
                });
            }
            ["element", name, count] => elements.push(Element {
                name: (*name).to_owned(),
                count: count
                    .parse()
                    .map_err(|_| ReadError::at(at, "an element count is not a number"))?,
                properties: Vec::new(),
            }),
            ["property", "list", count, kind, name] => {
                let (Some(count), Some(kind)) = (Kind::parse(count), Kind::parse(kind)) else {
                    return Err(ReadError::at(at, "unknown number type"));
                };
                elements
                    .last_mut()
                    .ok_or_else(|| ReadError::at(at, "a property before any element"))?
                    .properties
                    .push(Property {
                        name: (*name).to_owned(),
                        kind,
                        count: Some(count),
                    });
            }
            ["property", kind, name] => {
                let kind =
                    Kind::parse(kind).ok_or_else(|| ReadError::at(at, "unknown number type"))?;
                elements
                    .last_mut()
                    .ok_or_else(|| ReadError::at(at, "a property before any element"))?
                    .properties
                    .push(Property {
                        name: (*name).to_owned(),
                        kind,
                        count: None,
                    });
            }
            ["end_header"] => break,
            [] | ["comment", ..] | ["obj_info", ..] => {}
            _ => return Err(ReadError::at(at, format!("cannot read \"{line}\""))),
        }
    }
    let format = format.ok_or_else(|| ReadError::new("the header gives no format"))?;
    Ok((format, elements, rest))
}

/// The numbers after the header, read one element row at a time.
struct Values<'a> {
    format: Format,
    data: &'a [u8],
    at: usize,
    words: Option<std::str::SplitAsciiWhitespace<'a>>,
}

impl<'a> Values<'a> {
    fn new(format: Format, data: &'a [u8]) -> Self {
        let words = (format == Format::Text)
            .then(|| {
                std::str::from_utf8(data)
                    .ok()
                    .map(str::split_ascii_whitespace)
            })
            .flatten();
        Self {
            format,
            data,
            at: 0,
            words,
        }
    }

    /// One row of an element: for each property its numbers (one, or a list's).
    fn row(&mut self, element: &Element) -> Result<Vec<Vec<f64>>, ReadError> {
        element
            .properties
            .iter()
            .map(|p| match p.count {
                None => Ok(vec![self.number(p.kind)?]),
                Some(count) => {
                    let n = self.number(count)?;
                    if !(0.0..=1e9).contains(&n) || n.fract() != 0.0 {
                        return Err(ReadError::new(format!(
                            "a {} list has a bad length",
                            element.name
                        )));
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let n = n as usize;
                    (0..n).map(|_| self.number(p.kind)).collect()
                }
            })
            .collect()
    }

    fn number(&mut self, kind: Kind) -> Result<f64, ReadError> {
        if self.format == Format::Text {
            let word = self
                .words
                .as_mut()
                .and_then(Iterator::next)
                .ok_or_else(|| ReadError::new("the data ends too soon"))?;
            let value: f64 = word
                .parse()
                .map_err(|_| ReadError::new(format!("\"{word}\" is not a number")))?;
            // Text floats are read as written; a float property's decimal is its value.
            return Ok(value);
        }
        let size = kind.size();
        let bytes = self
            .data
            .get(self.at..self.at + size)
            .ok_or_else(|| ReadError::new("the data ends too soon"))?;
        self.at += size;
        let mut b = [0u8; 8];
        b[..size].copy_from_slice(bytes);
        if self.format == Format::Big {
            b[..size].reverse();
        }
        Ok(match kind {
            Kind::I8 => f64::from(i8::from_le_bytes([b[0]])),
            Kind::U8 => f64::from(b[0]),
            Kind::I16 => f64::from(i16::from_le_bytes([b[0], b[1]])),
            Kind::U16 => f64::from(u16::from_le_bytes([b[0], b[1]])),
            Kind::I32 => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Kind::U32 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Kind::F32 => crate::stl::widen(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Kind::F64 => f64::from_le_bytes(b),
        })
    }
}

/// The meshes as one PLY mesh, points and faces in order, ready to write. PLY keeps data per
/// point, so a point whose corners disagree is written once per different value.
struct Flat {
    points: Vec<Point>,
    normals: Option<Vec<Point>>,
    colours: Option<Vec<[f32; 3]>>,
    texture: Option<Vec<[f64; 2]>>,
    faces: Vec<Vec<u32>>,
}

/// A point with its data, as bits, so equal values compare equal.
type Key = (u32, [u64; 3], [u32; 3], [u64; 2]);

/// A point to write: its vertex, when first met, and its normal, colour and texture coordinate.
type Written = (u32, usize, Point, [f32; 3], [f64; 2]);

fn flatten(meshes: &[(&str, &Mesh)]) -> Flat {
    let has = |f: &dyn Fn(&Mesh) -> bool| !meshes.is_empty() && meshes.iter().all(|(_, m)| f(m));
    let with_normals = has(&|m| !m.normals().is_none());
    let with_colours = has(&|m| !m.colours().is_none());
    let with_texture = has(&|m| !m.texture().is_none());
    let mut flat = Flat {
        points: Vec::new(),
        normals: with_normals.then(Vec::new),
        colours: with_colours.then(Vec::new),
        texture: with_texture.then(Vec::new),
        faces: Vec::new(),
    };
    for (_, mesh) in meshes {
        // Each point with the data at one of its corners: one written point per different pair,
        // in the mesh's own point order (a point's values in the order its corners come).
        let mut seen: HashMap<Key, usize> = HashMap::new();
        let mut kinds: Vec<Written> = Vec::new();
        let mut corner_kind = Vec::with_capacity(mesh.corners().len());
        {
            for c in 0..mesh.corners().len() {
                let v = mesh.corners()[c];
                let n = with_normals
                    .then(|| mesh.normals().at(c, v as usize).copied())
                    .flatten()
                    .unwrap_or_default();
                let colour = with_colours
                    .then(|| mesh.colours().at(c, v as usize).copied())
                    .flatten()
                    .unwrap_or_default();
                let uv = with_texture
                    .then(|| mesh.texture().at(c, v as usize).copied())
                    .flatten()
                    .unwrap_or_default();
                let key = (
                    v,
                    n.map(f64::to_bits),
                    colour.map(f32::to_bits),
                    uv.map(f64::to_bits),
                );
                let next = kinds.len();
                let kind = *seen.entry(key).or_insert_with(|| {
                    kinds.push((v, next, n, colour, uv));
                    next
                });
                corner_kind.push(kind);
            }
        }
        let mut order: Vec<usize> = (0..kinds.len()).collect();
        order.sort_by_key(|&k| (kinds[k].0, kinds[k].1));
        let start = flat.points.len();
        let mut number = vec![0u32; kinds.len()];
        for (i, &k) in order.iter().enumerate() {
            number[k] = u32::try_from(start + i).unwrap_or(u32::MAX);
            let (v, _, n, colour, uv) = kinds[k];
            flat.points.push(mesh.vertices()[v as usize]);
            if let Some(all) = &mut flat.normals {
                all.push(n);
            }
            if let Some(all) = &mut flat.colours {
                all.push(colour);
            }
            if let Some(all) = &mut flat.texture {
                all.push(uv);
            }
        }
        for f in 0..mesh.face_count() {
            flat.faces
                .push(mesh.face_range(f).map(|c| number[corner_kind[c]]).collect());
        }
    }
    flat
}

fn header_text(flat: &Flat, format: &str) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "ply\nformat {format} 1.0\ncomment Curva\nelement vertex {}\n\
         property double x\nproperty double y\nproperty double z\n",
        flat.points.len()
    );
    if flat.normals.is_some() {
        out.push_str("property float nx\nproperty float ny\nproperty float nz\n");
    }
    if flat.colours.is_some() {
        out.push_str("property uchar red\nproperty uchar green\nproperty uchar blue\n");
    }
    if flat.texture.is_some() {
        out.push_str("property double s\nproperty double t\n");
    }
    let _ = write!(
        out,
        "element face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        flat.faces.len()
    );
    out
}

/// A colour channel from 0 to 1 as a byte.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The meshes as one text PLY. PLY has no object names, so the names are not written; faces of
/// more than 255 corners are split, as the corner count is one byte.
#[must_use]
pub fn write_text(meshes: &[(&str, &Mesh)]) -> String {
    let flat = flatten(meshes);
    let flat = Flat {
        faces: short_faces(flat.faces),
        ..flat
    };
    let mut out = header_text(&flat, "ascii");
    for (i, p) in flat.points.iter().enumerate() {
        let _ = write!(out, "{} {} {}", p[0], p[1], p[2]);
        if let Some(n) = &flat.normals {
            #[allow(clippy::cast_possible_truncation)]
            let n = n[i].map(|c| c as f32);
            let _ = write!(out, " {} {} {}", n[0], n[1], n[2]);
        }
        if let Some(c) = &flat.colours {
            let c = c[i].map(byte);
            let _ = write!(out, " {} {} {}", c[0], c[1], c[2]);
        }
        if let Some(t) = &flat.texture {
            let _ = write!(out, " {} {}", t[i][0], t[i][1]);
        }
        out.push('\n');
    }
    for face in &flat.faces {
        let _ = write!(out, "{}", face.len());
        for v in face {
            let _ = write!(out, " {v}");
        }
        out.push('\n');
    }
    out
}

/// The meshes as one binary PLY, little-endian (see [`write_text`]).
#[must_use]
pub fn write_binary(meshes: &[(&str, &Mesh)]) -> Vec<u8> {
    let flat = flatten(meshes);
    let flat = Flat {
        faces: short_faces(flat.faces),
        ..flat
    };
    let mut out = header_text(&flat, "binary_little_endian").into_bytes();
    for (i, p) in flat.points.iter().enumerate() {
        for c in p {
            out.extend_from_slice(&c.to_le_bytes());
        }
        if let Some(n) = &flat.normals {
            for c in n[i] {
                #[allow(clippy::cast_possible_truncation)]
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        if let Some(c) = &flat.colours {
            out.extend(c[i].map(byte));
        }
        if let Some(t) = &flat.texture {
            for c in t[i] {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    for face in &flat.faces {
        out.push(u8::try_from(face.len()).unwrap_or(u8::MAX));
        for v in face {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Faces of more than 255 corners split into fans of at most 255, so each length fits a byte.
fn short_faces(faces: Vec<Vec<u32>>) -> Vec<Vec<u32>> {
    let mut out = Vec::with_capacity(faces.len());
    for face in faces {
        if face.len() <= 255 {
            out.push(face);
            continue;
        }
        let mut start = 1;
        while start + 1 < face.len() {
            let end = (start + 253).min(face.len() - 1);
            let mut part = vec![face[0]];
            part.extend_from_slice(&face[start..=end]);
            out.push(part);
            start = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stl::tests::cube;

    /// What Rhino 8 wrote for a small pyramid with normals and point colours (17 November 2026),
    /// its material values shortened to the same count.
    const FROM_RHINO: &str = "ply\r\nformat ascii 1.0\r\ncomment File exported by Rhinoceros Version 8\r\n\
        element vertex 5\r\nproperty float x\r\nproperty float y\r\nproperty float z\r\n\
        property float nx\r\nproperty float ny\r\nproperty float nz\r\n\
        property uchar red\r\nproperty uchar green\r\nproperty uchar blue\r\n\
        element face 5\r\nproperty list uchar uint vertex_indices\r\n\
        element material 1\r\nproperty uchar ambient_red\r\nproperty double shine\r\nend_header\r\n\
        0 0 0 -0.7046567 -0.7046567 -0.083173454 255 0 0\r\n\
        1 0 0 0.7046567 -0.7046567 -0.083173454 0 255 0\r\n\
        1 1 0 0.7046567 0.7046567 -0.083173454 0 0 255\r\n\
        0 1 0 -0.7046567 0.7046567 -0.083173454 255 255 0\r\n\
        0.5 0.5 1 0 0 1 10 20 30\r\n\
        4 0 3 2 1\r\n3 0 1 4\r\n3 1 2 4\r\n3 2 3 4\r\n3 3 0 4\r\n0 0\r\n";

    #[test]
    fn reads_rhinos_ply_with_normals_and_colours() {
        let contents = read(FROM_RHINO.as_bytes()).unwrap();
        assert_eq!(contents.skipped, vec![("material".to_owned(), 1)]);
        let mesh = &contents.meshes[0].mesh;
        assert_eq!((mesh.vertex_count(), mesh.face_count()), (5, 5));
        assert_eq!(mesh.face(0), &[0, 3, 2, 1]);
        assert_eq!(mesh.vertices()[4], [0.5, 0.5, 1.0]);
        let Attribute::PerVertex(n) = mesh.normals() else {
            panic!()
        };
        assert_eq!(n[0], [-0.7046567, -0.7046567, -0.083173454]);
        let Attribute::PerVertex(c) = mesh.colours() else {
            panic!()
        };
        assert_eq!(c[0], [1.0, 0.0, 0.0]);
        assert_eq!(c[4], [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0]);
        assert!(mesh.is_closed());
    }

    /// The cube with a normal, a colour and a texture coordinate at each point.
    fn dressed() -> Mesh {
        let normals: Vec<Point> = (0..8).map(|i| [0.6, 0.8 * f64::from(i % 2), 0.0]).collect();
        let colours: Vec<[f32; 3]> = (0..8).map(|i| [i as f32 / 255.0, 1.0, 0.0]).collect();
        let texture: Vec<[f64; 2]> = (0..8).map(|i| [0.1 * f64::from(i), 0.5]).collect();
        cube()
            .with_normals(Attribute::PerVertex(normals))
            .unwrap()
            .with_colours(Attribute::PerVertex(colours))
            .unwrap()
            .with_texture(Attribute::PerVertex(texture))
            .unwrap()
    }

    #[test]
    fn text_and_binary_both_come_back_exactly() {
        let mesh = dressed();
        let text = write_text(&[("cube", &mesh)]);
        assert!(text.starts_with("ply\nformat ascii 1.0\n"));
        let binary = write_binary(&[("cube", &mesh)]);
        for data in [text.into_bytes(), binary] {
            let back = read(&data).unwrap();
            assert_eq!(back.meshes[0].mesh, mesh);
            assert!(back.skipped.is_empty());
        }
    }

    #[test]
    fn points_far_from_the_origin_keep_every_digit() {
        let far = cube().translated([1.0e6 + 0.1, -2.0e7, 0.3]);
        let back = read(&write_binary(&[("", &far)])).unwrap();
        assert_eq!(back.meshes[0].mesh, far);
    }

    #[test]
    fn a_point_with_different_corners_is_written_once_for_each() {
        // Texture per corner, each face its own square: 24 corners, 24 different values.
        let texture: Vec<[f64; 2]> = (0..24)
            .map(|i| [f64::from(i % 4), f64::from(i / 4)])
            .collect();
        let mesh = cube().with_texture(Attribute::PerCorner(texture)).unwrap();
        let back = read(write_text(&[("", &mesh)]).as_bytes()).unwrap();
        let back = &back.meshes[0].mesh;
        assert_eq!((back.vertex_count(), back.face_count()), (24, 6));
        for c in 0..24 {
            let (v, w) = (mesh.corners()[c] as usize, back.corners()[c] as usize);
            assert_eq!(back.vertices()[w], mesh.vertices()[v]);
            assert_eq!(back.texture().at(c, w), mesh.texture().at(c, v));
        }
        assert!(
            (back.volume().unwrap() - 1.0).abs() < 1e-12,
            "joined by place"
        );
    }

    #[test]
    fn several_meshes_go_into_one() {
        let far = cube().translated([3.0, 0.0, 0.0]);
        let back = read(&write_binary(&[("a", &cube()), ("b", &far)])).unwrap();
        let mesh = &back.meshes[0].mesh;
        assert_eq!(
            (back.meshes.len(), mesh.vertex_count(), mesh.face_count()),
            (1, 16, 12)
        );
    }

    #[test]
    fn reads_big_endian_binary_and_other_number_types() {
        let mut data = b"ply\nformat binary_big_endian 1.0\nelement vertex 3\n\
            property short x\nproperty int y\nproperty double z\n\
            element face 1\nproperty list ushort int vertex_index\nend_header\n"
            .to_vec();
        for (x, y, z) in [(0i16, 0i32, 0.0f64), (2, 0, 0.0), (0, 3, 0.5)] {
            data.extend_from_slice(&x.to_be_bytes());
            data.extend_from_slice(&y.to_be_bytes());
            data.extend_from_slice(&z.to_be_bytes());
        }
        data.extend_from_slice(&3u16.to_be_bytes());
        for v in [0i32, 1, 2] {
            data.extend_from_slice(&v.to_be_bytes());
        }
        let mesh = &read(&data).unwrap().meshes[0].mesh;
        assert_eq!(
            mesh.vertices(),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.5]]
        );
        assert_eq!(mesh.face(0), &[0, 1, 2]);
    }

    #[test]
    fn faces_that_cannot_be_kept_are_counted() {
        let text = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\n\
            property float z\nelement face 3\nproperty list uchar int vertex_indices\nend_header\n\
            0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n2 0 1\n3 0 1 7\n";
        let contents = read(text.as_bytes()).unwrap();
        assert_eq!(contents.meshes[0].mesh.face_count(), 1);
        assert_eq!(contents.skipped, vec![("degenerate face".to_owned(), 2)]);
    }

    #[test]
    fn broken_files_say_why() {
        assert!(read(b"solid x\n").is_err());
        assert!(
            read(b"ply\nformat ascii 1.0\nelement vertex 1\n").is_err(),
            "no end_header"
        );
        let short = "ply\nformat ascii 1.0\nelement vertex 2\nproperty float x\nproperty float y\n\
            property float z\nend_header\n0 0 0\n1 0\n";
        assert_eq!(
            read(short.as_bytes()).unwrap_err().problem,
            "the data ends too soon"
        );
        let binary_short = write_binary(&[("", &cube())]);
        assert!(read(&binary_short[..binary_short.len() - 3]).is_err());
    }

    #[test]
    fn faces_of_more_than_255_corners_are_split_into_fans() {
        let n = 300u32;
        let points: Vec<Point> = (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / f64::from(n);
                [a.cos(), a.sin(), 0.0]
            })
            .collect();
        let ring: Vec<u32> = (0..n).collect();
        let disc = Mesh::new(points, &[ring]).unwrap();
        let back = read(&write_binary(&[("", &disc)])).unwrap();
        let back = &back.meshes[0].mesh;
        assert_eq!(back.face_count(), 2);
        assert!(back.faces().all(|f| f.len() <= 255));
        assert!((back.area() - disc.area()).abs() < 1e-12);
    }
}
