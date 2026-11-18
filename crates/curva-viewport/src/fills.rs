//! Annotations' filled shapes (letters, arrowheads) as triangles, through lyon (decision 0030:
//! lyon turns fills into triangles). Each shape was made into a flat face by the modelling
//! engine and meshed: about 3 ms a letter, 16 of the 18 seconds a real drawing of 356 texts took
//! to open. A shape is flat, so its contours are sampled, laid in their plane and filled there.

use curva_kernel::{BodyMesh, Curve, Point};
use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};

/// One filled shape, its contours (the outline and any holes) as curves in one plane, added to
/// `mesh` as triangles facing the way its outline runs counter-clockwise. A hole is wherever the
/// contours overlap an even number of times, so their directions do not matter. `false` when
/// nothing could be filled (fewer than three distinct points, or not flat enough to have a plane).
pub(crate) fn add_filled(contours: &[Curve], mesh: &mut BodyMesh) -> bool {
    let rings: Vec<Vec<Point>> = contours
        .iter()
        .filter_map(|curve| curve.sample().ok())
        .map(|points| crate::simplify::fewer(points, Some(crate::simplify::PART_OF_SIZE)))
        .filter(|ring| ring.len() >= 3)
        .collect();
    let Some(outline) = rings.iter().max_by_key(|ring| ring.len()) else {
        return false;
    };
    let Some((origin, u, v, normal)) = plane_of(outline) else {
        return false;
    };
    let flat = |p: &Point| -> [f32; 2] {
        let d: Point = std::array::from_fn(|k| p[k] - origin[k]);
        // Measured from a point of the shape, so a letter far from the origin keeps its detail
        // in the 32-bit numbers lyon works in.
        #[allow(clippy::cast_possible_truncation)]
        [dot(d, u) as f32, dot(d, v) as f32]
    };
    let mut path = Path::builder();
    for ring in &rings {
        let [x, y] = flat(&ring[0]);
        path.begin(point(x, y));
        for p in &ring[1..] {
            let [x, y] = flat(p);
            path.line_to(point(x, y));
        }
        path.end(true);
    }
    let path = path.build();
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let filled = FillTessellator::new().tessellate_path(
        &path,
        &FillOptions::even_odd(),
        &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
            vertex.position().to_array()
        }),
    );
    if filled.is_err() || buffers.indices.len() < 3 {
        return false;
    }
    let base = u32::try_from(mesh.positions.len()).unwrap_or(u32::MAX);
    for [x, y] in &buffers.vertices {
        let (x, y) = (f64::from(*x), f64::from(*y));
        mesh.positions
            .push(std::array::from_fn(|k| origin[k] + x * u[k] + y * v[k]));
        mesh.normals.push(normal);
    }
    let (triangles, _) = buffers.indices.as_chunks::<3>();
    for triangle in triangles {
        // Counter-clockwise in the plane, so facing along its normal.
        let [a, b, c] = triangle.map(|i| i + base);
        let corner = |i: u32| buffers.vertices[(i - base) as usize];
        let (p, q, r) = (corner(a), corner(b), corner(c));
        let turn = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        mesh.triangles
            .push(if turn < 0.0 { [a, c, b] } else { [a, b, c] });
        mesh.triangle_faces.push(0);
    }
    true
}

/// The plane of a closed outline: a point on it, two directions across it, and its normal,
/// pointing the way from which the outline runs counter-clockwise (Newell's method).
fn plane_of(outline: &[Point]) -> Option<(Point, Point, Point, Point)> {
    let mut n = [0.0; 3];
    for (k, p) in outline.iter().enumerate() {
        let q = outline[(k + 1) % outline.len()];
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    let normal = unit(n)?;
    // Across the plane: the axis least along the normal, made square to it.
    let axis = if normal[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = unit(cross(axis, normal))?;
    let v = cross(normal, u);
    Some((outline[0], u, v, normal))
}

fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: Point) -> Option<Point> {
    let length = dot(a, a).sqrt();
    (length > 1e-300).then(|| a.map(|c| c / length))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(low: f64, high: f64, z: f64) -> Curve {
        Curve::polyline(
            vec![
                [low, low, z],
                [high, low, z],
                [high, high, z],
                [low, high, z],
            ],
            true,
        )
        .unwrap()
    }

    fn area(mesh: &BodyMesh) -> f64 {
        mesh.triangles
            .iter()
            .map(|&[a, b, c]| {
                let [p, q, r] = [a, b, c].map(|i| mesh.positions[i as usize]);
                let n = cross(
                    std::array::from_fn(|k| q[k] - p[k]),
                    std::array::from_fn(|k| r[k] - p[k]),
                );
                dot(n, n).sqrt() / 2.0
            })
            .sum()
    }

    /// A square with a square hole, as an "O" is, fills only the ring between them, facing up,
    /// and stays where it is far from the origin.
    #[test]
    fn a_shape_with_a_hole_fills_only_around_it() {
        let far = 1.0e6;
        let mut mesh = BodyMesh::default();
        assert!(add_filled(
            &[
                square(far, far + 10.0, 5.0),
                square(far + 3.0, far + 7.0, 5.0)
            ],
            &mut mesh
        ));
        assert!(
            (area(&mesh) - (100.0 - 16.0)).abs() < 1e-3,
            "{}",
            area(&mesh)
        );
        assert!(mesh.normals.iter().all(|n| (n[2] - 1.0).abs() < 1e-9));
        assert!(
            mesh.positions.iter().all(|p| (p[2] - 5.0).abs() < 1e-6
                && p[0] >= far - 1e-3
                && p[0] <= far + 10.0 + 1e-3)
        );
        // Each triangle turns counter-clockwise seen from its normal's side.
        for &[a, b, c] in &mesh.triangles {
            let [p, q, r] = [a, b, c].map(|i| mesh.positions[i as usize]);
            let n = cross(
                std::array::from_fn(|k| q[k] - p[k]),
                std::array::from_fn(|k| r[k] - p[k]),
            );
            assert!(n[2] > 0.0);
        }
    }

    #[test]
    fn a_shape_too_small_to_fill_adds_nothing() {
        let line = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        let mut mesh = BodyMesh::default();
        assert!(!add_filled(&[line], &mut mesh));
        assert!(mesh.triangles.is_empty());
    }
}
