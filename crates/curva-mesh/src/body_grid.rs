//! Rhino's `Mesh` of a body: its untrimmed faces as grids of quads, the rest as the kernel's
//! triangles (#733). Each face keeps its own points, as Rhino's `Mesh` gives one point per face
//! at each place along an edge.

use crate::{Attribute, Mesh, Point};
use curva_kernel::{BodyMesh, NurbsSurface};

impl Mesh {
    /// A body's mesh from the kernel's triangles `made`, with each face that has an untrimmed
    /// surface in `grids` (by face number) gridded in quads instead, at the distance `chord` and
    /// `angle` (radians). A grid faces the way the kernel's triangles of that face do.
    #[must_use]
    pub fn from_body_with_grids(
        made: &BodyMesh,
        grids: &[Option<NurbsSurface>],
        chord: f64,
        angle: f64,
    ) -> Self {
        let face_count = made
            .triangle_faces
            .iter()
            .copied()
            .max()
            .map_or(0, |f| f + 1)
            .max(grids.len());
        let mut parts: Vec<Mesh> = Vec::with_capacity(face_count);
        for face in 0..face_count {
            let triangles = triangles_of(made, face);
            let part = match grids.get(face).and_then(Option::as_ref) {
                Some(surface) if !triangles.triangles.is_empty() => {
                    let grid = Mesh::surface_grid(surface, chord, angle, false);
                    if faces_against(&grid, &triangles) {
                        grid.flipped()
                    } else {
                        grid
                    }
                }
                _ => Mesh::from_body_mesh(&triangles).without_unused_vertices().0,
            };
            if part.face_count() > 0 {
                parts.push(part);
            }
        }
        let refs: Vec<&Mesh> = parts.iter().collect();
        Mesh::joined(&refs).unwrap_or_default()
    }
}

/// The kernel's triangles of face `face` alone, on all the points.
fn triangles_of(made: &BodyMesh, face: usize) -> BodyMesh {
    let triangles: Vec<[u32; 3]> = made
        .triangles
        .iter()
        .zip(&made.triangle_faces)
        .filter(|&(_, &f)| f == face)
        .map(|(t, _)| *t)
        .collect();
    BodyMesh {
        positions: made.positions.clone(),
        normals: made.normals.clone(),
        triangle_faces: vec![face; triangles.len()],
        triangles,
        ..BodyMesh::default()
    }
}

/// Whether the face's triangles face against the grid: the grid's normal at its point nearest
/// the first triangle's middle, against that triangle's.
fn faces_against(grid: &Mesh, triangles: &BodyMesh) -> bool {
    let Some(t) = triangles.triangles.first() else {
        return false;
    };
    let [a, b, c] = t.map(|v| triangles.positions[v as usize]);
    let middle: Point = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
    let along = curva_kernel::cross(
        std::array::from_fn(|k| b[k] - a[k]),
        std::array::from_fn(|k| c[k] - a[k]),
    );
    let Attribute::PerVertex(normals) = grid.normals() else {
        return false;
    };
    let gap = |v: usize| {
        let p = grid.vertices()[v];
        (0..3).map(|k| (p[k] - middle[k]).powi(2)).sum::<f64>()
    };
    let nearest = (0..grid.vertex_count()).min_by(|&i, &j| gap(i).total_cmp(&gap(j)));
    nearest.is_some_and(|v| (0..3).map(|k| normals[v][k] * along[k]).sum::<f64>() < 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Body;

    /// A face's surface as the kernel will give it (#749), here built by hand: the box's faces are
    /// untrimmed planes, so each is one quad.
    fn plane(corner: Point, x: Point, y: Point) -> NurbsSurface {
        let at =
            |a: f64, b: f64| -> Point { std::array::from_fn(|k| corner[k] + a * x[k] + b * y[k]) };
        NurbsSurface::new(
            [1, 1],
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            &[
                vec![at(0.0, 0.0), at(0.0, 1.0)],
                vec![at(1.0, 0.0), at(1.0, 1.0)],
            ],
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_box_of_untrimmed_faces_is_six_quads_facing_out() {
        let body = Body::cuboid([0.0; 3], [10.0; 3]).unwrap();
        let made = body.mesh().unwrap();
        // Each face's plane, found from the kernel's triangles of that face: its corners.
        let grids: Vec<Option<NurbsSurface>> = (0..6)
            .map(|face| {
                let t = triangles_of(&made, face);
                let mut points: Vec<Point> = t
                    .triangles
                    .iter()
                    .flatten()
                    .map(|&v| t.positions[v as usize])
                    .collect();
                points.sort_by(|a, b| a.partial_cmp(b).unwrap());
                points.dedup();
                let (o, far) = (points[0], points[points.len() - 1]);
                let side = |k: usize| -> Point {
                    std::array::from_fn(|i| if i == k { far[k] - o[k] } else { 0.0 })
                };
                let axes: Vec<Point> = (0..3).filter(|&k| far[k] != o[k]).map(side).collect();
                Some(plane(o, axes[0], axes[1]))
            })
            .collect();
        let (chord, angle) = crate::density_tolerances(0.5, 17.32);
        let mesh = Mesh::from_body_with_grids(&made, &grids, chord, angle);
        assert_eq!((mesh.vertex_count(), mesh.face_count()), (24, 6));
        let volume = mesh.welded(1e-9).0.volume().unwrap();
        assert!((volume - 1000.0).abs() < 1e-6, "{volume}");
    }

    #[test]
    fn faces_without_a_grid_keep_the_kernels_triangles() {
        let body = Body::cuboid([0.0; 3], [10.0; 3]).unwrap();
        let made = body.mesh().unwrap();
        let mesh = Mesh::from_body_with_grids(&made, &[], 1.0, 0.3);
        assert_eq!(mesh.face_count(), made.triangles.len());
        assert!((mesh.welded(1e-9).0.volume().unwrap() - 1000.0).abs() < 1e-6);
    }
}
