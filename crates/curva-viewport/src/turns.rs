//! Silhouettes of curved surfaces in views whose faces hide nothing (Wireframe, Ghosted), as
//! Rhino 8 draws them: the line where a curved face turns away from the viewer, such as a
//! cylinder's two sides, moving as the view turns. Views with solid faces find outlines in the
//! depth picture instead (`outline.rs`); here there is no depth picture of the faces to search.
//!
//! Each place where two triangles of one curved face meet at an angle is a turn. A turn is part
//! of the silhouette when one of its triangles faces the viewer and the other faces away: seen
//! on screen, both triangles' far corners then lie on the same side of the line they share. The
//! graphics card makes that test for every turn, every frame (`gpu/turns.rs`), so turning the
//! view costs nothing here. (Silhouette edges from facing changes: Markosian et al., "Real-time
//! nonphotorealistic rendering", SIGGRAPH 1997.)

use std::collections::HashMap;

use curva_kernel::{BodyMesh, Point};

use crate::{FaceKind, Outlines, narrow_from};

/// Two triangles meeting at less than this angle's cosine are a turn: their normals differ by
/// more than about a quarter of a degree, which every curved face's triangles do, while
/// triangles of a flat face, the same plane in a rounding error's difference, never do.
const BENT: f64 = 0.99999;

/// Meshes with more triangles than this draw no turns: a scan's million triangles have half a million,
/// each held placed for drawing, hundreds of megabytes for a few such meshes. Their open borders and
/// unwelded edges still draw.
pub(crate) const MOST_TURNED_TRIANGLES: usize = 200_000;

/// A turn as the graphics card draws it: the line two triangles share, from `a` to `b`, and the
/// far corner of each triangle, `c` and `d`, all measured from the scene's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedTurn {
    /// One end of the line the two triangles share.
    pub a: [f32; 3],
    /// Its other end.
    pub b: [f32; 3],
    /// The first triangle's far corner.
    pub c: [f32; 3],
    /// The second triangle's far corner.
    pub d: [f32; 3],
    /// Its object's colour when not selected.
    pub colour: [f32; 3],
    /// The object's number in [`crate::SceneParts::objects`].
    pub object: u32,
}

/// The turns of `mesh`, as indices into its positions: the shared line's two ends, then each
/// triangle's far corner. Only triangles of the same face count, so where two faces meet (an
/// edge, drawn already) is never a turn; corners in the same place are one, so a face's seam,
/// where its mesh starts again, turns as anywhere else.
pub(crate) fn mesh_turns(mesh: &BodyMesh) -> Vec<[u32; 4]> {
    let face_of = |triangle: usize| mesh.triangle_faces.get(triangle).copied().unwrap_or(0);
    // Each corner stands for the first corner of its face in the same place.
    let mut first = HashMap::new();
    let mut same = vec![0u32; mesh.positions.len()];
    for (triangle, corners) in mesh.triangles.iter().enumerate() {
        for &corner in corners {
            let Some(p) = mesh.positions.get(corner as usize) else {
                continue;
            };
            let key = (face_of(triangle), p.map(f64::to_bits));
            same[corner as usize] = *first.entry(key).or_insert(corner);
        }
    }
    // The triangles on each side of each line, with their far corners.
    let mut sides: HashMap<(u32, u32), Vec<(usize, u32)>> = HashMap::new();
    for (triangle, corners) in mesh.triangles.iter().enumerate() {
        let welded = corners.map(|corner| same.get(corner as usize).copied().unwrap_or(corner));
        for k in 0..3 {
            let (a, b, far) = (welded[k], welded[(k + 1) % 3], corners[(k + 2) % 3]);
            if a != b {
                sides
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push((triangle, far));
            }
        }
    }
    let normal = |triangle: usize| {
        let [a, b, c] = mesh.triangles[triangle].map(|i| mesh.positions[i as usize]);
        unit(cross(minus(b, a), minus(c, a)))
    };
    let mut turns: Vec<[u32; 4]> = sides
        .into_iter()
        .filter_map(|((a, b), triangles)| match triangles[..] {
            [(one, c), (other, d)] if dot(normal(one), normal(other)) < BENT => Some([a, b, c, d]),
            _ => None,
        })
        .collect();
    turns.sort_unstable();
    turns
}

/// A mesh's turns and its outline edges, from one pass over the sides of its triangles sorted by
/// their two vertex numbers (#663; a hash map of every side took 1.5 s on a scan's million
/// triangles).
///
/// The turns are as its faces are drawn ([`crate::meshes::body_mesh`], which gives triangle `t`
/// corners `3t` to `3t + 2`): triangles meeting at an angle on a side they share by vertex
/// number, so where the mesh is welded, however sharp. The outline edges are the sides used by
/// one triangle only, by vertex number: its open borders and both sides of every unwelded seam
/// ([`crate::meshes::outline`]), as Rhino 8 draws them; a face's own diagonals are always
/// shared, so these are the edges of its faces used once.
pub(crate) fn mesh_turns_and_borders(mesh: &curva_mesh::Mesh) -> (Vec<[u32; 4]>, Vec<[u32; 2]>) {
    let split = mesh.triangles();
    let points = mesh.vertices();
    let valid = |triangle: &[u32; 3]| triangle.iter().all(|&v| (v as usize) < points.len());
    // Each side as its two vertex numbers, lower first, packed into one number to sort by, with
    // its triangle and which side of it.
    let mut sides: Vec<(u64, u32, u8)> = Vec::with_capacity(split.triangles.len() * 3);
    for (t, triangle) in split.triangles.iter().enumerate() {
        let Ok(t) = u32::try_from(t) else { break };
        if !valid(triangle) {
            continue;
        }
        for k in 0..3u8 {
            let (a, b) = (triangle[k as usize], triangle[(k as usize + 1) % 3]);
            if a != b {
                sides.push(((u64::from(a.min(b)) << 32) | u64::from(a.max(b)), t, k));
            }
        }
    }
    sides.sort_unstable_by_key(|side| side.0);
    let normals: Vec<Point> = split
        .triangles
        .iter()
        .map(|triangle| {
            if !valid(triangle) || split.triangles.len() > MOST_TURNED_TRIANGLES {
                return [0.0; 3];
            }
            let [a, b, c] = triangle.map(|v| points[v as usize]);
            unit(cross(minus(b, a), minus(c, a)))
        })
        .collect();
    let turned = split.triangles.len() <= MOST_TURNED_TRIANGLES;
    let corner = |t: u32, k: u8| 3 * t + u32::from(k % 3);
    let (mut turns, mut borders) = (Vec::new(), Vec::new());
    for run in sides.chunk_by(|a, b| a.0 == b.0) {
        match *run {
            [(key, ..)] => borders.push([(key >> 32) as u32, key as u32]),
            [(_, one, k), (_, other, j)]
                if turned && dot(normals[one as usize], normals[other as usize]) < BENT =>
            {
                turns.push([
                    corner(one, k),
                    corner(one, k + 1),
                    corner(one, k + 2),
                    corner(other, j + 2),
                ]);
            }
            _ => {}
        }
    }
    (turns, borders)
}

/// The turns of every curved surface and solid in `outlines`, and of every mesh, placed where
/// each copy sits, numbered by `numbers` and measured from `origin`. SubDs are left out.
pub(crate) fn turns_of(
    outlines: &Outlines,
    numbers: &HashMap<u64, u32>,
    origin: Point,
) -> Vec<PlacedTurn> {
    let mut out = Vec::new();
    let mut known: HashMap<*const BodyMesh, std::sync::Arc<Vec<[u32; 4]>>> = HashMap::new();
    for faces in outlines.faces.iter() {
        let turns = match (&faces.turns, faces.face_kind) {
            (Some(given), _) => given,
            (None, FaceKind::Nurbs) => known
                .entry(std::sync::Arc::as_ptr(&faces.mesh))
                .or_insert_with(|| std::sync::Arc::new(mesh_turns(&faces.mesh))),
            _ => continue,
        };
        let object = numbers.get(&faces.id).copied().unwrap_or(crate::NO_OBJECT);
        let at = |corner| faces.corner(corner).map(|p| narrow_from(p, origin));
        for &[a, b, c, d] in turns.iter() {
            if let (Some(a), Some(b), Some(c), Some(d)) = (at(a), at(b), at(c), at(d)) {
                out.push(PlacedTurn {
                    a,
                    b,
                    c,
                    d,
                    colour: faces.colour,
                    object,
                });
            }
        }
    }
    out
}

fn minus(a: Point, b: Point) -> Point {
    std::array::from_fn(|k| a[k] - b[k])
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: Point) -> Point {
    let length = dot(a, a).sqrt();
    if length > 0.0 {
        a.map(|v| v / length)
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `n` squares bent round a quarter of a cylinder, or lying flat, one face.
    fn strip(n: usize, bent: bool) -> BodyMesh {
        let mut mesh = BodyMesh::default();
        for i in 0..=n {
            let t = i as f64 / n as f64 * std::f64::consts::FRAC_PI_2;
            let (x, y) = if bent { (t.cos(), t.sin()) } else { (1.0, t) };
            mesh.positions.push([x, y, 0.0]);
            mesh.positions.push([x, y, 1.0]);
        }
        mesh.normals = vec![[0.0, 0.0, 1.0]; mesh.positions.len()];
        for i in 0..n as u32 {
            let [a, b, c, d] = [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3];
            mesh.triangles.push([a, c, d]);
            mesh.triangles.push([a, d, b]);
        }
        mesh.triangle_faces = vec![0; mesh.triangles.len()];
        mesh
    }

    #[test]
    fn a_curved_face_turns_between_its_strips_and_a_flat_one_never() {
        // Between each pair of neighbouring squares, and not across a square's diagonal.
        assert_eq!(mesh_turns(&strip(4, true)).len(), 3);
        assert!(mesh_turns(&strip(4, false)).is_empty());
    }

    #[test]
    fn a_seam_turns_as_anywhere_else_but_where_two_faces_meet_does_not() {
        // The last square's corners repeated, as a mesh starts again at a seam.
        let mut mesh = strip(4, true);
        let start = mesh.positions.len() as u32;
        mesh.positions
            .extend([mesh.positions[8], mesh.positions[9]]);
        mesh.normals.extend([[0.0, 0.0, 1.0]; 2]);
        let more = [mesh.positions[8][0] - 0.3, 1.2];
        mesh.positions.push([more[0], more[1], 0.0]);
        mesh.positions.push([more[0], more[1], 1.0]);
        mesh.normals.extend([[0.0, 0.0, 1.0]; 2]);
        mesh.triangles.push([start, start + 2, start + 3]);
        mesh.triangles.push([start, start + 3, start + 1]);
        mesh.triangle_faces.extend([0, 0]);
        assert_eq!(mesh_turns(&mesh).len(), 4);
        // The same square as a face of its own meets the strip at an edge, not a turn.
        let last = mesh.triangle_faces.len();
        mesh.triangle_faces[last - 2..].fill(1);
        assert_eq!(mesh_turns(&mesh).len(), 3);
    }

    /// A mesh turns where it is welded and bent: a MeshCylinder up its smooth side, once between
    /// each two of its 16 columns on each of its 4 rows, and nowhere on its flat ends or where
    /// they meet the side unwelded; a MeshBox, unwelded at its edges, nowhere (#663).
    #[test]
    fn a_mesh_turns_where_welded_and_bent() {
        let frame = curva_mesh::Frame::world();
        let cylinder = curva_mesh::Mesh::mesh_cylinder(&frame, 5.0, 10.0, (4, 16), true, true);
        assert_eq!(mesh_turns_and_borders(&cylinder).0.len(), 16 * 4);
        let box_mesh = curva_mesh::Mesh::mesh_box(&frame, [10.0; 3], [4; 3]);
        assert!(mesh_turns_and_borders(&box_mesh).0.is_empty());
    }

    /// A mesh past [`MOST_TURNED_TRIANGLES`], such as a scan, draws no turns, though curved all over,
    /// and still finds its borders the same way as smaller meshes.
    #[test]
    fn a_mesh_past_the_limit_draws_no_turns_but_keeps_its_borders() {
        let frame = curva_mesh::Frame::world();
        let small = curva_mesh::Mesh::mesh_torus(&frame, 10.0, 3.0, (20, 40));
        assert!(!mesh_turns_and_borders(&small).0.is_empty());
        let big = curva_mesh::Mesh::mesh_torus(&frame, 10.0, 3.0, (300, 350));
        assert!(big.triangles().triangles.len() > MOST_TURNED_TRIANGLES);
        let (turns, borders) = mesh_turns_and_borders(&big);
        assert!(turns.is_empty());
        assert_eq!(borders, big.outline_edges(), "a closed ring has none");
    }
}
