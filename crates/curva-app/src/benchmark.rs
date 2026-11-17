//! The orbiting benchmark `docs/performance.md` records: the scenes it draws and its report.

use crate::*;

/// Spins the view for a fixed number of frames and reports how fast the window drew them.
#[derive(Default)]
pub(crate) struct Benchmark {
    pub(crate) times: Vec<f64>,
    pub(crate) work: Vec<f32>,
    pub(crate) snapping: Vec<f64>,
    /// Whether the graphics card has a timer, so its own time per frame is reported.
    pub(crate) gpu_timer: bool,
    /// Whether to select and deselect an object every [`SELECT_EVERY`] frames (`--select`).
    pub(crate) select: bool,
    /// The frames at which the selection changed.
    pub(crate) toggles: Vec<usize>,
}

/// How often `--select` changes the selection, in frames.
pub(crate) const SELECT_EVERY: usize = 10;

impl Benchmark {
    /// With `--select`, once the window has settled: selects the first object that can be
    /// selected, or clears the selection, every [`SELECT_EVERY`] frames, as clicking would.
    pub(crate) fn select_step(&mut self, doc: &mut Document) {
        let frame = self.times.len();
        if !self.select || frame < SETTLE_FRAMES || !frame.is_multiple_of(SELECT_EVERY) {
            return;
        }
        let Some(first) = doc.selectable().first().copied() else {
            return;
        };
        if doc.selection().is_empty() {
            doc.select(&[first]);
        } else {
            doc.select(&[]);
        }
        self.toggles.push(frame);
    }

    /// The slowest gap between frames in the few frames after each selection change, as the
    /// change is drawn in the next frame or two; the median over all changes, in milliseconds.
    fn selection_gap_ms(&self) -> Option<f64> {
        let mut slowest: Vec<f64> = self
            .toggles
            .iter()
            .filter_map(|&at| {
                let after = self.times.get(at..(at + 4).min(self.times.len()))?;
                after
                    .windows(2)
                    .map(|pair| (pair[1] - pair[0]) * 1000.0)
                    .max_by(f64::total_cmp)
            })
            .collect();
        slowest.sort_by(f64::total_cmp);
        slowest.get(slowest.len() / 2).copied()
    }
}

/// Builds a benchmark scene: square grids side by side along X, in this order: circles, spline
/// curves, solids, then block instances of a tree, all moved by `scene.offset`.
/// Returns a line saying what the scene holds, for the report.
pub(crate) fn benchmark_scene(doc: &mut Document, scene: BenchmarkScene) -> Result<String, String> {
    let started = std::time::Instant::now();
    let text = |error: curva_kernel::KernelError| error.to_string();
    let at = scene.offset;
    // Where the next grid starts along X, and the grid's side, for `count` items `spacing` apart.
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
    let circles = grid(scene.curves, BENCHMARK_SPACING);
    for index in 0..scene.curves {
        doc.add(Curve::circle(circles(index), BENCHMARK_RADIUS).map_err(text)?);
    }
    let splines = grid(scene.splines, BENCHMARK_SPACING);
    for index in 0..scene.splines {
        let [x, y, z] = splines(index);
        // A wave through five points, so it is cut by the spline rule, not the circle rule.
        let points = (0..5)
            .map(|k: u8| {
                let t = f64::from(k) / 4.0;
                let wave = if k.is_multiple_of(2) {
                    0.0
                } else {
                    BENCHMARK_RADIUS
                };
                [
                    x - BENCHMARK_RADIUS + 2.0 * BENCHMARK_RADIUS * t,
                    y + wave,
                    z,
                ]
            })
            .collect();
        doc.add(Curve::interpolated(points, false).map_err(text)?);
    }
    let solids = grid(scene.solids, BENCHMARK_SPACING);
    // With `shapes` set, solids are copies of that many different shapes, moved: copies share
    // their mesh, as copies and arrays made in Curva do. Without it, every solid is its own.
    let mut shapes = Vec::new();
    for index in 0..scene.shapes {
        shapes.push(benchmark_shape(index)?);
    }
    for index in 0..scene.solids {
        let place = solids(index);
        let body = if shapes.is_empty() {
            let base = Curve::circle(place, BENCHMARK_RADIUS).map_err(text)?;
            curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], BENCHMARK_SPACING, true)
                .map_err(text)?
        } else {
            shapes[index % shapes.len()].translated(place)
        };
        doc.add_body(body);
    }
    if scene.house {
        benchmark_house(doc)?;
    }
    if scene.building {
        benchmark_building(doc)?;
    }
    let mut tree_triangles = 0;
    if scene.trees > 0 {
        let trees = grid(scene.trees, BENCHMARK_TREE_SPACING);
        let tree = benchmark_tree()?;
        for body in &tree {
            tree_triangles += body.mesh().map_err(text)?.triangles.len();
        }
        let ids = tree
            .into_iter()
            .map(|body| doc.add_body(body))
            .collect::<Vec<_>>();
        let (definition, first) = doc.define_block("tree".to_owned(), [0.0; 3], &ids);
        doc.remove(&[first]);
        for index in 0..scene.trees {
            doc.insert_block(definition, curva_document::blocks::moved_by(trees(index)));
        }
    }
    Ok(format!(
        "scene: {} circles, {} splines, {} solids ({}), {} trees of {tree_triangles} triangles, \
         moved by {:?} mm, built in {:.1} s",
        scene.curves,
        scene.splines,
        scene.solids,
        match scene.shapes {
            0 => "each its own".to_owned(),
            shapes => format!("copies of {shapes} shapes"),
        },
        scene.trees,
        scene.offset,
        started.elapsed().as_secs_f64(),
    ))
}

/// A small house for cut drawings (spec `views-and-styles.md` §5), in millimetres: a ground
/// floor of 12 by 8 m with 300 mm outer walls, a door and windows (the window walls stop at sill
/// and head height, so a plan cut at 1.2 m shows the opening), two inner walls, a table, a stair,
/// the upper floor's slab, and trees around.
fn benchmark_house(doc: &mut Document) -> Result<(), String> {
    let text = |error: curva_kernel::KernelError| error.to_string();
    let mut add = |low: Point, size: Point| -> Result<(), String> {
        doc.add_body(curva_kernel::Body::cuboid(low, size).map_err(text)?);
        Ok(())
    };
    let (height, sill, head) = (3000.0, 900.0, 2100.0);
    // A run of wall along X or Y, with windows: full-height pieces between, sill and head
    // pieces across each window.
    let mut wall = |start: Point,
                    along_x: bool,
                    length: f64,
                    thick: f64,
                    windows: &[(f64, f64)]|
     -> Result<(), String> {
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
            let (l, s) = piece(at, from, 0.0, height);
            add(l, s)?;
            let (l, s) = piece(from, to, 0.0, sill);
            add(l, s)?;
            let (l, s) = piece(from, to, head, height);
            add(l, s)?;
            at = to;
        }
        let (l, s) = piece(at, length, 0.0, height);
        add(l, s)
    };
    // Outer walls: south with the door (a gap from 2.5 to 3.5 m) and a window, north, west, east.
    wall([0.0, 0.0, 0.0], true, 2500.0, 300.0, &[])?;
    wall([3500.0, 0.0, 0.0], true, 8500.0, 300.0, &[(2500.0, 5500.0)])?;
    wall(
        [0.0, 7700.0, 0.0],
        true,
        12000.0,
        300.0,
        &[(4000.0, 8000.0)],
    )?;
    wall([0.0, 300.0, 0.0], false, 7400.0, 300.0, &[(2200.0, 4700.0)])?;
    wall([11700.0, 300.0, 0.0], false, 7400.0, 300.0, &[])?;
    // Inner walls, 150 mm.
    wall([7000.0, 300.0, 0.0], false, 4200.0, 150.0, &[])?;
    wall([300.0, 4500.0, 0.0], true, 3700.0, 150.0, &[])?;
    // Floor and the upper floor's slab.
    add([0.0, 0.0, -300.0], [12000.0, 8000.0, 300.0])?;
    add([0.0, 0.0, height], [12000.0, 8000.0, 300.0])?;
    // A table and a stair of six steps.
    add([1800.0, 1600.0, 0.0], [1800.0, 900.0, 750.0])?;
    for step in 0..6 {
        let rise = f64::from(step + 1) * 170.0;
        add(
            [8200.0 + f64::from(step) * 280.0, 5600.0, 0.0],
            [280.0, 1100.0, rise],
        )?;
    }
    // Ground around, lower than the floor.
    add([-3000.0, -3000.0, -700.0], [18000.0, 14000.0, 400.0])?;
    let trunk = curva_kernel::Body::extruded(
        &[Curve::circle([0.0; 3], 180.0).map_err(text)?],
        [0.0, 0.0, 1.0],
        3000.0,
        true,
    )
    .map_err(text)?;
    let crown = curva_kernel::Body::sphere([0.0, 0.0, 4300.0], 1800.0).map_err(text)?;
    let ids = [doc.add_body(trunk), doc.add_body(crown)];
    let (tree, first) = doc.define_block("house tree".to_owned(), [0.0; 3], &ids);
    doc.remove(&[first]);
    for at in [
        [-2500.0, -2500.0, -300.0],
        [14500.0, -2000.0, -300.0],
        [14000.0, 10500.0, -300.0],
    ] {
        doc.insert_block(tree, curva_document::blocks::moved_by(at));
    }
    Ok(())
}
/// A small building for pictures of visualisation styles (spec `views-and-styles.md` §5), in
/// millimetres: a plinth, a ground-floor volume, an upper volume cantilevered over the entrance,
/// a roof box, slender columns under the cantilever, and trees as block instances.
fn benchmark_building(doc: &mut Document) -> Result<(), String> {
    let text = |error: curva_kernel::KernelError| error.to_string();
    let block = |low: Point, size: Point| curva_kernel::Body::cuboid(low, size).map_err(text);
    let parts = [
        // Plinth.
        ([-3000.0, -6000.0, -400.0], [20000.0, 18000.0, 400.0]),
        // Ground floor.
        ([0.0, 0.0, 0.0], [12000.0, 8000.0, 3200.0]),
        // Upper floor, reaching out over the entrance.
        ([4000.0, -3500.0, 3200.0], [9000.0, 9500.0, 3200.0]),
        // Roof box.
        ([6000.0, 0.0, 6400.0], [4500.0, 4500.0, 2200.0]),
    ];
    for (low, size) in parts {
        doc.add_body(block(low, size)?);
    }
    for x in [5000.0, 8500.0, 12000.0] {
        doc.add_body(block([x - 150.0, -3350.0, 0.0], [300.0, 300.0, 3200.0])?);
    }
    let trunk = curva_kernel::Body::extruded(
        &[Curve::circle([0.0; 3], 180.0).map_err(text)?],
        [0.0, 0.0, 1.0],
        3000.0,
        true,
    )
    .map_err(text)?;
    let crown = curva_kernel::Body::sphere([0.0, 0.0, 4300.0], 1800.0).map_err(text)?;
    let ids = [doc.add_body(trunk), doc.add_body(crown)];
    let (tree, first) = doc.define_block("building tree".to_owned(), [0.0; 3], &ids);
    doc.remove(&[first]);
    for at in [
        [-1500.0, -4500.0, 0.0],
        [15500.0, -4000.0, 0.0],
        [15000.0, 9500.0, 0.0],
        [-1800.0, 10000.0, 0.0],
        [2500.0, -5200.0, 0.0],
    ] {
        doc.insert_block(tree, curva_document::blocks::moved_by(at));
    }
    Ok(())
}
/// One of the different solids of the Large scene, at the origin: cylinders and boxes of several
/// sizes, each fitting within a grid cell.
fn benchmark_shape(index: usize) -> Result<curva_kernel::Body, String> {
    let text = |error: curva_kernel::KernelError| error.to_string();
    let step = (index % 10) as f64 / 10.0;
    let height = BENCHMARK_SPACING * (0.5 + 2.0 * step);
    if index.is_multiple_of(2) {
        let radius = BENCHMARK_RADIUS * (0.4 + 0.6 * step);
        let base = Curve::circle([0.0; 3], radius).map_err(text)?;
        curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], height, true).map_err(text)
    } else {
        let side = 2.0 * BENCHMARK_RADIUS * (0.4 + 0.6 * step);
        curva_kernel::Body::cuboid([-side / 2.0, -side / 2.0, 0.0], [side, side, height])
            .map_err(text)
    }
}

/// The solids of the tree block, standing at the origin: a trunk and a crown of spheres.
fn benchmark_tree() -> Result<Vec<curva_kernel::Body>, String> {
    let text = |error: curva_kernel::KernelError| error.to_string();
    let base = Curve::circle([0.0; 3], 1.5).map_err(text)?;
    let mut bodies =
        vec![curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], 30.0, true).map_err(text)?];
    for [x, y, z, radius] in BENCHMARK_CROWN {
        bodies.push(curva_kernel::Body::sphere([x, y, z], radius).map_err(text)?);
    }
    Ok(bodies)
}

/// What a benchmark scene holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BenchmarkScene {
    /// Circles.
    pub(crate) curves: usize,
    /// Spline curves through five points.
    pub(crate) splines: usize,
    /// Solids.
    pub(crate) solids: usize,
    /// How many different shapes the solids are copies of; 0: each solid its own.
    pub(crate) shapes: usize,
    /// Block instances of one tree.
    pub(crate) trees: usize,
    /// Where the scene is moved to, for drawing far from the origin.
    pub(crate) offset: Point,
    /// Whether the benchmark also selects and deselects an object now and then (`--select`).
    pub(crate) select: bool,
    /// A small two-storey building with trees, for pictures of visualisation styles.
    pub(crate) building: bool,
    /// A small house with walls, openings, a table and a stair, for cut drawings.
    pub(crate) house: bool,
}

impl BenchmarkScene {
    /// The standard scenes of spec `display-pipeline.md` §6, by name.
    pub(crate) fn named(name: &str) -> Option<Self> {
        let large = Self {
            curves: 40_000,
            splines: 10_000,
            solids: 10_000,
            shapes: 100,
            trees: 2_000,
            offset: [0.0; 3],
            select: false,
            building: false,
            house: false,
        };
        match name {
            "medium" => Some(Self {
                curves: 5_000,
                solids: 500,
                ..Self::default()
            }),
            "large" => Some(large),
            // 500 km from the origin along X and Y, in millimetres.
            "house" => Some(Self {
                house: true,
                ..Self::default()
            }),
            "building" => Some(Self {
                building: true,
                ..Self::default()
            }),
            "far" => Some(Self {
                offset: [5.0e8, 5.0e8, 0.0],
                ..large
            }),
            _ => None,
        }
    }
}

pub(crate) fn curve_kind(curve: &Curve) -> &'static str {
    match curve {
        Curve::Point { .. } => "ui.kind.point",
        Curve::Line { .. } => "ui.kind.line",
        Curve::Polyline { .. } => "ui.kind.polyline",
        Curve::Circle { .. } => "ui.kind.circle",
        Curve::Arc { .. } => "ui.kind.arc",
        Curve::Interpolated { .. } | Curve::Nurbs { .. } => "ui.kind.smooth",
        Curve::Compound { .. } => "ui.kind.joined",
    }
}

pub(crate) fn snap_label(kind: SnapKind) -> &'static str {
    match kind {
        SnapKind::End => "ui.snap.end",
        SnapKind::Mid => "ui.snap.mid",
        SnapKind::Center => "ui.snap.center",
        SnapKind::Intersection => "ui.snap.intersection",
        SnapKind::Perpendicular => "ui.snap.perpendicular",
        SnapKind::Near => "ui.snap.near",
    }
}

impl Benchmark {
    /// Notes one frame. Returns whether enough have been seen.
    pub(crate) fn record(&mut self, ui: &egui::Ui, frame: &eframe::Frame) -> bool {
        self.times.push(ui.input(|input| input.time));
        // The card's timer starts with the first frame and starts again once the window has
        // settled, like the frame gaps.
        if self.times.len() == 1 || self.times.len() == SETTLE_FRAMES {
            self.gpu_timer = time_gpu(frame, true);
        }
        self.work.extend(frame.info().cpu_usage);
        ui.ctx().request_repaint();
        self.times.len() >= BENCHMARK_FRAMES
    }

    pub(crate) fn report(&self, frame: &eframe::Frame) {
        if let Some(state) = frame.wgpu_render_state() {
            let info = state.adapter.get_info();
            println!(
                "adapter: {} ({:?}, {:?})",
                info.name, info.backend, info.device_type
            );
        }
        let mut gaps: Vec<f64> = self.times[SETTLE_FRAMES..]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) * 1000.0)
            .collect();
        gaps.sort_by(f64::total_cmp);
        let middle = gaps[gaps.len() / 2];
        let worst = gaps[gaps.len() * 99 / 100];
        let snapping = self.snapping.iter().sum::<f64>() / self.snapping.len().max(1) as f64;
        // egui counts time from the window opening, so this is how long the scene took to show.
        println!(
            "first frame: {:.1} s after the window opened",
            self.times[0]
        );
        println!("snapping per frame: {snapping:.2} ms");
        // Like the frame gaps, leaving out the opening frames, whose first upload of a large
        // scene would otherwise swamp the average.
        let settled = self.work.get(SETTLE_FRAMES..).unwrap_or_default();
        let work = settled.iter().sum::<f32>() * 1000.0 / settled.len().max(1) as f32;
        println!("frame gap: median {middle:.1} ms, 99th percentile {worst:.1} ms");
        println!("work per frame: {work:.2} ms");
        match gpu_frame_ms(frame) {
            Some(ms) if self.gpu_timer => {
                println!(
                    "graphics card per frame, Curva's lines only (Bevy's time comes in step 5): {ms:.2} ms"
                );
            }
            _ => println!("graphics card per frame: not measured (the card has no timer)"),
        }
        if let Some(gap) = self.selection_gap_ms() {
            println!(
                "after a selection change: median slowest frame gap {gap:.1} ms ({} changes)",
                self.toggles.len()
            );
        }
    }
}

/// Switches the viewport's timer on the graphics card on or off. Returns whether the card has one.
fn time_gpu(frame: &eframe::Frame, on: bool) -> bool {
    let Some(state) = frame.wgpu_render_state() else {
        return false;
    };
    let mut renderer = state.renderer.write();
    renderer
        .callback_resources
        .get_mut::<LineRenderer>()
        .is_some_and(|lines| lines.time_gpu(&state.device, &state.queue, on))
}

/// The graphics card's average time per frame for drawing the viewports, if measured.
fn gpu_frame_ms(frame: &eframe::Frame) -> Option<f64> {
    let state = frame.wgpu_render_state()?;
    let renderer = state.renderer.read();
    renderer
        .callback_resources
        .get::<LineRenderer>()?
        .gpu_frame_ms()
}

/// Spacing and size of the circles in a benchmark scene, in millimetres.
pub(crate) const BENCHMARK_SPACING: f64 = 12.0;

pub(crate) const BENCHMARK_RADIUS: f64 = 5.0;

pub(crate) const BENCHMARK_FRAMES: usize = 240;

/// Leading frames left out of the report, as they include the window opening.
pub(crate) const SETTLE_FRAMES: usize = 30;

/// How far, in points, the benchmark drags the view each frame.
pub(crate) const BENCHMARK_DRAG: f64 = 4.0;

/// Spacing of the trees in a benchmark scene, in millimetres.
pub(crate) const BENCHMARK_TREE_SPACING: f64 = 40.0;

/// The tree's crown: spheres as centre and radius, in millimetres. One sphere and the trunk mesh to
/// about 5,000 triangles, as spec `display-pipeline.md` §6 asks (the report prints the count).
pub(crate) const BENCHMARK_CROWN: [[f64; 4]; 1] = [[0.0, 0.0, 40.0, 12.0]];

#[cfg(test)]
mod tests {
    use super::*;

    /// A small scene with every kind, so building it stays quick.
    fn small(offset: Point) -> BenchmarkScene {
        BenchmarkScene {
            curves: 4,
            splines: 3,
            solids: 5,
            shapes: 2,
            trees: 3,
            offset,
            select: false,
            building: false,
            house: false,
        }
    }

    #[test]
    fn the_standard_scenes_hold_what_the_spec_lists() {
        let medium = BenchmarkScene::named("medium").unwrap();
        assert_eq!(
            (medium.curves, medium.solids, medium.trees),
            (5_000, 500, 0)
        );
        let large = BenchmarkScene::named("large").unwrap();
        assert_eq!(large.curves + large.splines, 50_000);
        assert_eq!((large.solids, large.trees), (10_000, 2_000));
        let far = BenchmarkScene::named("far").unwrap();
        assert_eq!(far.offset, [5.0e8, 5.0e8, 0.0]);
        assert_eq!(
            BenchmarkScene {
                offset: [0.0; 3],
                ..far
            },
            large
        );
        assert!(BenchmarkScene::named("huge").is_none());
    }

    #[test]
    fn a_scene_adds_every_object_and_one_tree_definition() {
        let mut doc = Document::default();
        let summary = benchmark_scene(&mut doc, small([0.0; 3])).unwrap();
        // 4 circles, 3 splines, 5 solids and 3 tree instances.
        assert_eq!(doc.state().objects.len(), 15);
        assert_eq!(doc.definitions().len(), 1);
        assert_eq!(doc.instances_of(doc.definitions()[0].id).len(), 3);
        assert!(summary.starts_with("scene: 4 circles, 3 splines, 5 solids"));
    }

    #[test]
    fn a_far_scene_is_the_same_scene_moved() {
        let bounds = |offset: Point| {
            let mut doc = Document::default();
            // Without trees, which take long to mesh in a test build.
            let scene = BenchmarkScene {
                trees: 0,
                ..small(offset)
            };
            benchmark_scene(&mut doc, scene).unwrap();
            Outlines::of(doc.state()).bounds().unwrap()
        };
        let (near_low, near_high) = bounds([0.0; 3]);
        let (far_low, far_high) = bounds([5.0e8, 5.0e8, 0.0]);
        for axis in 0..3 {
            let shift = if axis < 2 { 5.0e8 } else { 0.0 };
            assert!((far_low[axis] - near_low[axis] - shift).abs() < 1e-3);
            assert!((far_high[axis] - near_high[axis] - shift).abs() < 1e-3);
        }
    }

    #[test]
    fn select_changes_the_selection_every_few_frames_once_settled() {
        let mut doc = Document::default();
        let scene = BenchmarkScene {
            curves: 2,
            ..BenchmarkScene::default()
        };
        benchmark_scene(&mut doc, scene).unwrap();
        let mut benchmark = Benchmark {
            select: true,
            ..Benchmark::default()
        };
        let mut selected = Vec::new();
        for frame in 0..SETTLE_FRAMES + 3 * SELECT_EVERY {
            // A frame every 5 ms, and 30 ms just after each change, as a slow redraw would be.
            let gap = if benchmark.toggles.last().is_some_and(|&at| at + 1 == frame) {
                0.030
            } else {
                0.005
            };
            let last = benchmark.times.last().copied().unwrap_or(0.0);
            benchmark.times.push(last + gap);
            benchmark.select_step(&mut doc);
            selected.push(doc.selection().len());
        }
        assert_eq!(
            benchmark.toggles,
            [
                SETTLE_FRAMES,
                SETTLE_FRAMES + 10,
                SETTLE_FRAMES + 20,
                SETTLE_FRAMES + 30
            ]
        );
        // Nothing selected while settling, then on, off, on, off.
        assert!(selected[..SETTLE_FRAMES - 1].iter().all(|&n| n == 0));
        assert_eq!(selected[SETTLE_FRAMES - 1], 1);
        assert_eq!(selected.last(), Some(&0));
        let gap = benchmark.selection_gap_ms().unwrap();
        assert!((gap - 30.0).abs() < 1e-6, "{gap}");
        // Without `--select`, nothing changes.
        let mut idle = Benchmark {
            times: vec![0.0; SETTLE_FRAMES],
            ..Benchmark::default()
        };
        idle.select_step(&mut doc);
        assert!(idle.toggles.is_empty());
    }

    /// The test house drawn as a plan: Make2D from the top with a clipping plane at 1.2 m keeping
    /// what is below (spec `live-plans.md` §6). Prints how long it took and what it made. Run it
    /// in an optimised build: `cargo test --release -p curva-app plan_build -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore = "a timing, run by hand in an optimised build"]
    fn plan_build_of_the_test_house() {
        use curva_document::clipping::{ClippingPlane, SectionFill};
        let mut doc = Document::default();
        // The test house, or another standard scene named in `PLAN_SCENE` (`large`).
        let scene = std::env::var("PLAN_SCENE").unwrap_or_else(|_| "house".to_owned());
        benchmark_scene(&mut doc, BenchmarkScene::named(&scene).unwrap()).unwrap();
        let cut = f64::from(crate::agent::PLAN_CUT_HEIGHT);
        let frame = doc.add(
            Curve::polyline(
                vec![
                    [-20_000.0, -20_000.0, cut],
                    [40_000.0, -20_000.0, cut],
                    [40_000.0, 40_000.0, cut],
                    [-20_000.0, 40_000.0, cut],
                ],
                true,
            )
            .unwrap(),
        );
        doc.add_clipping_plane(ClippingPlane {
            frame,
            flipped: false,
            views: Vec::new(),
            section: SectionFill::default(),
            hatch: None,
        });
        if doc.plane_cut_of(frame).is_some_and(|c| c.keep[2] > 0.0) {
            doc.change_clipping_plane(frame, |c| c.flipped = true);
        }
        doc.set_view(curva_kernel::hidden::View {
            eye: [6000.0, 4000.0, 1.0e6],
            forward: [0.0, 0.0, -1.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            parallel: true,
        });
        let all: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&all);
        let before = doc.state().objects.len();
        let started = std::time::Instant::now();
        let mut processor = curva_commands::Processor::default();
        processor.submit(&mut doc, "Make2D");
        processor.submit(&mut doc, "");
        let took = started.elapsed();
        let made = doc.state().objects.len() - before;
        println!("plan of {scene}: {made} objects in {took:.3?}");
        if let Ok(path) = std::env::var("DUMP") {
            let drawn: Vec<String> = doc.state().objects[before..]
                .iter()
                .map(|o| format!("{} {:?}", doc.state().layer_path(o.layer), o.shape))
                .collect();
            std::fs::write(
                path,
                drawn.join(
                    "
",
                ),
            )
            .unwrap();
        }
        assert!(made > 0);
    }
}
