//! The orbiting benchmark `docs/performance.md` records: the scenes it draws and its report.

use crate::*;

/// Spins the view for a fixed number of frames and reports how fast the window drew them.
#[derive(Default)]
pub(crate) struct Benchmark {
    pub(crate) times: Vec<f64>,
    pub(crate) work: Vec<f32>,
    pub(crate) snapping: Vec<f64>,
}

/// A square grid of `count` circles.
/// A grid of circles, then a grid of upright cylinders beside it.
pub(crate) fn benchmark_scene(doc: &mut Document, scene: BenchmarkScene) -> Result<(), String> {
    let place = |index: usize, count: usize, shift: f64| {
        let side = (count as f64).sqrt().ceil().max(1.0) as usize;
        let (column, row) = ((index % side) as f64, (index / side) as f64);
        [
            column * BENCHMARK_SPACING + shift,
            row * BENCHMARK_SPACING,
            0.0,
        ]
    };
    for index in 0..scene.curves {
        let centre = place(index, scene.curves, 0.0);
        doc.add(Curve::circle(centre, BENCHMARK_RADIUS).map_err(|error| error.to_string())?);
    }
    let beside = (scene.curves as f64).sqrt().ceil() * BENCHMARK_SPACING + BENCHMARK_SPACING;
    for index in 0..scene.solids {
        let base = Curve::circle(place(index, scene.solids, beside), BENCHMARK_RADIUS)
            .map_err(|error| error.to_string())?;
        let up = curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], BENCHMARK_SPACING, true)
            .map_err(|error| error.to_string())?;
        doc.add_body(up);
    }
    Ok(())
}

/// What a benchmark scene holds.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BenchmarkScene {
    pub(crate) curves: usize,
    pub(crate) solids: usize,
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
        println!("snapping per frame: {snapping:.2} ms");
        let work = self.work.iter().sum::<f32>() * 1000.0 / self.work.len().max(1) as f32;
        println!("frame gap: median {middle:.1} ms, 99th percentile {worst:.1} ms");
        println!("work per frame: {work:.2} ms");
    }
}

/// Spacing and size of the circles in a benchmark scene, in millimetres.
pub(crate) const BENCHMARK_SPACING: f64 = 12.0;

pub(crate) const BENCHMARK_RADIUS: f64 = 5.0;

pub(crate) const BENCHMARK_FRAMES: usize = 240;

/// Leading frames left out of the report, as they include the window opening.
pub(crate) const SETTLE_FRAMES: usize = 30;

/// How far, in points, the benchmark drags the view each frame.
pub(crate) const BENCHMARK_DRAG: f64 = 4.0;
