//! Spike for #186 (decision 0024): the Medium benchmark scene's curves and edges drawn by
//! re_renderer's line renderer, next to Curva's own line drawing, from the same camera.
//!
//! Pictures: the whole scene; a close-up of a few circles (edge smoothing); the close-up with one
//! circle selected through re_renderer's outline. Speed: re_renderer's time per frame while
//! orbiting, rebuilding its line data every frame as it is designed to.
//!
//! Run: `cargo run --release -- <output folder>`.

use curva_viewport::{Camera, Outlines, Picture, save_picture};
use egui_wgpu::wgpu;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder, ViewBuilderId};
use re_renderer::{Color32, LineDrawableBuilder, RenderConfig, RenderContext, Rgba, Size};
use std::time::Instant;

const SIZE: [u32; 2] = [1280, 800];

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();

    let started = Instant::now();
    let outlines = medium_scene();
    let segments: usize = outlines
        .objects
        .iter()
        .map(|(_, p)| p.len().saturating_sub(1))
        .sum();
    println!(
        "scene: {} lines, {segments} segments, built in {:.1} s",
        outlines.objects.len(),
        started.elapsed().as_secs_f64()
    );

    let (low, high) = outlines.bounds().unwrap();
    let mut whole = Camera::default();
    whole.frame(low, high);
    // A close-up of a few circles in a corner of the grid.
    let mut close = Camera::default();
    close.frame([0.0, 0.0, 0.0], [30.0, 30.0, 5.0]);

    // Curva's own line drawing, through its picture renderer.
    for (name, camera) in [("whole", &whole), ("close", &close)] {
        let picture = Picture {
            lines: curva_viewport::object_lines(&outlines, &[]),
            faces: Vec::new(),
            matrix: camera.matrix(f64::from(SIZE[0]) / f64::from(SIZE[1])),
            light: camera.forward().map(|v| v as f32),
            ghosted: false,
        };
        save_picture(picture, SIZE, &out.join(format!("curva-{name}.png"))).unwrap();
    }

    // re_renderer, on a device asked for exactly as Curva asks.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) = pollster::block_on(
        adapter.request_device(&curva_viewport::device_descriptor(&adapter, false)),
    )
    .unwrap();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let started = Instant::now();
    let mut ctx = RenderContext::new(&adapter, device.clone(), queue.clone(), format, |caps| {
        RenderConfig::best_for_device_caps(caps)
    })
    .unwrap();
    println!(
        "re_renderer: {}, ready in {:.0} ms, edge smoothing {:?}",
        adapter.get_info().name,
        started.elapsed().as_secs_f64() * 1000.0,
        ctx.render_config().msaa_mode
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE[0],
            height: SIZE[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    for (name, camera, selected) in [
        ("whole", &whole, None),
        ("close", &close, None),
        ("close-selected", &close, Some(0)),
    ] {
        draw(
            &mut ctx, &device, &queue, &view, &outlines, camera, selected, &mut None,
        );
        save_png(
            &out.join(format!("re-{name}.png")),
            &read_back(&device, &queue, &target),
        );
    }

    // Orbiting: 120 frames, each rebuilding the line data and drawing it.
    let mut camera = whole;
    let (mut build, mut total) = (0.0, 0.0);
    let frames = 120;
    for _ in 0..frames {
        camera.orbit(4.0, 0.0);
        let started = Instant::now();
        let built = draw(&mut ctx, &device, &queue, &view, &outlines, &camera, None, &mut None);
        total += started.elapsed().as_secs_f64() * 1000.0;
        build += built;
    }
    println!(
        "re_renderer orbit: {:.2} ms per frame in all, of which {:.2} ms rebuilding its line data",
        total / f64::from(frames),
        build / f64::from(frames)
    );
    // Orbiting again with the line data built once and kept, as Curva keeps its lines.
    ctx.begin_frame();
    let mut held = Some(build_lines(&ctx, &outlines, None));
    let mut total = 0.0;
    for _ in 0..frames {
        camera.orbit(4.0, 0.0);
        let started = Instant::now();
        draw(&mut ctx, &device, &queue, &view, &outlines, &camera, None, &mut held);
        total += started.elapsed().as_secs_f64() * 1000.0;
    }
    println!("re_renderer orbit, lines kept: {:.2} ms per frame", total / f64::from(frames));
    save_png(&out.join("re-kept.png"), &read_back(&device, &queue, &target));
}

/// The Medium scene's lines: 5,000 circles, and the edges of 500 cylinders (copies of one, so the
/// scene builds quickly; their lines are the same as separately made cylinders').
fn medium_scene() -> Outlines {
    let mut doc = curva_document::Document::default();
    let spacing = 12.0;
    for k in 0..5000 {
        let (x, y) = (f64::from(k % 71) * spacing, f64::from(k / 71) * spacing);
        doc.add(curva_kernel::Curve::circle([x, y, 0.0], 5.0).unwrap());
    }
    let base = curva_kernel::Curve::circle([0.0; 3], 5.0).unwrap();
    let cylinder = curva_kernel::Body::extruded(&[base], [0.0, 0.0, 1.0], spacing, true).unwrap();
    let beside = 72.0 * spacing;
    for k in 0..500 {
        let (x, y) = (
            beside + f64::from(k % 23) * spacing,
            f64::from(k / 23) * spacing,
        );
        doc.add_body(cylinder.translated([x, y, 0.0]));
    }
    Outlines::of(doc.state())
}

/// Draws the scene's lines with re_renderer from `camera` into `view`. Returns how long building
/// the line data took, in milliseconds.
fn draw(
    ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    outlines: &Outlines,
    camera: &Camera,
    selected: Option<usize>,
    held: &mut Option<re_renderer::renderer::LineDrawData>,
) -> f64 {
    ctx.begin_frame();
    let started = Instant::now();
    // Kept from an earlier frame when asked (`held` already set), as Curva keeps its lines.
    let data = match held.clone() {
        Some(kept) => kept,
        None => build_lines(ctx, outlines, selected),
    };
    let built = started.elapsed().as_secs_f64() * 1000.0;
    render(ctx, device, queue, view, camera, selected, data);
    built
}

/// re_renderer's line data for the scene, rebuilt from the outlines.
fn build_lines(
    ctx: &RenderContext,
    outlines: &Outlines,
    selected: Option<usize>,
) -> re_renderer::renderer::LineDrawData {
    let mut lines = LineDrawableBuilder::new(ctx);
    {
        let mut batch = lines.batch("scene");
        for (index, (_, points)) in outlines.objects.iter().enumerate() {
            let strip: Vec<glam::Vec3> = points
                .iter()
                .map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
                .collect();
            let colour = outlines.colour_of(outlines.objects[index].0, &[]).map_or(
                Color32::WHITE,
                |[r, g, b]| {
                    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
                },
            );
            // Curva's default line is 2 pixels wide: a radius of 1 point.
            let mut builder = batch
                .add_strip(strip.into_iter())
                .radius(Size::new_ui_points(1.0))
                .color(colour);
            if selected == Some(index) {
                builder = builder.outline_mask_ids(re_renderer::OutlineMaskPreference::some(1, 0));
            }
            drop(builder);
        }
    }
    lines.into_draw_data().unwrap()
}

/// Draws `data` from `camera` into `view`.
fn render(
    ctx: &mut RenderContext,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    camera: &Camera,
    selected: Option<usize>,
    data: re_renderer::renderer::LineDrawData,
) {
    let look = camera.look();
    let eye = glam::Vec3::new(look.eye[0] as f32, look.eye[1] as f32, look.eye[2] as f32);
    let forward = glam::Vec3::new(
        look.forward[0] as f32,
        look.forward[1] as f32,
        look.forward[2] as f32,
    );
    let up = glam::Vec3::new(look.up[0] as f32, look.up[1] as f32, look.up[2] as f32);
    let aspect = f64::from(SIZE[0]) / f64::from(SIZE[1]);
    // Curva's projection's vertical scale gives its field of view.
    let m = camera.matrix(aspect);
    // The focal length is the length of the matrix's second row's first three entries (its
    // camera up direction times the focal length).
    let focal = (m[0][1].powi(2) + m[1][1].powi(2) + m[2][1].powi(2)).sqrt();
    let vertical_fov = 2.0 * (1.0 / focal).atan();
    let mut builder = ViewBuilder::new(
        ctx,
        TargetConfiguration {
            name: "spike".into(),
            resolution_in_pixel: SIZE,
            view_from_world: re_math::IsoTransform::look_at_rh(eye, eye + forward, up).unwrap(),
            projection_from_view: Projection::Perspective {
                vertical_fov,
                near_plane_distance: 0.1,
                aspect_ratio: SIZE[0] as f32 / SIZE[1] as f32,
            },
            outline_config: selected.map(|_| re_renderer::OutlineConfig {
                outline_radius_pixel: 2.0,
                color_layer_a: Rgba::from_rgb(0.878, 0.471, 0.184),
                color_layer_b: Rgba::TRANSPARENT,
            }),
            ..Default::default()
        },
        ViewBuilderId::new(1),
    )
    .unwrap();
    builder.queue_draw(ctx, data).unwrap();
    // Curva's picture background (0.11 grey, as stored).
    let commands = builder
        .draw(ctx, Rgba::from_rgb(0.012, 0.012, 0.012))
        .unwrap();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        builder.composite(ctx, &mut pass);
    }
    ctx.before_submit();
    queue.submit([commands, encoder.finish()]);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let row = (SIZE[0] * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(SIZE[1]),
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
                rows_per_image: Some(SIZE[1]),
            },
        },
        wgpu::Extent3d {
            width: SIZE[0],
            height: SIZE[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let mut out = Vec::new();
    for line in mapped.chunks(row as usize) {
        out.extend_from_slice(&line[..(SIZE[0] * 4) as usize]);
    }
    out
}

fn save_png(path: &std::path::Path, pixels: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), SIZE[0], SIZE[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}
