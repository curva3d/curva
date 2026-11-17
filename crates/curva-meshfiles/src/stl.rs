//! STL, the stereolithography file: loose triangles with no shared points, as text or binary
//! (3D Systems, "StereoLithography Interface Specification", 1988; binary layout as every
//! slicer and Rhino read it: an 80-byte header, a 32-bit triangle count, then 50 bytes a
//! triangle).
//!
//! Reading joins points at exactly the same place into one vertex, so a closed part comes in
//! closed. Writing splits faces into triangles. Binary STL keeps only single-precision numbers
//! (about seven digits): a part far from the origin loses detail, which is the format's limit.

use crate::{Contents, Named, ReadError};
use curva_mesh::{Mesh, MeshError, Point};
use std::collections::HashMap;
use std::fmt::Write as _;

/// The meshes in an STL file, text or binary, told apart by the file itself.
///
/// A binary file is recognised by its size matching its triangle count, so binary files whose
/// header starts with "solid" (as some CAD programs write) still read as binary.
///
/// # Errors
/// When the file is neither, is cut short, or has a number that is not one.
pub fn read(data: &[u8]) -> Result<Contents, ReadError> {
    if is_binary(data) {
        read_binary(data)
    } else if data.trim_ascii_start().starts_with(b"solid") {
        read_text(data)
    } else if data.len() >= 84 {
        Err(ReadError::new(
            "binary STL whose length does not match its triangle count",
        ))
    } else {
        Err(ReadError::new("not an STL file"))
    }
}

fn is_binary(data: &[u8]) -> bool {
    let Some(count) = data.get(80..84) else {
        return false;
    };
    let count = u32::from_le_bytes([count[0], count[1], count[2], count[3]]);
    usize::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(50))
        .and_then(|n| n.checked_add(84))
        == Some(data.len())
}

fn read_binary(data: &[u8]) -> Result<Contents, ReadError> {
    let mut welder = Welder::default();
    for triangle in data[84..].as_chunks::<50>().0 {
        // Skip the stored normal (12 bytes): it is worked out again from the corners.
        let corner = |k: usize| -> Point {
            std::array::from_fn(|i| {
                let at = 12 + 12 * k + 4 * i;
                let bytes = [
                    triangle[at],
                    triangle[at + 1],
                    triangle[at + 2],
                    triangle[at + 3],
                ];
                widen(f32::from_le_bytes(bytes))
            })
        };
        welder.face(&[corner(0), corner(1), corner(2)]);
    }
    let mut contents = Contents::default();
    welder.finish(String::new(), &mut contents)?;
    Ok(contents)
}

fn read_text(data: &[u8]) -> Result<Contents, ReadError> {
    let text = String::from_utf8_lossy(data);
    let mut contents = Contents::default();
    let mut welder = Welder::default();
    let mut name = String::new();
    let mut ring: Vec<Point> = Vec::new();
    for (k, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        match keyword.to_ascii_lowercase().as_str() {
            "solid" => {
                if !welder.is_empty() {
                    welder.finish(std::mem::take(&mut name), &mut contents)?;
                    welder = Welder::default();
                }
                name = words.collect::<Vec<_>>().join(" ");
            }
            "vertex" => {
                let mut p = [0.0; 3];
                for c in &mut p {
                    *c = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| ReadError::at(k + 1, "a vertex without three numbers"))?;
                }
                ring.push(p);
            }
            // Text STL allows loops of more than three corners; they come in as one face.
            "endloop" => welder.face(&std::mem::take(&mut ring)),
            "endsolid" => {
                welder.finish(std::mem::take(&mut name), &mut contents)?;
                welder = Welder::default();
            }
            _ => {}
        }
    }
    if !welder.is_empty() {
        welder.finish(name, &mut contents)?;
    }
    Ok(contents)
}

/// A single-precision number as the decimal it was written from: `0.1` stays `0.1` rather than
/// becoming `0.100000001490116`, so a part drawn in round millimetres keeps round numbers.
pub(crate) fn widen(x: f32) -> f64 {
    x.to_string().parse().unwrap_or(f64::from(x))
}

/// Joins corners at exactly the same place into shared vertices.
#[derive(Default)]
struct Welder {
    vertices: Vec<Point>,
    at: HashMap<[u64; 3], u32>,
    faces: Vec<Vec<u32>>,
    degenerate: usize,
}

impl Welder {
    fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.degenerate == 0
    }

    fn face(&mut self, ring: &[Point]) {
        let face: Vec<u32> = ring.iter().map(|&p| self.vertex(p)).collect();
        // A triangle with two corners in one place has no area: slicers drop them, and so does
        // Curva, counting them for the command bar.
        if face.len() < 3 || (1..face.len()).any(|i| face[..i].contains(&face[i])) {
            self.degenerate += 1;
        } else {
            self.faces.push(face);
        }
    }

    fn vertex(&mut self, p: Point) -> u32 {
        // Adding zero makes -0 and +0 the same place.
        let key = p.map(|c| (c + 0.0).to_bits());
        let next = u32::try_from(self.vertices.len()).unwrap_or(u32::MAX);
        *self.at.entry(key).or_insert_with(|| {
            self.vertices.push(p);
            next
        })
    }

    fn finish(self, name: String, contents: &mut Contents) -> Result<(), ReadError> {
        contents.skip("triangles with no area", self.degenerate);
        if self.faces.is_empty() {
            return Ok(());
        }
        let mesh = Mesh::new(self.vertices, &self.faces).map_err(|e| match e {
            MeshError::NotFinite { .. } => ReadError::new("a corner that is not a number"),
            other => ReadError::new(other.to_string()),
        })?;
        contents.meshes.push(Named { name, mesh });
        Ok(())
    }
}

/// The meshes as one binary STL file: all their faces as triangles, each with its normal.
#[must_use]
pub fn write_binary(meshes: &[(&str, &Mesh)]) -> Vec<u8> {
    let mut header = [b' '; 80];
    let label = b"Curva binary STL";
    header[..label.len()].copy_from_slice(label);
    let mut out = header.to_vec();
    let mut count: u32 = 0;
    out.extend_from_slice(&count.to_le_bytes());
    for (_, mesh) in meshes {
        for [a, b, c] in triangles(mesh) {
            for value in normal(a, b, c).into_iter().chain(a).chain(b).chain(c) {
                #[allow(clippy::cast_possible_truncation)] // The format holds single precision.
                out.extend_from_slice(&(value as f32).to_le_bytes());
            }
            out.extend_from_slice(&0u16.to_le_bytes());
            count = count.saturating_add(1);
        }
    }
    out[80..84].copy_from_slice(&count.to_le_bytes());
    out
}

/// The meshes as one text STL file, each a `solid` of its own name. Numbers are written in full,
/// so nothing is lost; readers that keep single precision round them as they read.
#[must_use]
pub fn write_text(meshes: &[(&str, &Mesh)]) -> String {
    let mut out = String::new();
    for (name, mesh) in meshes {
        let name = one_line(name);
        let _ = writeln!(out, "solid {name}");
        for [a, b, c] in triangles(mesh) {
            let [nx, ny, nz] = normal(a, b, c);
            let _ = writeln!(out, "  facet normal {nx} {ny} {nz}\n    outer loop");
            for [x, y, z] in [a, b, c] {
                let _ = writeln!(out, "      vertex {x} {y} {z}");
            }
            let _ = writeln!(out, "    endloop\n  endfacet");
        }
        let _ = writeln!(out, "endsolid {name}");
    }
    out
}

fn triangles(mesh: &Mesh) -> impl Iterator<Item = [Point; 3]> + '_ {
    mesh.triangles()
        .triangles
        .into_iter()
        .map(|t| t.map(|v| mesh.vertices()[v as usize]))
}

fn one_line(name: &str) -> String {
    name.replace(['\r', '\n'], " ")
}

/// The unit normal of a triangle, or zero for one with no area.
fn normal(a: Point, b: Point, c: Point) -> Point {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 0.0 {
        n.map(|c| c / length)
    } else {
        [0.0; 3]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A unit cube of six quads, counter-clockwise from outside.
    pub(crate) fn cube() -> Mesh {
        let v = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let faces = [
            [0u32, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ];
        Mesh::new(v, &faces).unwrap()
    }

    fn only(contents: &Contents) -> &Mesh {
        assert_eq!(contents.meshes.len(), 1);
        &contents.meshes[0].mesh
    }

    #[test]
    fn a_cube_comes_back_closed_from_binary_with_shared_corners() {
        let data = write_binary(&[("cube", &cube())]);
        assert_eq!(data.len(), 84 + 12 * 50);
        let contents = read(&data).unwrap();
        let mesh = only(&contents);
        assert_eq!((mesh.vertex_count(), mesh.face_count()), (8, 12));
        assert!(mesh.is_closed());
        assert!((mesh.volume().unwrap() - 1.0).abs() < 1e-12);
        assert!(contents.skipped.is_empty());
    }

    #[test]
    fn a_cube_comes_back_closed_from_text_with_its_name() {
        let text = write_text(&[("my part", &cube())]);
        assert!(text.starts_with("solid my part\n"));
        let contents = read(text.as_bytes()).unwrap();
        assert_eq!(contents.meshes[0].name, "my part");
        let mesh = only(&contents);
        assert_eq!((mesh.vertex_count(), mesh.face_count()), (8, 12));
        assert!((mesh.volume().unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn round_millimetres_stay_round_through_single_precision() {
        let mesh = cube().translated([0.1, 25.4, -3.3]);
        let back = read(&write_binary(&[("", &mesh)])).unwrap();
        assert_eq!(
            only(&back).bounds(),
            Some(([0.1, 25.4, -3.3], [1.1, 26.4, -2.3]))
        );
    }

    #[test]
    fn a_binary_file_whose_header_says_solid_still_reads_as_binary() {
        let mut data = write_binary(&[("", &cube())]);
        data[..12].copy_from_slice(b"solid part 1");
        assert_eq!(only(&read(&data).unwrap()).face_count(), 12);
    }

    #[test]
    fn several_solids_in_one_text_file_are_several_meshes() {
        let one = cube();
        let two = cube().translated([5.0, 0.0, 0.0]);
        let text = write_text(&[("a", &one), ("b", &two)]);
        let contents = read(text.as_bytes()).unwrap();
        let names: Vec<&str> = contents.meshes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(contents.meshes[1].mesh.bounds().unwrap().0, [5.0, 0.0, 0.0]);
    }

    #[test]
    fn triangles_with_no_area_are_left_out_and_counted() {
        let text = "solid t
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 0 0 0
vertex 1 0 0
endloop
endfacet
endsolid t
";
        let contents = read(text.as_bytes()).unwrap();
        assert_eq!(only(&contents).face_count(), 1);
        assert_eq!(contents.skipped, [("triangles with no area".to_owned(), 1)]);
    }

    #[test]
    fn bad_files_say_what_is_wrong() {
        let mut short = write_binary(&[("", &cube())]);
        short.truncate(short.len() - 10);
        assert!(read(&short).unwrap_err().problem.contains("length"));
        assert_eq!(read(b"hello").unwrap_err().problem, "not an STL file");
        let error = read(b"solid\nfacet\nouter loop\nvertex 1 two 3\n").unwrap_err();
        assert_eq!(error.line, Some(4));
    }
}
