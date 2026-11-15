//! Spike for #185 (decision 0024): a plan of the test house, cut at 1.2 m, built as Curva's own
//! vector page (`curva_document::drawing::Page`) from geometry Curva already computes (solids
//! trimmed at the cut, their cut outlines, Make2D's hidden lines), then drawn four ways:
//!
//! 1. written to PDF by `curva-pdf`, then rasterised with hayro (the reference: what prints);
//! 2. Vello on the graphics card, with the device Curva asks for (`device_descriptor`);
//! 3. the same on software graphics (WARP on Windows), as a stand-in for a weak card;
//! 4. vello_cpu on the processor alone.
//!
//! Timings: building the page once; drawing it; redrawing while panning and zooming (each frame
//! rebuilds Vello's scene with a new transform, as a live view would).
//!
//! Run: `cargo run --release -- <output folder>`.

use curva_document::drawing::{Mark, Page};
use curva_kernel::hidden::{LineKind, Options, View, hidden_lines};
use curva_kernel::{Body, Combine, Curve, Point};
use std::time::Instant;
use vello::kurbo::{Affine, BezPath, Cap, Join, Stroke};
use vello::peniko::{Color, Fill};
use vello::wgpu;

/// Plan cut height and drawing scale.
const CUT: f64 = 1200.0;
const SCALE: f64 = 50.0;
/// A3 landscape, millimetres.
const PAGE: [f64; 2] = [420.0, 297.0];
/// Pixels per paper millimetre for the pictures (about 150 dpi).
const PX_PER_MM: f64 = 6.0;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();

    let started = Instant::now();
    let page = plan_page();
    let build = started.elapsed();
    let lines = page
        .marks
        .iter()
        .filter(|m| matches!(m, Mark::Line { .. }))
        .count();
    let fills = page.marks.len() - lines;
    println!(
        "page: {lines} lines, {fills} fills, built in {:.0} ms",
        build.as_secs_f64() * 1000.0
    );

    let size = [(PAGE[0] * PX_PER_MM) as u32, (PAGE[1] * PX_PER_MM) as u32];

    // 1. The PDF, and hayro's picture of it.
    let pdf = curva_pdf::write_pdf(std::slice::from_ref(&page)).unwrap();
    std::fs::write(out.join("plan.pdf"), &pdf).unwrap();
    let started = Instant::now();
    let pdf_pixels = rasterise_pdf(&pdf, size);
    println!(
        "pdf via hayro: {:.0} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    save_png(&out.join("plan-pdf.png"), size, &pdf_pixels);

    // 2 and 3. Vello on the graphics card, then on software graphics.
    for (label, fallback) in [("vello-gpu", false), ("vello-warp", true)] {
        match vello_gpu(&page, size, fallback) {
            Ok((pixels, report)) => {
                println!("{label}: {report}");
                save_png(&out.join(format!("plan-{label}.png")), size, &pixels);
                println!("  differs from pdf: {}", difference(&pixels, &pdf_pixels));
            }
            Err(error) => println!("{label}: failed: {error}"),
        }
    }

    // 3b. Vello Hybrid, on a device of its own (it uses wgpu 29).
    for (label, fallback) in [("vello-hybrid", false), ("vello-hybrid-warp", true)] {
        match vello_hybrid(&page, size, fallback) {
            Ok((pixels, report)) => {
                println!("{label}: {report}");
                save_png(&out.join(format!("plan-{label}.png")), size, &pixels);
                println!("  differs from pdf: {}", difference(&pixels, &pdf_pixels));
            }
            Err(error) => println!("{label}: failed: {error}"),
        }
    }

    // 4. vello_cpu.
    let (pixels, report) = vello_cpu(&page, size);
    println!("vello-cpu: {report}");
    save_png(&out.join("plan-vello-cpu.png"), size, &pixels);
    println!("  differs from pdf: {}", difference(&pixels, &pdf_pixels));
}

/// The test house of `--scene house`, as plain solids (trees too, without blocks).
fn house() -> Vec<Body> {
    let mut bodies = Vec::new();
    let (height, sill, head) = (3000.0, 900.0, 2100.0);
    let mut add = |low: Point, size: Point| bodies.push(Body::cuboid(low, size).unwrap());
    let wall = |add: &mut dyn FnMut(Point, Point),
                start: Point,
                along_x: bool,
                length: f64,
                thick: f64,
                windows: &[(f64, f64)]| {
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
            add(l, s);
            let (l, s) = piece(from, to, 0.0, sill);
            add(l, s);
            let (l, s) = piece(from, to, head, height);
            add(l, s);
            at = to;
        }
        let (l, s) = piece(at, length, 0.0, height);
        add(l, s);
    };
    wall(&mut add, [0.0, 0.0, 0.0], true, 2500.0, 300.0, &[]);
    wall(
        &mut add,
        [3500.0, 0.0, 0.0],
        true,
        8500.0,
        300.0,
        &[(2500.0, 5500.0)],
    );
    wall(
        &mut add,
        [0.0, 7700.0, 0.0],
        true,
        12000.0,
        300.0,
        &[(4000.0, 8000.0)],
    );
    wall(
        &mut add,
        [0.0, 300.0, 0.0],
        false,
        7400.0,
        300.0,
        &[(2200.0, 4700.0)],
    );
    wall(&mut add, [11700.0, 300.0, 0.0], false, 7400.0, 300.0, &[]);
    wall(&mut add, [7000.0, 300.0, 0.0], false, 4200.0, 150.0, &[]);
    wall(&mut add, [300.0, 4500.0, 0.0], true, 3700.0, 150.0, &[]);
    add([0.0, 0.0, -300.0], [12000.0, 8000.0, 300.0]);
    add([0.0, 0.0, height], [12000.0, 8000.0, 300.0]);
    add([1800.0, 1600.0, 0.0], [1800.0, 900.0, 750.0]);
    for step in 0..6 {
        let rise = f64::from(step + 1) * 170.0;
        add(
            [8200.0 + f64::from(step) * 280.0, 5600.0, 0.0],
            [280.0, 1100.0, rise],
        );
    }
    add([-3000.0, -3000.0, -700.0], [18000.0, 14000.0, 400.0]);
    let rim = Curve::circle([0.0; 3], 180.0).unwrap();
    let trunk = Body::extruded(&[rim], [0.0, 0.0, 1.0], 3000.0, true).unwrap();
    let crown = Body::sphere([0.0, 0.0, 4300.0], 1800.0).unwrap();
    for at in [
        [-1800.0, -1800.0, -300.0],
        [13800.0, -1500.0, -300.0],
        [13500.0, 9600.0, -300.0],
    ] {
        bodies.push(trunk.translated(at));
        bodies.push(crown.translated(at));
    }
    bodies
}

/// Where a model point lands on the page: the house's middle at the page's middle, at 1:50.
fn on_page(x: f64, y: f64) -> [f64; 2] {
    let middle = [6000.0, 4000.0];
    [
        PAGE[0] / 2.0 + (x - middle[0]) / SCALE,
        PAGE[1] / 2.0 + (y - middle[1]) / SCALE,
    ]
}

/// The plan as a page: cut fills, then the lines below the cut, each weighted by how far below
/// the cut its solid's top lies, then one hatch.
fn plan_page() -> Page {
    let bodies = house();
    let above = Body::cuboid([-1.0e5, -1.0e5, CUT], [2.0e5, 2.0e5, 1.0e5]).unwrap();
    let mut marks = Vec::new();
    let mut below = Vec::new();
    let mut depth_of = Vec::new();
    for body in &bodies {
        let (_, high) = body.bounds().unwrap();
        // The cut outline, filled solid, with a heavy stroke.
        // The cut comes back in pieces (a circle as arcs): joined into closed loops first.
        if let Ok(curves) = body
            .section([0.0, 0.0, CUT], [0.0, 0.0, 1.0])
            .and_then(|pieces| curva_kernel::join_all(&pieces))
        {
            let contours: Vec<Vec<[f64; 2]>> = curves
                .iter()
                .filter_map(|c| c.sample().ok())
                .map(|points| points.iter().map(|p| on_page(p[0], p[1])).collect())
                .collect();
            if !contours.is_empty() {
                for contour in &contours {
                    marks.push(Mark::Line {
                        points: contour.clone(),
                        closed: true,
                        colour: [0.0; 3],
                        width: 0.5,
                        dashes: Vec::new(),
                    });
                }
                marks.push(Mark::Fill {
                    contours,
                    colour: [0.0; 3],
                });
            }
        }
        if high[2] <= CUT {
            below.push(body.clone());
            depth_of.push(CUT - high[2]);
        } else if let Ok(Some(trimmed)) = body.combined(&above, Combine::Difference) {
            below.push(trimmed);
            depth_of.push(0.0);
        }
    }
    let view = View {
        eye: [6000.0, 4000.0, 1.0e5],
        forward: [0.0, 0.0, -1.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        parallel: true,
    };
    let drawn = hidden_lines(
        &below,
        &[],
        &view,
        Options {
            tangent_edges: false,
        },
    )
    .unwrap();
    for line in drawn
        .iter()
        .filter(|l| l.visible && l.kind != LineKind::Tangent)
    {
        let depth = depth_of.get(line.source).copied().unwrap_or(0.0);
        // Thick and black at the cut, thin and light 1.5 m below.
        let t = (depth / 1500.0).clamp(0.0, 1.0);
        let Ok(points) = line.curve.sample() else {
            continue;
        };
        marks.push(Mark::Line {
            // The view plane is centred on the eye: x right, y up from it.
            points: points
                .iter()
                .map(|p| on_page(p[0] + 6000.0, p[1] + 4000.0))
                .collect(),
            closed: false,
            colour: [(0.75 * t) as f32; 3],
            width: 0.35 - 0.22 * t,
            dashes: Vec::new(),
        });
    }
    // One hatch: diagonal lines across the table, 2 mm apart on paper.
    let (low, high) = (on_page(1800.0, 1600.0), on_page(3600.0, 2500.0));
    let mut c = low[0] - (high[1] - low[1]);
    while c < high[0] {
        let start_x = c.max(low[0]);
        let end_x = (c + (high[1] - low[1])).min(high[0]);
        marks.push(Mark::Line {
            points: vec![
                [start_x, low[1] + (start_x - c)],
                [end_x, low[1] + (end_x - c)],
            ],
            closed: false,
            colour: [0.5; 3],
            width: 0.13,
            dashes: Vec::new(),
        });
        c += 2.0;
    }
    Page {
        name: "Plan".into(),
        size: PAGE,
        marks,
    }
}

/// The page as kurbo paths in pixels (downward Y), each with how it is drawn.
enum Item {
    Stroke(BezPath, Color, f64),
    Fill(BezPath, Color),
}

fn items(page: &Page, view: Affine) -> Vec<Item> {
    let px = |p: [f64; 2]| {
        view * vello::kurbo::Point::new(p[0] * PX_PER_MM, (page.size[1] - p[1]) * PX_PER_MM)
    };
    let colour = |c: [f32; 3]| {
        Color::from_rgb8(
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
        )
    };
    let path = |points: &[[f64; 2]], closed: bool| {
        let mut path = BezPath::new();
        for (i, p) in points.iter().enumerate() {
            if i == 0 {
                path.move_to(px(*p));
            } else {
                path.line_to(px(*p));
            }
        }
        if closed {
            path.close_path();
        }
        path
    };
    let zoom = view.as_coeffs()[0];
    page.marks
        .iter()
        .map(|mark| match mark {
            Mark::Line {
                points,
                closed,
                colour: c,
                width,
                ..
            } => {
                // A hairline (0) draws a quarter point wide, as the PDF does.
                let mm = if *width > 0.0 {
                    *width
                } else {
                    0.25 * 25.4 / 72.0
                };
                Item::Stroke(path(points, *closed), colour(*c), mm * PX_PER_MM * zoom)
            }
            Mark::Fill {
                contours,
                colour: c,
            } => {
                let mut all = BezPath::new();
                for contour in contours {
                    for element in path(contour, true).elements() {
                        all.push(*element);
                    }
                }
                Item::Fill(all, colour(*c))
            }
        })
        .collect()
}

fn vello_scene(page: &Page, view: Affine) -> vello::Scene {
    let mut scene = vello::Scene::new();
    for item in items(page, view) {
        match item {
            Item::Stroke(path, colour, width) => {
                let style = Stroke::new(width)
                    .with_caps(Cap::Round)
                    .with_join(Join::Round);
                scene.stroke(&style, Affine::IDENTITY, colour, None, &path);
            }
            Item::Fill(path, colour) => {
                scene.fill(Fill::EvenOdd, Affine::IDENTITY, colour, None, &path);
            }
        }
    }
    scene
}

/// Vello on the graphics card (or software graphics): the picture, and timings.
fn vello_gpu(page: &Page, size: [u32; 2], fallback: bool) -> Result<(Vec<u8>, String), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: fallback,
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let name = adapter.get_info().name;
    // Exactly what Curva asks of the card: the baseline, no extras.
    let (device, queue) = pollster::block_on(
        adapter.request_device(&curva_viewport::device_descriptor(&adapter, false)),
    )
    .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            antialiasing_support: vello::AaSupport::area_only(),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let setup = started.elapsed();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let params = vello::RenderParams {
        base_color: Color::WHITE,
        width: size[0],
        height: size[1],
        antialiasing_method: vello::AaConfig::Area,
    };
    let mut draw = |transform: Affine| -> Result<(), String> {
        let scene = vello_scene(page, transform);
        renderer
            .render_to_texture(&device, &queue, &scene, &view, &params)
            .map_err(|e| e.to_string())?;
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        Ok(())
    };
    let started = Instant::now();
    draw(Affine::IDENTITY)?;
    let first = started.elapsed();
    // Panning and zooming: 60 frames, each a new transform.
    let frames = 60;
    let started = Instant::now();
    for k in 0..frames {
        let f = f64::from(k) / f64::from(frames);
        draw(Affine::translate((-200.0 * f, -100.0 * f)) * Affine::scale(1.0 + f))?;
    }
    let per_frame = started.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    draw(Affine::IDENTITY)?;
    let pixels = read_back(&device, &queue, &texture, size);
    Ok((
        pixels,
        format!(
            "{name}: shaders ready in {:.0} ms, first picture {:.1} ms, pan/zoom {per_frame:.2} ms per frame",
            setup.as_secs_f64() * 1000.0,
            first.as_secs_f64() * 1000.0
        ),
    ))
}

fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
) -> Vec<u8> {
    let row = (size[0] * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(size[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let mut out = Vec::with_capacity((size[0] * size[1] * 4) as usize);
    for line in mapped.chunks(row as usize) {
        out.extend_from_slice(&line[..(size[0] * 4) as usize]);
    }
    out
}

/// vello_cpu, through hayro's copy of it: the picture, and timings.
fn vello_cpu(page: &Page, size: [u32; 2]) -> (Vec<u8>, String) {
    use hayro::vello_cpu::{Pixmap, RenderContext, color::AlphaColor, kurbo as k};
    let to_k = |p: &BezPath| k::BezPath::from_svg(&p.to_svg()).unwrap();
    let draw = |transform: Affine| -> Pixmap {
        let mut context = RenderContext::new(size[0] as u16, size[1] as u16);
        context.set_paint(AlphaColor::from_rgb8(255, 255, 255));
        context.fill_rect(&k::Rect::new(
            0.0,
            0.0,
            f64::from(size[0]),
            f64::from(size[1]),
        ));
        for item in items(page, transform) {
            match item {
                Item::Stroke(path, c, width) => {
                    let [r, g, b, a] = c.to_rgba8().to_u8_array();
                    context.set_paint(AlphaColor::from_rgba8(r, g, b, a));
                    context.set_stroke(
                        k::Stroke::new(width)
                            .with_caps(k::Cap::Round)
                            .with_join(k::Join::Round),
                    );
                    context.stroke_path(&to_k(&path));
                }
                Item::Fill(path, c) => {
                    let [r, g, b, a] = c.to_rgba8().to_u8_array();
                    context.set_paint(AlphaColor::from_rgba8(r, g, b, a));
                    context.set_fill_rule(hayro::vello_cpu::peniko::Fill::EvenOdd);
                    context.fill_path(&to_k(&path));
                }
            }
        }
        context.flush();
        let mut pixmap = Pixmap::new(size[0] as u16, size[1] as u16);
        context.render(&mut pixmap, &mut hayro::vello_cpu::Resources::new());
        pixmap
    };
    let started = Instant::now();
    let pixmap = draw(Affine::IDENTITY);
    let first = started.elapsed();
    let frames = 20;
    let started = Instant::now();
    for f in 0..frames {
        let f = f64::from(f) / f64::from(frames);
        let _ = draw(Affine::translate((-200.0 * f, -100.0 * f)) * Affine::scale(1.0 + f));
    }
    let per_frame = started.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    (
        pixmap.data_as_u8_slice().to_vec(),
        format!(
            "first picture {:.1} ms, pan/zoom {per_frame:.1} ms per frame",
            first.as_secs_f64() * 1000.0
        ),
    )
}

fn rasterise_pdf(pdf: &[u8], size: [u32; 2]) -> Vec<u8> {
    use hayro::hayro_syntax::Pdf;
    let pdf = Pdf::new(std::sync::Arc::new(pdf.to_vec())).unwrap();
    let page = &pdf.pages()[0];
    // Points to pixels: the page is PAGE millimetres wide.
    let scale = (f64::from(size[0]) / (PAGE[0] * 72.0 / 25.4)) as f32;
    let pixmap = hayro::render(
        page,
        &hayro::RenderCache::new(),
        &hayro::hayro_interpret::InterpreterSettings::default(),
        &hayro::RenderSettings::default(),
        &hayro::PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        },
    );
    // hayro's picture can be a pixel wider or taller than ours: copied row by row at its own
    // width, cropped or padded with white to ours.
    let (w, h) = (usize::from(pixmap.width()), usize::from(pixmap.height()));
    let source = pixmap.data_as_u8_slice();
    let (sw, sh) = (size[0] as usize, size[1] as usize);
    let mut data = vec![255u8; sw * sh * 4];
    for y in 0..sh.min(h) {
        let n = sw.min(w) * 4;
        data[y * sw * 4..y * sw * 4 + n].copy_from_slice(&source[y * w * 4..y * w * 4 + n]);
    }
    data
}

/// How different two pictures are: the share of pixels differing by more than 64 in any channel.
fn difference(a: &[u8], b: &[u8]) -> String {
    let pixels = a.len().min(b.len()) / 4;
    let differ = (0..pixels)
        .filter(|&i| (0..3).any(|c| a[i * 4 + c].abs_diff(b[i * 4 + c]) > 64))
        .count();
    format!("{:.2}% of pixels", 100.0 * differ as f64 / pixels as f64)
}

fn save_png(path: &std::path::Path, size: [u32; 2], pixels: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels[..(size[0] * size[1] * 4) as usize])
        .unwrap();
}

/// Vello Hybrid 0.2 (the latest release), which uses wgpu 29 and so runs on a device of its own,
/// not Curva's: the picture, and timings. It asks for no features beyond the standard limits.
fn vello_hybrid(
    page: &Page,
    size: [u32; 2],
    fallback: bool,
) -> Result<(Vec<u8>, String), String> {
    use hayro::vello_cpu::{color::AlphaColor, kurbo as k, peniko};
    use wgpu29 as w;
    let instance = w::Instance::new(w::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&w::RequestAdapterOptions {
        force_fallback_adapter: fallback,
        ..Default::default()
    }))
        .map_err(|e| e.to_string())?;
    let name = adapter.get_info().name;
    let (device, queue) = pollster::block_on(adapter.request_device(&w::DeviceDescriptor {
        required_features: w::Features::empty(),
        required_limits: w::Limits {
            max_texture_dimension_2d: 8192,
            ..w::Limits::default()
        },
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let texture = device.create_texture(&w::TextureDescriptor {
        label: None,
        size: w::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: w::TextureDimension::D2,
        format: w::TextureFormat::Rgba8Unorm,
        usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&w::TextureViewDescriptor::default());
    let started = Instant::now();
    let (mut renderer, mut resources) = vello_hybrid::Renderer::new(
        &device,
        &vello_hybrid::RenderTargetConfig {
            format: w::TextureFormat::Rgba8Unorm,
            width: size[0],
            height: size[1],
        },
    );
    let setup = started.elapsed();
    let to_k = |p: &BezPath| k::BezPath::from_svg(&p.to_svg()).unwrap();
    let mut draw = |transform: Affine| -> Result<(), String> {
        let mut scene = vello_hybrid::Scene::new(size[0] as u16, size[1] as u16);
        scene.set_paint(AlphaColor::from_rgb8(255, 255, 255));
        scene.fill_rect(&k::Rect::new(0.0, 0.0, f64::from(size[0]), f64::from(size[1])));
        for item in items(page, transform) {
            match item {
                Item::Stroke(path, c, width) => {
                    let [r, g, b, a] = c.to_rgba8().to_u8_array();
                    scene.set_paint(AlphaColor::from_rgba8(r, g, b, a));
                    scene.set_stroke(
                        k::Stroke::new(width)
                            .with_caps(k::Cap::Round)
                            .with_join(k::Join::Round),
                    );
                    scene.stroke_path(&to_k(&path));
                }
                Item::Fill(path, c) => {
                    let [r, g, b, a] = c.to_rgba8().to_u8_array();
                    scene.set_paint(AlphaColor::from_rgba8(r, g, b, a));
                    scene.set_fill_rule(peniko::Fill::EvenOdd);
                    scene.fill_path(&to_k(&path));
                }
            }
        }
        let mut encoder = device.create_command_encoder(&w::CommandEncoderDescriptor::default());
        renderer
            .render(
                &scene,
                &mut resources,
                &device,
                &queue,
                &mut encoder,
                &vello_hybrid::RenderSize {
                    width: size[0],
                    height: size[1],
                },
                &view,
                &vello_hybrid::TextureBindings::new(),
            )
            .map_err(|e| format!("{e:?}"))?;
        queue.submit([encoder.finish()]);
        device
            .poll(w::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        Ok(())
    };
    let started = Instant::now();
    draw(Affine::IDENTITY)?;
    let first = started.elapsed();
    let frames = 60;
    let started = Instant::now();
    for f in 0..frames {
        let f = f64::from(f) / f64::from(frames);
        draw(Affine::translate((-200.0 * f, -100.0 * f)) * Affine::scale(1.0 + f))?;
    }
    let per_frame = started.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    draw(Affine::IDENTITY)?;
    // Read back.
    let row = (size[0] * 4).next_multiple_of(w::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&w::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(size[1]),
        usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&w::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        w::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: w::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size[1]),
            },
        },
        w::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(w::MapMode::Read, |_| {});
    device.poll(w::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((size[0] * size[1] * 4) as usize);
    for line in mapped.chunks(row as usize) {
        pixels.extend_from_slice(&line[..(size[0] * 4) as usize]);
    }
    Ok((
        pixels,
        format!(
            "{name}: ready in {:.0} ms, first picture {:.1} ms, pan/zoom {per_frame:.2} ms per frame",
            setup.as_secs_f64() * 1000.0,
            first.as_secs_f64() * 1000.0
        ),
    ))
}
