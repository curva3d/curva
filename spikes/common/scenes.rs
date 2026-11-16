//! The benchmark scenes of spec `display-pipeline.md` §6 (Medium, Large, Far) and the picture
//! scenes (building, house), built as in `crates/curva-app/src/benchmark.rs`, for the renderer
//! spikes (#186, #198, #199). Each spike includes this file, so all draw the same scenes.
//!
//! A scene is kept as shapes (each meshed once, at its own origin) and placements of them, so a
//! renderer can draw copies as instances; and the curves, as the points Curva draws them along.

#![allow(dead_code)]

use curva_kernel::{Body, BodyMesh, Curve};

pub type Point = [f64; 3];

const SPACING: f64 = 12.0;
const RADIUS: f64 = 5.0;
const TREE_SPACING: f64 = 40.0;

/// A scene ready to draw.
pub struct Scene {
    /// Each different solid's mesh, at its own origin.
    pub shapes: Vec<BodyMesh>,
    /// Solids: which shape, moved by how much.
    pub placed: Vec<(usize, Point)>,
    /// How many placements are tree parts (block instances in Curva).
    pub tree_parts: usize,
    /// Curves, as the points Curva draws them along.
    pub curves: Vec<Vec<Point>>,
}

impl Scene {
    /// The box around everything.
    pub fn bounds(&self) -> (Point, Point) {
        let mut low = [f64::MAX; 3];
        let mut high = [f64::MIN; 3];
        let mut grow = |p: Point| {
            for a in 0..3 {
                low[a] = low[a].min(p[a]);
                high[a] = high[a].max(p[a]);
            }
        };
        for (shape, at) in &self.placed {
            for p in &self.shapes[*shape].positions {
                grow(add(*p, *at));
            }
        }
        for c in &self.curves {
            for p in c {
                grow(*p);
            }
        }
        (low, high)
    }

    /// The solids' edges as Curva draws them in shaded views (seams left out), moved into place.
    pub fn edges(&self) -> Vec<Vec<Point>> {
        let mut out = Vec::new();
        for (shape, at) in &self.placed {
            let mesh = &self.shapes[*shape];
            for (edge, seam) in mesh.edges.iter().zip(&mesh.seams) {
                if !seam {
                    out.push(edge.iter().map(|p| add(*p, *at)).collect());
                }
            }
        }
        out
    }

    pub fn triangles(&self) -> usize {
        self.placed
            .iter()
            .map(|(s, _)| self.shapes[*s].triangles.len())
            .sum()
    }
}

pub fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A scene by name: medium, large, far, building, house.
pub fn named(name: &str) -> Scene {
    match name {
        "medium" => grid_scene(5_000, 0, 500, 0, 0, [0.0; 3]),
        "large" => grid_scene(40_000, 10_000, 10_000, 100, 2_000, [0.0; 3]),
        "far" => grid_scene(40_000, 10_000, 10_000, 100, 2_000, [5.0e8, 5.0e8, 0.0]),
        "building" => building(),
        "house" => house(),
        "meshes" => meshes(),
        _ => panic!("unknown scene {name}"),
    }
}

fn curve_points(curves: Vec<Curve>) -> Vec<Vec<Point>> {
    let mut doc = curva_document::Document::default();
    for c in curves {
        doc.add(c);
    }
    curva_viewport::Outlines::of(doc.state())
        .objects
        .into_iter()
        .map(|(_, p)| p)
        .collect()
}

fn mesh(body: &Body) -> BodyMesh {
    body.mesh().unwrap()
}

/// The grid scenes, as `benchmark_scene` lays them out.
fn grid_scene(
    curves: usize,
    splines: usize,
    solids: usize,
    shapes: usize,
    trees: usize,
    at: Point,
) -> Scene {
    let mut shift = 0.0;
    let mut grid = |count: usize, spacing: f64| {
        let side = (count as f64).sqrt().ceil().max(1.0) as usize;
        let start = shift;
        if count > 0 {
            shift += (side as f64 + 1.0) * spacing;
        }
        move |index: usize| {
            let (column, row) = ((index % side) as f64, (index / side) as f64);
            [
                at[0] + start + column * spacing,
                at[1] + row * spacing,
                at[2],
            ]
        }
    };
    let mut all = Vec::new();
    let circles = grid(curves, SPACING);
    for i in 0..curves {
        all.push(Curve::circle(circles(i), RADIUS).unwrap());
    }
    let wave = grid(splines, SPACING);
    for i in 0..splines {
        let [x, y, z] = wave(i);
        let points = (0..5u8)
            .map(|k| {
                let t = f64::from(k) / 4.0;
                let w = if k.is_multiple_of(2) { 0.0 } else { RADIUS };
                [x - RADIUS + 2.0 * RADIUS * t, y + w, z]
            })
            .collect();
        all.push(Curve::interpolated(points, false).unwrap());
    }
    let mut scene = Scene {
        shapes: Vec::new(),
        placed: Vec::new(),
        tree_parts: 0,
        curves: curve_points(all),
    };
    let place = grid(solids, SPACING);
    if shapes == 0 {
        // Each solid its own (Medium): one cylinder, meshed once, kept as separate shapes so a
        // renderer treats them as separate meshes, as Curva does.
        let base = Curve::circle([0.0; 3], RADIUS).unwrap();
        let cylinder = mesh(&Body::extruded(&[base], [0.0, 0.0, 1.0], SPACING, true).unwrap());
        for i in 0..solids {
            scene.shapes.push(cylinder.clone());
            scene.placed.push((i, place(i)));
        }
    } else {
        for i in 0..shapes {
            scene.shapes.push(mesh(&shape(i)));
        }
        for i in 0..solids {
            scene.placed.push((i % shapes, place(i)));
        }
    }
    if trees > 0 {
        let spots = grid(trees, TREE_SPACING);
        let first = scene.shapes.len();
        let base = Curve::circle([0.0; 3], 1.5).unwrap();
        scene.shapes.push(mesh(
            &Body::extruded(&[base], [0.0, 0.0, 1.0], 30.0, true).unwrap(),
        ));
        scene
            .shapes
            .push(mesh(&Body::sphere([0.0, 0.0, 40.0], 12.0).unwrap()));
        for i in 0..trees {
            scene.placed.push((first, spots(i)));
            scene.placed.push((first + 1, spots(i)));
        }
        scene.tree_parts = 2 * trees;
    }
    scene
}

fn shape(index: usize) -> Body {
    let step = (index % 10) as f64 / 10.0;
    let height = SPACING * (0.5 + 2.0 * step);
    if index.is_multiple_of(2) {
        let base = Curve::circle([0.0; 3], RADIUS * (0.4 + 0.6 * step)).unwrap();
        Body::extruded(&[base], [0.0, 0.0, 1.0], height, true).unwrap()
    } else {
        let side = 2.0 * RADIUS * (0.4 + 0.6 * step);
        Body::cuboid([-side / 2.0, -side / 2.0, 0.0], [side, side, height]).unwrap()
    }
}

/// Boxes, each its own shape, and the picture scenes' tree (trunk and crown) at `trees`.
fn boxes_and_trees(boxes: &[(Point, Point)], trees: &[Point]) -> Scene {
    let mut scene = Scene {
        shapes: Vec::new(),
        placed: Vec::new(),
        tree_parts: 0,
        curves: Vec::new(),
    };
    for (low, size) in boxes {
        scene.placed.push((scene.shapes.len(), [0.0; 3]));
        scene.shapes.push(mesh(&Body::cuboid(*low, *size).unwrap()));
    }
    let first = scene.shapes.len();
    let trunk = Curve::circle([0.0; 3], 180.0).unwrap();
    scene.shapes.push(mesh(
        &Body::extruded(&[trunk], [0.0, 0.0, 1.0], 3000.0, true).unwrap(),
    ));
    scene
        .shapes
        .push(mesh(&Body::sphere([0.0, 0.0, 4300.0], 1800.0).unwrap()));
    for at in trees {
        scene.placed.push((first, *at));
        scene.placed.push((first + 1, *at));
    }
    scene.tree_parts = 2 * trees.len();
    scene
}

fn building() -> Scene {
    let mut boxes = vec![
        ([-3000.0, -6000.0, -400.0], [20000.0, 18000.0, 400.0]),
        ([0.0, 0.0, 0.0], [12000.0, 8000.0, 3200.0]),
        ([4000.0, -3500.0, 3200.0], [9000.0, 9500.0, 3200.0]),
        ([6000.0, 0.0, 6400.0], [4500.0, 4500.0, 2200.0]),
    ];
    for x in [5000.0, 8500.0, 12000.0] {
        boxes.push(([x - 150.0, -3350.0, 0.0], [300.0, 300.0, 3200.0]));
    }
    boxes_and_trees(
        &boxes,
        &[
            [-1500.0, -4500.0, 0.0],
            [15500.0, -4000.0, 0.0],
            [15000.0, 9500.0, 0.0],
            [-1800.0, 10000.0, 0.0],
            [2500.0, -5200.0, 0.0],
        ],
    )
}

fn house() -> Scene {
    let mut boxes: Vec<(Point, Point)> = Vec::new();
    let (height, sill, head) = (3000.0, 900.0, 2100.0);
    let mut wall =
        |start: Point, along_x: bool, length: f64, thick: f64, windows: &[(f64, f64)]| {
            let piece = |from: f64, to: f64, low: f64, high: f64| {
                if along_x {
                    (
                        [start[0] + from, start[1], low],
                        [to - from, thick, high - low],
                    )
                } else {
                    (
                        [start[0], start[1] + from, low],
                        [thick, to - from, high - low],
                    )
                }
            };
            let mut at = 0.0;
            for &(from, to) in windows {
                boxes.push(piece(at, from, 0.0, height));
                boxes.push(piece(from, to, 0.0, sill));
                boxes.push(piece(from, to, head, height));
                at = to;
            }
            boxes.push(piece(at, length, 0.0, height));
        };
    wall([0.0, 0.0, 0.0], true, 2500.0, 300.0, &[]);
    wall([3500.0, 0.0, 0.0], true, 8500.0, 300.0, &[(2500.0, 5500.0)]);
    wall(
        [0.0, 7700.0, 0.0],
        true,
        12000.0,
        300.0,
        &[(4000.0, 8000.0)],
    );
    wall([0.0, 300.0, 0.0], false, 7400.0, 300.0, &[(2200.0, 4700.0)]);
    wall([11700.0, 300.0, 0.0], false, 7400.0, 300.0, &[]);
    wall([7000.0, 300.0, 0.0], false, 4200.0, 150.0, &[]);
    wall([300.0, 4500.0, 0.0], true, 3700.0, 150.0, &[]);
    boxes.push(([0.0, 0.0, -300.0], [12000.0, 8000.0, 300.0]));
    boxes.push(([0.0, 0.0, height], [12000.0, 8000.0, 300.0]));
    boxes.push(([1800.0, 1600.0, 0.0], [1800.0, 900.0, 750.0]));
    for step in 0..6 {
        let rise = f64::from(step + 1) * 170.0;
        boxes.push((
            [8200.0 + f64::from(step) * 280.0, 5600.0, 0.0],
            [280.0, 1100.0, rise],
        ));
    }
    boxes.push(([-3000.0, -3000.0, -700.0], [18000.0, 14000.0, 400.0]));
    boxes_and_trees(
        &boxes,
        &[
            [-2500.0, -2500.0, -300.0],
            [14500.0, -2000.0, -300.0],
            [14000.0, 10500.0, -300.0],
        ],
    )
}

/// A wavy grid mesh of `n` by `n` quads, `size` wide, starting at `at`: positions, normals and
/// triangles. `phase` makes each one different (and moves it over time for the SubD test).
pub fn wavy_grid(n: usize, size: f64, at: Point, height: f64, phase: f64) -> BodyMesh {
    let step = size / n as f64;
    let k = std::f64::consts::TAU * 3.0 / size;
    let mut m = BodyMesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (i as f64 * step, j as f64 * step);
            let (sx, cx) = (k * x + phase).sin_cos();
            let (sy, cy) = (k * y + 0.7 * phase).sin_cos();
            m.positions.push([at[0] + x, at[1] + y, at[2] + height * sx * sy]);
            // The surface's slopes give its normal.
            let (dx, dy) = (height * k * cx * sy, height * k * sx * cy);
            let l = (dx * dx + dy * dy + 1.0).sqrt();
            m.normals.push([-dx / l, -dy / l, 1.0 / l]);
        }
    }
    let row = (n + 1) as u32;
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * row + i;
            m.triangles.push([a, a + 1, a + row + 1]);
            m.triangles.push([a, a + row + 1, a + row]);
        }
    }
    m.triangle_faces = vec![0; m.triangles.len()];
    let edge = |p: &[Point]| p.to_vec();
    let last = n;
    let at_ij = |i: usize, j: usize| m.positions[j * (n + 1) + i];
    let outline: Vec<Point> = (0..=last)
        .map(|i| at_ij(i, 0))
        .chain((0..=last).map(|j| at_ij(last, j)))
        .chain((0..=last).rev().map(|i| at_ij(i, last)))
        .chain((0..=last).rev().map(|j| at_ij(0, j)))
        .collect();
    m.edges = vec![edge(&outline)];
    m.edge_ids = vec![0];
    m.seams = vec![false];
    m
}

/// Big meshes that are all different, as imported terrain or scans: 16 tiles of 100 m, each a
/// grid of 700 by 700 quads (about 980,000 triangles), 15.7 million triangles in all. Nothing is
/// repeated, so instancing cannot help.
pub fn meshes() -> Scene {
    let mut scene = Scene {
        shapes: Vec::new(),
        placed: Vec::new(),
        tree_parts: 0,
        curves: Vec::new(),
    };
    for t in 0..16 {
        let at = [(t % 4) as f64 * 100_000.0, (t / 4) as f64 * 100_000.0, 0.0];
        scene.placed.push((t, [0.0; 3]));
        scene.shapes.push(wavy_grid(700, 100_000.0, at, 5_000.0, t as f64));
    }
    scene
}

/// A SubD while it is edited, at time `t`: the limit surface (the cage of `cage` by `cage` faces
/// subdivided three times, so 8 by 8 quads per cage face) and the cage's lines, 300 mm above it.
/// Dragging a control point changes both every frame.
pub fn subd(cage: usize, t: f64) -> (BodyMesh, Vec<Vec<Point>>) {
    let size = 10_000.0;
    let surface = wavy_grid(cage * 8, size, [0.0; 3], 800.0, t);
    let coarse = wavy_grid(cage, size, [0.0, 0.0, 300.0], 800.0, t);
    let p = |i: usize, j: usize| coarse.positions[j * (cage + 1) + i];
    let mut lines = Vec::new();
    for j in 0..=cage {
        lines.push((0..=cage).map(|i| p(i, j)).collect());
    }
    for i in 0..=cage {
        lines.push((0..=cage).map(|j| p(i, j)).collect());
    }
    (surface, lines)
}
