//! OBJ, the Wavefront object file: points, texture coordinates and normals in shared lists, and
//! faces of any number of corners that pick from them (Wavefront Technologies, "Advanced
//! Visualizer" manual, Appendix B1, "Object Files (.obj)"). Vertex colours follow the points as
//! three more numbers, as Rhino, Blender and MeshLab write them.
//!
//! Each `o` and `g` starts a new mesh, named by it, so a file of several parts comes in as
//! several objects. Materials, curves and surfaces are left out and counted.

use crate::{Contents, Named, ReadError};
use curva_mesh::{Attribute, Mesh, Point};
use std::collections::HashMap;
use std::fmt::Write as _;

/// One face corner: the point, and its texture coordinate and normal if it names them, all as
/// places in the file's shared lists.
type Corner = (usize, Option<usize>, Option<usize>);

/// The meshes in an OBJ file.
///
/// # Errors
/// When a number is not one, or a face names a point, texture coordinate or normal the file
/// does not have.
pub fn read(data: &[u8]) -> Result<Contents, ReadError> {
    let text = String::from_utf8_lossy(data);
    let mut file = File::default();
    let mut contents = Contents::default();
    let mut name = String::new();
    let mut faces: Vec<Vec<Corner>> = Vec::new();
    let mut joined = String::new();
    let mut start = 0;
    for (k, line) in text.lines().enumerate() {
        // A line ending in a backslash goes on to the next.
        if joined.is_empty() {
            start = k + 1;
        }
        if let Some(head) = line.strip_suffix('\\') {
            joined.push_str(head);
            joined.push(' ');
            continue;
        }
        joined.push_str(line);
        let line = std::mem::take(&mut joined);
        let line = line.split('#').next().unwrap_or_default();
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        let numbers = |words: std::str::SplitWhitespace<'_>| -> Result<Vec<f64>, ReadError> {
            words
                .map(|w| {
                    w.parse::<f64>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| ReadError::at(start, format!("\"{w}\" is not a number")))
                })
                .collect()
        };
        match keyword {
            "v" => {
                let n = numbers(words)?;
                if n.len() < 3 {
                    return Err(ReadError::at(start, "a point without three numbers"));
                }
                file.points.push([n[0], n[1], n[2]]);
                #[allow(clippy::cast_possible_truncation)] // Colours are kept as f32.
                file.colours
                    .push((n.len() >= 6).then(|| [n[3] as f32, n[4] as f32, n[5] as f32]));
            }
            "vt" => {
                let n = numbers(words)?;
                let u = *n
                    .first()
                    .ok_or_else(|| ReadError::at(start, "a texture coordinate without numbers"))?;
                file.texture.push([u, n.get(1).copied().unwrap_or(0.0)]);
            }
            "vn" => {
                let n = numbers(words)?;
                if n.len() < 3 {
                    return Err(ReadError::at(start, "a normal without three numbers"));
                }
                file.normals.push([n[0], n[1], n[2]]);
            }
            "f" | "fo" => {
                let face = words
                    .map(|w| {
                        file.corner(w).ok_or_else(|| {
                            ReadError::at(
                                start,
                                format!(
                                    "the face corner \"{w}\" names something the file does not have"
                                ),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                faces.push(face);
            }
            "o" | "g" => {
                file.finish(std::mem::take(&mut name), &mut faces, &mut contents)?;
                name = words.collect::<Vec<_>>().join(" ");
            }
            "l" => contents.skip("lines", 1),
            "p" => contents.skip("points", 1),
            "mtllib" => contents.skip("material files", 1),
            "curv" | "curv2" => contents.skip("curves", 1),
            "surf" => contents.skip("surfaces", 1),
            // Smoothing groups, material choices and the free-form settings change nothing Curva
            // keeps.
            _ => {}
        }
    }
    file.finish(name, &mut faces, &mut contents)?;
    Ok(contents)
}

/// The file's shared lists.
#[derive(Default)]
struct File {
    points: Vec<Point>,
    colours: Vec<Option<[f32; 3]>>,
    texture: Vec<[f64; 2]>,
    normals: Vec<Point>,
}

impl File {
    /// A face corner, `v`, `v/vt`, `v//vn` or `v/vt/vn`, as places in the lists. Numbers count
    /// from 1, and negative ones back from the last so far.
    fn corner(&self, word: &str) -> Option<Corner> {
        let mut parts = word.split('/');
        let place = |part: Option<&str>, len: usize| -> Option<Option<usize>> {
            match part {
                None | Some("") => Some(None),
                Some(text) => {
                    let n: i64 = text.parse().ok()?;
                    let len = i64::try_from(len).ok()?;
                    let at = if n < 0 { len + n } else { n - 1 };
                    (0..len)
                        .contains(&at)
                        .then(|| usize::try_from(at).ok())
                        .flatten()
                        .map(Some)
                }
            }
        };
        let v = place(parts.next(), self.points.len())??;
        let vt = place(parts.next(), self.texture.len())?;
        let vn = place(parts.next(), self.normals.len())?;
        Some((v, vt, vn))
    }

    /// The faces so far as one mesh, with only the points they use.
    fn finish(
        &self,
        name: String,
        faces: &mut Vec<Vec<Corner>>,
        contents: &mut Contents,
    ) -> Result<(), ReadError> {
        let faces = std::mem::take(faces);
        let (kept, bad): (Vec<_>, Vec<_>) = faces.into_iter().partition(|face| {
            face.len() >= 3 && (1..face.len()).all(|i| face[..i].iter().all(|c| c.0 != face[i].0))
        });
        contents.skip("faces with fewer than three different corners", bad.len());
        if kept.is_empty() {
            return Ok(());
        }
        // The points the faces use, kept in file order, so a mesh written by Curva reads back
        // with its points numbered as before.
        let mut used: Vec<usize> = kept.iter().flatten().map(|c| c.0).collect();
        used.sort_unstable();
        used.dedup();
        let local: HashMap<usize, u32> = used
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, u32::try_from(i).unwrap_or(u32::MAX)))
            .collect();
        let rings: Vec<Vec<u32>> = kept
            .iter()
            .map(|face| face.iter().map(|c| local[&c.0]).collect())
            .collect();
        let corners: Vec<Corner> = kept.into_iter().flatten().collect();
        let mesh = Mesh::new(used.iter().map(|&v| self.points[v]).collect(), &rings)
            .map_err(|e| ReadError::new(e.to_string()))?;

        let texture = attribute(&corners, |c| c.1, &self.texture, used.len(), &local);
        let normals = attribute(&corners, |c| c.2, &self.normals, used.len(), &local);
        let colours: Option<Vec<[f32; 3]>> = used.iter().map(|&v| self.colours[v]).collect();
        let (texture, missing_texture) = texture;
        let (normals, missing_normals) = normals;
        if missing_texture {
            contents.skip("texture coordinates on only some faces", 1);
        }
        if missing_normals {
            contents.skip("normals on only some faces", 1);
        }
        let mesh = mesh
            .with_texture(texture)
            .and_then(|m| m.with_normals(normals))
            .and_then(|m| m.with_colours(colours.map_or(Attribute::None, Attribute::PerVertex)))
            .map_err(|e| ReadError::new(e.to_string()))?;
        contents.meshes.push(Named { name, mesh });
        Ok(())
    }
}

/// Texture coordinates or normals for a mesh from its corners: one per point when each point
/// always names the same one, else one per corner; none, and `true`, when only some corners name
/// one.
fn attribute<T: Copy>(
    corners: &[Corner],
    pick: impl Fn(&Corner) -> Option<usize>,
    list: &[T],
    points: usize,
    local: &HashMap<usize, u32>,
) -> (Attribute<T>, bool) {
    let named: Option<Vec<usize>> = corners.iter().map(&pick).collect();
    let Some(named) = named else {
        return (Attribute::None, corners.iter().any(|c| pick(c).is_some()));
    };
    let mut per_point: Vec<Option<usize>> = vec![None; points];
    let shared = corners.iter().zip(&named).all(|(c, &n)| {
        let slot = &mut per_point[local[&c.0] as usize];
        *slot.get_or_insert(n) == n
    });
    if shared {
        let values = per_point.iter().map(|n| list[n.unwrap_or(0)]).collect();
        (Attribute::PerVertex(values), false)
    } else {
        (
            Attribute::PerCorner(named.iter().map(|&n| list[n]).collect()),
            false,
        )
    }
}

/// The meshes as one OBJ file, each an `o` of its own name. Numbers are written in full, so a
/// mesh reads back exactly.
///
/// OBJ keeps colours per point, so a mesh with a colour per corner is written with each point
/// taking the colour of its first corner.
#[must_use]
pub fn write(meshes: &[(&str, &Mesh)]) -> String {
    let mut out = String::from("# Written by Curva\n");
    let (mut v0, mut vt0, mut vn0) = (1usize, 1usize, 1usize);
    for (name, mesh) in meshes {
        let name = name.replace(['\r', '\n'], " ");
        let _ = writeln!(
            out,
            "o {}",
            if name.trim().is_empty() {
                "mesh"
            } else {
                name.trim()
            }
        );
        let colours = point_colours(mesh);
        for (i, [x, y, z]) in mesh.vertices().iter().enumerate() {
            match colours.as_ref().map(|c| c[i]) {
                Some([r, g, b]) => {
                    let _ = writeln!(out, "v {x} {y} {z} {r} {g} {b}");
                }
                None => {
                    let _ = writeln!(out, "v {x} {y} {z}");
                }
            }
        }
        let texture = values(mesh.texture());
        for [u, v] in texture {
            let _ = writeln!(out, "vt {u} {v}");
        }
        let normals = values(mesh.normals());
        for [x, y, z] in normals {
            let _ = writeln!(out, "vn {x} {y} {z}");
        }
        for f in 0..mesh.face_count() {
            out.push('f');
            for corner in mesh.face_range(f) {
                let v = mesh.corners()[corner] as usize;
                // Kept per point: the point's own entry; per corner: the corner's.
                let place = |per_vertex: Option<bool>, start: usize| {
                    per_vertex.map(|by_point| start + if by_point { v } else { corner })
                };
                let vt = place(per_vertex(mesh.texture()), vt0);
                let vn = place(per_vertex(mesh.normals()), vn0);
                let _ = match (vt, vn) {
                    (None, None) => write!(out, " {}", v0 + v),
                    (Some(t), None) => write!(out, " {}/{t}", v0 + v),
                    (None, Some(n)) => write!(out, " {}//{n}", v0 + v),
                    (Some(t), Some(n)) => write!(out, " {}/{t}/{n}", v0 + v),
                };
            }
            out.push('\n');
        }
        v0 += mesh.vertex_count();
        vt0 += texture.len();
        vn0 += normals.len();
    }
    out
}

/// Whether an attribute is kept per point (`Some(true)`), per corner (`Some(false)`), or not at
/// all.
fn per_vertex<T>(attribute: &Attribute<T>) -> Option<bool> {
    match attribute {
        Attribute::None => None,
        Attribute::PerVertex(_) => Some(true),
        Attribute::PerCorner(_) => Some(false),
    }
}

fn values<T>(attribute: &Attribute<T>) -> &[T] {
    match attribute {
        Attribute::None => &[],
        Attribute::PerVertex(v) | Attribute::PerCorner(v) => v,
    }
}

fn point_colours(mesh: &Mesh) -> Option<Vec<[f32; 3]>> {
    match mesh.colours() {
        Attribute::None => None,
        Attribute::PerVertex(c) => Some(c.clone()),
        Attribute::PerCorner(c) => {
            let mut out: Vec<Option<[f32; 3]>> = vec![None; mesh.vertex_count()];
            for (corner, &v) in mesh.corners().iter().enumerate() {
                out[v as usize].get_or_insert(c[corner]);
            }
            Some(out.into_iter().map(Option::unwrap_or_default).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stl::tests::cube;

    #[test]
    fn a_mesh_with_every_kind_of_data_reads_back_exactly() {
        // A cube with an L-shaped n-gon added, a texture coordinate per corner, a normal per
        // point and a colour per point, and awkward numbers.
        let mut cube = cube().translated([0.1, 1e9, -2.0 / 3.0]);
        let corners = cube.corners().len();
        let texture = (0..corners).map(|i| [i as f64 / 7.0, 0.3]).collect();
        let normals = (0..8).map(|i| [0.0, 0.0, f64::from(i)]).collect();
        let colours = (0..8).map(|i| [0.5, 0.25, i as f32 / 8.0]).collect();
        cube = cube
            .with_texture(Attribute::PerCorner(texture))
            .unwrap()
            .with_normals(Attribute::PerVertex(normals))
            .unwrap()
            .with_colours(Attribute::PerVertex(colours))
            .unwrap();
        let l = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
            ],
            &[[0u32, 1, 2, 3, 4, 5]],
        )
        .unwrap();
        let text = write(&[("box", &cube), ("ell", &l)]);
        let contents = read(text.as_bytes()).unwrap();
        assert!(contents.skipped.is_empty(), "{:?}", contents.skipped);
        assert_eq!(contents.meshes.len(), 2);
        assert_eq!(contents.meshes[0].name, "box");
        assert_eq!(contents.meshes[0].mesh, cube);
        assert_eq!(contents.meshes[1].mesh, l);
    }

    #[test]
    fn every_corner_form_and_negative_numbers_read() {
        let text = "# a square and a triangle
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vt 0 0
vn 0 0 1
f 1//1 2//1 3//1 4//1
f -4/1 -3/1 -2/1
";
        let contents = read(text.as_bytes()).unwrap();
        let mesh = &contents.meshes[0].mesh;
        assert_eq!(mesh.face(0), [0, 1, 2, 3]);
        assert_eq!(mesh.face(1), [0, 1, 2]);
        // Normals on the first face only: left out and said so.
        assert!(mesh.normals().is_none());
        assert_eq!(contents.skipped.len(), 2);
    }

    #[test]
    fn groups_become_separate_meshes_with_only_their_points() {
        let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 5 5 5\nv 6 5 5\nv 5 6 5\n\
                    g first\nf 1 2 3\ng second part\nf 4 5 \\\n 6\nl 1 2\nmtllib a.mtl\n";
        let contents = read(text.as_bytes()).unwrap();
        let names: Vec<&str> = contents.meshes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["first", "second part"]);
        assert_eq!(contents.meshes[1].mesh.vertex_count(), 3);
        assert_eq!(contents.meshes[1].mesh.bounds().unwrap().0, [5.0, 5.0, 5.0]);
        assert_eq!(
            contents.skipped,
            [("lines".to_owned(), 1), ("material files".to_owned(), 1)]
        );
    }

    #[test]
    fn bad_files_say_where() {
        let error = read(b"v 0 0 0\nv 1 x 0\n").unwrap_err();
        assert_eq!(error.line, Some(2));
        let error = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n").unwrap_err();
        assert_eq!(error.line, Some(4));
    }
}
