//! Mesh objects (#267, spec `mesh-and-subd.md` §4, Stage A) as the triangles the viewport draws
//! bodies from, so Bevy shades them and picking and snapping find them as they do bodies.

use curva_kernel::BodyMesh;
use curva_kernel::Point;

/// A mesh's faces as triangles with a normal at each corner, in the form bodies are drawn from.
///
/// Normals the mesh carries are kept: per corner, so an unwelded edge shades as a hard crease;
/// per vertex, shared by the faces around it. A mesh without normals gets them from its faces,
/// averaged at each vertex, so welded edges shade smooth, as Rhino shades a mesh it reads without
/// normals. Each triangle is listed under the face it came from, for picking faces later (#333).
/// With it, the colour at each of its positions when the mesh has vertex colours, per vertex or
/// per corner as stored.
#[must_use]
pub(crate) fn body_mesh(mesh: &curva_mesh::Mesh) -> (BodyMesh, Option<Vec<[f32; 3]>>) {
    let split = mesh.triangles();
    let vertices = mesh.vertices();
    let given = !mesh.normals().is_none();
    let coloured = !mesh.colours().is_none();
    let mut colours = Vec::new();
    // Each corner of each triangle gets its own position, so a corner's normal is its own; a
    // smooth vertex repeats its normal at every corner, which looks the same.
    let mut out = BodyMesh {
        positions: Vec::with_capacity(split.triangles.len() * 3),
        normals: Vec::with_capacity(split.triangles.len() * 3),
        triangles: Vec::with_capacity(split.triangles.len()),
        triangle_faces: split.faces.iter().map(|&f| f as usize).collect(),
        ..BodyMesh::default()
    };
    let smooth = if given {
        Vec::new()
    } else {
        vertex_normals(vertices, &split.triangles)
    };
    for (triangle, corners) in split.triangles.iter().zip(&split.corners) {
        let first = u32::try_from(out.positions.len()).unwrap_or(u32::MAX);
        for (&vertex, &corner) in triangle.iter().zip(corners) {
            let v = vertex as usize;
            out.positions.push(vertices[v]);
            let normal = if given {
                mesh.normals()
                    .at(corner as usize, v)
                    .copied()
                    .unwrap_or_default()
            } else {
                smooth[v]
            };
            out.normals.push(normal);
            if coloured {
                colours.push(
                    mesh.colours()
                        .at(corner as usize, v)
                        .copied()
                        .unwrap_or([1.0; 3]),
                );
            }
        }
        out.triangles.push([first, first + 1, first + 2]);
    }
    (out, coloured.then_some(colours))
}

/// Each vertex's normal: the faces around it, each weighted by its area, added and made one long.
fn vertex_normals(vertices: &[Point], triangles: &[[u32; 3]]) -> Vec<Point> {
    let mut sums = vec![[0.0; 3]; vertices.len()];
    for &[a, b, c] in triangles {
        let (p, q, r) = (
            vertices[a as usize],
            vertices[b as usize],
            vertices[c as usize],
        );
        let (u, w) = (sub(q, p), sub(r, p));
        // Twice the triangle's area, pointing out of its counter-clockwise side.
        let n = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        for v in [a, b, c] {
            let s = &mut sums[v as usize];
            *s = std::array::from_fn(|k| s[k] + n[k]);
        }
    }
    sums.into_iter()
        .map(|s| {
            let length = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
            if length > 0.0 {
                s.map(|v| v / length)
            } else {
                s
            }
        })
        .collect()
}

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| a[k] - b[k])
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_mesh::{Attribute, Mesh};

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9)
    }

    #[test]
    fn a_quad_becomes_two_triangles_facing_up() {
        let quad = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[[0, 1, 2, 3]],
        )
        .unwrap();
        let (drawn, colours) = body_mesh(&quad);
        assert!(colours.is_none());
        assert_eq!(drawn.triangles.len(), 2);
        assert_eq!(drawn.triangle_faces, vec![0, 0]);
        assert!(drawn.normals.iter().all(|&n| close(n, [0.0, 0.0, 1.0])));
    }

    #[test]
    fn a_welded_fold_shades_smooth_and_given_corner_normals_stay_hard() {
        // Two triangles meeting at a right angle along the edge from vertex 0 to vertex 1.
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 0, 3]];
        let (welded, _) = body_mesh(&Mesh::new(points.clone(), &faces).unwrap());
        // On the shared edge both faces' corners carry the same, averaged normal.
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let on_edge: Vec<Point> = welded
            .positions
            .iter()
            .zip(&welded.normals)
            .filter(|(p, _)| p[1] == 0.0 && p[2] == 0.0)
            .map(|(_, &n)| n)
            .collect();
        assert_eq!(on_edge.len(), 4);
        assert!(on_edge.iter().all(|&n| close(n, [0.0, half, half])));

        let up = [0.0, 0.0, 1.0];
        let side = [0.0, 1.0, 0.0];
        let hard = Mesh::new(points, &faces)
            .unwrap()
            .with_normals(Attribute::PerCorner(vec![up, up, up, side, side, side]))
            .unwrap();
        let (drawn, _) = body_mesh(&hard);
        assert!(drawn.normals[..3].iter().all(|&n| close(n, up)));
        assert!(drawn.normals[3..].iter().all(|&n| close(n, side)));
    }

    /// A mesh with vertex colours shows them instead of its layer's colour, in pictures and in
    /// the window's shared shape alike, and the selection colour when selected (#267).
    #[test]
    fn a_mesh_shows_its_vertex_colours() {
        use crate::{Outlines, SceneParts, face_vertices};
        let (red, blue) = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let mesh = Mesh::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[[0, 1, 2]],
        )
        .unwrap()
        .with_colours(Attribute::PerVertex(vec![red, blue, red]))
        .unwrap();
        let mut doc = curva_document::Document::default();
        let id = doc.add_shape(curva_document::Shape::Mesh(mesh));
        let outlines = Outlines::of(doc.state());
        let shown: Vec<[f32; 3]> = face_vertices(&outlines, &[])
            .iter()
            .map(|corner| corner.colour)
            .collect();
        assert_eq!(shown, vec![red, blue, red]);
        let shared = SceneParts::shared(&outlines);
        assert_eq!(shared.shapes[0].colour, [1.0; 3]);
        assert_eq!(
            shared.shapes[0].colours.as_deref(),
            Some(&vec![red, blue, red])
        );
        let chosen = face_vertices(&outlines, &[id]);
        assert!(
            chosen
                .iter()
                .all(|corner| corner.colour == crate::SELECTED_COLOUR)
        );
    }

    /// A closed mesh in the model shades, is picked by a click inside it as a solid, and draws
    /// its wires only in wireframe (#267).
    #[test]
    fn a_mesh_in_the_model_shades_picks_and_draws_wires_in_wireframe() {
        use crate::{Outlines, SceneParts, StandardView, object_lines, shaded_object_lines};
        const SIZE: [f64; 2] = [800.0, 600.0];
        let box_mesh = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [400.0, 0.0, 0.0],
                [400.0, 400.0, 0.0],
                [0.0, 400.0, 0.0],
                [0.0, 0.0, 40.0],
                [400.0, 0.0, 40.0],
                [400.0, 400.0, 40.0],
                [0.0, 400.0, 40.0],
            ],
            &[
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
            ],
        )
        .unwrap();
        let mut doc = curva_document::Document::default();
        let id = doc.add_shape(curva_document::Shape::Mesh(box_mesh));
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = camera.project([200.0, 200.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), Some(id));
        assert_eq!(outlines.kinds, vec![crate::PickKind::Solid; 12]);
        // Twelve wires, all left out where faces show.
        assert!(shaded_object_lines(&outlines, &[]).is_empty());
        assert_eq!(object_lines(&outlines, &[]).len(), 24);
        // Twelve triangles: for pictures written out, and for the window one shared shape.
        assert_eq!(SceneParts::of(&outlines).faces.len(), 36);
        assert_eq!(SceneParts::shared(&outlines).shapes.len(), 1);
    }
}
