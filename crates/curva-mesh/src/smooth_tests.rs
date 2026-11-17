//! Tests of the smooth surface (`smooth.rs`), against Catmull–Clark at double precision and the
//! bicubic B-spline.

use super::*;
use crate::mesh::tests::cube;
use std::collections::HashMap;

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Point, s: f64) -> Point {
    a.map(|c| c * s)
}

/// One level of Catmull–Clark at double precision for a closed, smooth cage, as Catmull and Clark
/// give it (Recursively generated B-spline surfaces on arbitrary topological meshes, 1978): face
/// points, edge points, then the old points moved; each face becomes quads.
fn reference_level(points: &[Point], faces: &[Vec<u32>]) -> (Vec<Point>, Vec<Vec<u32>>) {
    let n = points.len();
    let face_point: Vec<Point> = faces
        .iter()
        .map(|f| {
            let sum = f.iter().fold([0.0; 3], |s, &v| add(s, points[v as usize]));
            scale(sum, 1.0 / f.len() as f64)
        })
        .collect();
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, f) in faces.iter().enumerate() {
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            edge_faces.entry((a.min(b), a.max(b))).or_default().push(fi);
        }
    }
    let mut out: Vec<Point> = points.to_vec();
    out.extend(face_point.iter().copied());
    let mut edges: Vec<(u32, u32)> = edge_faces.keys().copied().collect();
    edges.sort_unstable();
    let mut edge_index = HashMap::new();
    for &(a, b) in &edges {
        let fs = &edge_faces[&(a, b)];
        let sum = add(
            add(points[a as usize], points[b as usize]),
            add(face_point[fs[0]], face_point[fs[1]]),
        );
        edge_index.insert((a, b), out.len());
        out.push(scale(sum, 0.25));
    }
    // Old points: (F + 2R + (n - 3) P) / n.
    for (v, slot) in out.iter_mut().enumerate().take(n) {
        let v32 = u32::try_from(v).unwrap();
        let around: Vec<usize> = (0..faces.len())
            .filter(|&f| faces[f].contains(&v32))
            .collect();
        let valence = around.len() as f64;
        let f_avg = scale(
            around.iter().fold([0.0; 3], |s, &f| add(s, face_point[f])),
            1.0 / valence,
        );
        let mids: Vec<Point> = edges
            .iter()
            .filter(|e| e.0 == v32 || e.1 == v32)
            .map(|&(a, b)| scale(add(points[a as usize], points[b as usize]), 0.5))
            .collect();
        let r_avg = scale(
            mids.iter().fold([0.0; 3], |s, &m| add(s, m)),
            1.0 / mids.len() as f64,
        );
        *slot = scale(
            add(
                add(f_avg, scale(r_avg, 2.0)),
                scale(points[v], valence - 3.0),
            ),
            1.0 / valence,
        );
    }
    let mut quads = Vec::new();
    for (fi, f) in faces.iter().enumerate() {
        let e = |a: u32, b: u32| u32::try_from(edge_index[&(a.min(b), a.max(b))]).unwrap();
        for k in 0..f.len() {
            let (prev, here, next) = (f[(k + f.len() - 1) % f.len()], f[k], f[(k + 1) % f.len()]);
            let centre = u32::try_from(n + fi).unwrap();
            quads.push(vec![here, e(here, next), centre, e(prev, here)]);
        }
    }
    (out, quads)
}

/// The farthest any of `reference` lies from the nearest of `ours`.
fn farthest_apart(ours: &[Point], reference: &[Point]) -> f64 {
    reference
        .iter()
        .map(|r| {
            ours.iter()
                .map(|p| {
                    ((p[0] - r[0]).powi(2) + (p[1] - r[1]).powi(2) + (p[2] - r[2]).powi(2)).sqrt()
                })
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}

#[test]
fn a_cube_refines_to_quads_by_the_usual_counts() {
    let subd = SubD::new(cube());
    let one = subd.refined(1).unwrap();
    assert_eq!((one.vertex_count(), one.face_count()), (26, 24));
    let two = subd.refined(2).unwrap();
    assert_eq!((two.vertex_count(), two.face_count()), (98, 96));
    assert!(two.faces().all(|f| f.len() == 4) && two.is_closed());
}

/// The precision measurement of spec §3.2: the library's weights are single precision. A 10 m
/// cube (10 000 mm), a million millimetres from the origin, refined three times by the library
/// and by Catmull–Clark at double precision, must agree well within a hundredth of a millimetre.
#[test]
fn single_precision_weights_move_a_ten_metre_subd_by_far_less_than_a_hundredth_of_a_millimetre() {
    let big = cube()
        .mapped(&curva_kernel::Similarity::scaling([0.0; 3], 10_000.0))
        .translated([1.0e6, -2.0e6, 3.0e5]);
    let ours = SubD::new(big.clone()).refined(3).unwrap();
    let (mut points, mut net): (Vec<Point>, Vec<Vec<u32>>) = (
        big.vertices().to_vec(),
        big.faces().map(<[u32]>::to_vec).collect(),
    );
    for _ in 0..3 {
        (points, net) = reference_level(&points, &net);
    }
    assert_eq!(ours.vertex_count(), points.len());
    let off = farthest_apart(ours.vertices(), &points);
    println!("ten-metre cube, three levels: farthest apart {off:e} mm");
    assert!(off < 1e-3, "off by {off} mm");
}

/// Spec §7's exactness test: on a regular grid (every point meeting four faces, no creases) the
/// smooth surface is the bicubic B-spline surface of the cage. A torus-shaped cage is regular
/// everywhere. At a cage point the B-spline is (16 P + 4 × its edge neighbours + its diagonal
/// neighbours) / 36, and at a face's middle each direction weighs its four control rows 1/48,
/// 23/48, 23/48, 1/48.
#[test]
fn on_a_regular_grid_the_smooth_surface_is_the_bicubic_b_spline() {
    let (nu, nv) = (8i64, 6i64);
    let at = |i: i64, j: i64| -> u32 {
        u32::try_from(i.rem_euclid(nu) * nv + j.rem_euclid(nv)).unwrap()
    };
    let mut points = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let (a, b) = (
                std::f64::consts::TAU * i as f64 / nu as f64,
                std::f64::consts::TAU * j as f64 / nv as f64,
            );
            // A lumpy torus, so nothing lines up by luck.
            let r = 3.0 + (1.0 + 0.2 * ((i * 7 + j * 3) % 5) as f64) * b.cos();
            points.push([
                r * a.cos(),
                r * a.sin(),
                (1.0 + 0.1 * (i % 3) as f64) * b.sin(),
            ]);
        }
    }
    let faces: Vec<[u32; 4]> = (0..nu)
        .flat_map(|i| (0..nv).map(move |j| (i, j)))
        .map(|(i, j)| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)])
        .collect();
    let subd = SubD::new(Mesh::new(points.clone(), &faces).unwrap());
    let (result, centre, refined) = subd.refine(1).unwrap();
    let limit = result.limit_stencils().unwrap();
    let on_surface: Vec<Point> = limit
        .position
        .interpolate(&refined)
        .iter()
        .map(|p| std::array::from_fn(|k| p[k] + centre[k]))
        .collect();
    let p = |i: i64, j: i64| points[at(i, j) as usize];
    let weighted = |terms: &[(f64, Point)]| -> Point {
        terms.iter().fold([0.0; 3], |s, (w, q)| {
            std::array::from_fn(|k| s[k] + w * q[k])
        })
    };
    let mut worst: f64 = 0.0;
    let mut checked = 0;
    for (r, origin) in result.lineage.vertex_origin.iter().enumerate() {
        let expected = match *origin {
            subdiv_kernels::VertexOrigin::Vertex(v) => {
                let (i, j) = (i64::from(v) / nv, i64::from(v) % nv);
                let mut terms = vec![(16.0 / 36.0, p(i, j))];
                for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    terms.push((4.0 / 36.0, p(i + di, j + dj)));
                }
                for (di, dj) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    terms.push((1.0 / 36.0, p(i + di, j + dj)));
                }
                weighted(&terms)
            }
            subdiv_kernels::VertexOrigin::Face(f) => {
                let first = i64::from(faces[f as usize][0]);
                let (i, j) = (first / nv, first % nv);
                let w = [1.0 / 48.0, 23.0 / 48.0, 23.0 / 48.0, 1.0 / 48.0];
                let mut terms = Vec::new();
                for (a, wa) in w.iter().enumerate() {
                    for (b, wb) in w.iter().enumerate() {
                        terms.push((wa * wb, p(i - 1 + a as i64, j - 1 + b as i64)));
                    }
                }
                weighted(&terms)
            }
            subdiv_kernels::VertexOrigin::Edge(_) => continue,
        };
        let got = on_surface[r];
        worst = worst.max(
            (0..3)
                .map(|k| (got[k] - expected[k]).abs())
                .fold(0.0, f64::max),
        );
        checked += 1;
    }
    assert_eq!(checked, 2 * 48, "every cage point and face middle");
    println!("regular grid: worst difference from the B-spline {worst:e}");
    // Exact but for the library's single-precision weights, on a cage about 8 units across.
    assert!(worst < 1e-5, "worst {worst}");
}

#[test]
fn a_cube_creased_on_every_edge_stays_a_cube() {
    let edges: Vec<[u32; 2]> = SubD::new(cube())
        .edge_uses()
        .into_keys()
        .map(|(a, b)| [a, b])
        .collect();
    let subd = SubD::new(cube()).with_creases(&edges);
    assert!((0..8).all(|v| subd.vertex_tag(v) == VertexTag::Corner));
    let surface = subd.surface(3).unwrap();
    assert!(surface.is_closed());
    assert!((surface.volume().unwrap() - 1.0).abs() < 1e-6);
    let (lo, hi) = surface.bounds().unwrap();
    // Within the library's single-precision weights.
    assert!(lo.iter().all(|c| c.abs() < 1e-6) && hi.iter().all(|c| (c - 1.0).abs() < 1e-6));
}

#[test]
fn a_smooth_cube_becomes_a_rounded_closed_surface_with_outward_normals() {
    let surface = SubD::new(cube()).surface(3).unwrap();
    assert!(surface.is_closed());
    let volume = surface.volume().unwrap();
    assert!(volume > 0.3 && volume < 1.0, "volume {volume}");
    let Attribute::PerVertex(n) = surface.normals() else {
        panic!("one normal per point")
    };
    let low = (0..surface.vertex_count())
        .min_by(|&a, &b| surface.vertices()[a][2].total_cmp(&surface.vertices()[b][2]))
        .unwrap();
    assert!(n[low][2] < -0.99, "{:?}", n[low]);
}

#[test]
fn an_open_grid_keeps_the_corners_it_is_given() {
    let points: Vec<Point> = (0..4u32)
        .flat_map(|j| {
            (0..4u32).map(move |i| [f64::from(i), f64::from(j), f64::from((i * j) % 3) * 0.2])
        })
        .collect();
    let at = |i: u32, j: u32| j * 4 + i;
    let faces: Vec<[u32; 4]> = (0..3)
        .flat_map(|j| (0..3).map(move |i| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]))
        .collect();
    let surface = SubD::new(Mesh::new(points.clone(), &faces).unwrap())
        .with_corners(&[0, 3, 12, 15])
        .surface(2)
        .unwrap();
    for corner in [0usize, 3, 12, 15] {
        let moved = farthest_apart(surface.vertices(), &[points[corner]]);
        assert!(moved < 1e-9, "corner {corner} moved by {moved}");
    }
    assert!(!surface.is_closed());
}

/// A cube cage of side 2 from the origin.
fn cube2() -> Mesh {
    cube().mapped(&curva_kernel::Similarity::scaling([0.0; 3], 2.0))
}

// Rhino 8's limit surfaces of the same cages (`SubD.CreateFromMesh`, then `ToBrep`, 17 November
// 2026): the smooth cube holds 2.620377 and covers 9.197474; the open 3 × 3 grid covers 8.666667;
// the cube creased on every edge is the cube, 8 and 24. Curva's surface, six levels fine, is
// facets just inside the limit surface, so a little smaller (measured: 0.025% less volume).

#[test]
fn a_smooth_cube_matches_rhinos_limit_surface() {
    let surface = SubD::new(cube2()).surface(6).unwrap();
    let (volume, area) = (surface.volume().unwrap(), surface.area());
    assert!((volume / 2.620_377 - 1.0).abs() < 5e-4, "volume {volume}");
    assert!((area / 9.197_474 - 1.0).abs() < 5e-4, "area {area}");
}

#[test]
fn an_open_grid_rounds_its_corners_as_rhinos_does() {
    let points: Vec<Point> = (0..4u32)
        .flat_map(|j| (0..4u32).map(move |i| [f64::from(i), f64::from(j), 0.0]))
        .collect();
    let at = |i: u32, j: u32| j * 4 + i;
    let faces: Vec<[u32; 4]> = (0..3)
        .flat_map(|j| (0..3).map(move |i| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]))
        .collect();
    let surface = SubD::new(Mesh::new(points, &faces).unwrap())
        .surface(6)
        .unwrap();
    let area = surface.area();
    assert!((area / 8.666_667 - 1.0).abs() < 5e-4, "area {area}");
}

#[test]
fn a_cube_creased_everywhere_matches_rhinos() {
    let subd = SubD::new(cube2());
    let edges: Vec<[u32; 2]> = subd.edge_uses().into_keys().map(|(a, b)| [a, b]).collect();
    let surface = subd.with_creases(&edges).surface(2).unwrap();
    assert!((surface.volume().unwrap() - 8.0).abs() < 1e-5);
    assert!((surface.area() - 24.0).abs() < 1e-5);
}
