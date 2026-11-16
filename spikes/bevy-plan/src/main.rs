//! Spike for #235: the test house's plan, cut at 1.2 m, built as Curva's own vector page
//! (`curva_document::drawing::Page`) exactly as in the Vello spike (#185), then drawn by Bevy
//! 0.20.1 on the device Curva asks for: every line and fill turned into triangles with its true
//! width in paper millimetres (lyon), drawn flat with edge smoothing. Compared with the PDF from
//! `curva-pdf`, rasterised with hayro, at 1x and at 4x zoom.
//!
//! Run: `cargo run --release -- <output folder>`.

use bevy::app::AppLabel;
use bevy::{
    app::{PanicHandlerPlugin, SubApps, TerminalCtrlCHandlerPlugin},
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::Image,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::{
        RenderApp, RenderPlugin,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{TextureFormat, TextureUsages},
        renderer::{RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue},
        settings::RenderCreation,
        texture::GpuImage,
    },
    window::ExitCondition,
};
use curva_document::drawing::{Mark, Page};
use curva_kernel::hidden::{LineKind, Options, View, hidden_lines};
use curva_kernel::{Body, Combine, Curve, Point};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers, math::point, path::Path,
};
use std::time::Instant;

/// Plan cut height and drawing scale.
const CUT: f64 = 1200.0;
const SCALE: f64 = 50.0;
/// A3 landscape, millimetres.
const PAGE: [f64; 2] = [420.0, 297.0];
/// Pixels per paper millimetre for the pictures (about 150 dpi).
const PX_PER_MM: f64 = 6.0;
/// The zoomed comparison: 4x, centred on a cut wall and the hatch (page millimetres, y up).
const ZOOM: f64 = 4.0;
const ZOOM_CENTRE: [f64; 2] = [120.0, 108.5];
/// How finely round caps and joins are cut into triangles, in paper millimetres.
const TOLERANCE: f32 = 0.005;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "out".into()));
    std::fs::create_dir_all(&out).unwrap();

    let started = Instant::now();
    let page = plan_page();
    let lines = page
        .marks
        .iter()
        .filter(|m| matches!(m, Mark::Line { .. }))
        .count();
    println!(
        "page: {lines} lines, {} fills, built in {:.0} ms",
        page.marks.len() - lines,
        started.elapsed().as_secs_f64() * 1000.0
    );
    let size = [(PAGE[0] * PX_PER_MM) as u32, (PAGE[1] * PX_PER_MM) as u32];

    let pdf = curva_pdf::write_pdf(std::slice::from_ref(&page)).unwrap();
    std::fs::write(out.join("plan.pdf"), &pdf).unwrap();
    let reference = rasterise_pdf(&pdf, size, 1.0, [0.0, 0.0]);
    save_png(&out.join("plan-pdf.png"), size, &reference);
    let zoom_low = [
        ZOOM_CENTRE[0] - PAGE[0] / ZOOM / 2.0,
        // Top edge of the zoomed window, measured down from the page's top, for hayro.
        PAGE[1] - (ZOOM_CENTRE[1] + PAGE[1] / ZOOM / 2.0),
    ];
    let reference_zoom = rasterise_pdf(&pdf, size, ZOOM, zoom_low);
    save_png(&out.join("plan-pdf-zoom.png"), size, &reference_zoom);

    let started = Instant::now();
    let mesh = page_mesh(&page);
    let triangles = mesh.indices().map_or(0, |i| i.len() / 3);
    println!(
        "triangles: {triangles}, made in {:.0} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );

    for samples in [4, 8] {
        let started = Instant::now();
        let mut viewer = Viewer::new(mesh.clone(), size, samples);
        viewer.look(1.0, [PAGE[0] / 2.0, PAGE[1] / 2.0]);
        let pixels = viewer.picture();
        println!(
            "\n== Bevy, {samples} samples per pixel\nfirst picture: {:.0} ms (start-up included)",
            started.elapsed().as_secs_f64() * 1000.0
        );
        save_png(
            &out.join(format!("plan-bevy-{samples}x.png")),
            size,
            &pixels,
        );
        save_png(
            &out.join(format!("plan-bevy-{samples}x-diff.png")),
            size,
            &difference_picture(&pixels, &reference),
        );
        println!("1x:  {}", difference(&pixels, &reference));

        viewer.look(ZOOM, ZOOM_CENTRE);
        let zoomed = viewer.picture();
        save_png(
            &out.join(format!("plan-bevy-{samples}x-zoom.png")),
            size,
            &zoomed,
        );
        save_png(
            &out.join(format!("plan-bevy-{samples}x-zoom-diff.png")),
            size,
            &difference_picture(&zoomed, &reference_zoom),
        );
        println!("4x:  {}", difference(&zoomed, &reference_zoom));

        // Panning and zooming: the triangles stay on the card; only the camera moves.
        let mut times = Vec::new();
        for frame in 0..240 {
            let t = f64::from(frame) / 240.0;
            let zoom = 1.0 + 3.0 * (t * std::f64::consts::PI).sin();
            let centre = [
                PAGE[0] / 2.0 + 40.0 * (t * 6.283).cos(),
                PAGE[1] / 2.0 + 30.0 * (t * 6.283).sin(),
            ];
            let started = Instant::now();
            viewer.look(zoom, centre);
            viewer.update();
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "redraw while panning and zooming: median {:.2} ms, 99th percentile {:.2} ms",
            times[times.len() / 2],
            times[times.len() * 99 / 100]
        );
        if let Some(bytes) = viewer.gpu_bytes() {
            println!("graphics memory: {} MB", bytes / 1_000_000);
        }
    }
}

/// The page as one mesh of flat-coloured triangles in paper millimetres, y up, each mark a
/// little above the one before so later marks cover earlier ones, as on paper.
fn page_mesh(page: &Page) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut fill = FillTessellator::new();
    let mut stroke = StrokeTessellator::new();
    let path_of = |points: &[[f64; 2]], closed: bool| {
        let mut builder = Path::builder();
        for (i, p) in points.iter().enumerate() {
            let at = point(p[0] as f32, p[1] as f32);
            if i == 0 {
                builder.begin(at);
            } else {
                builder.line_to(at);
            }
        }
        builder.end(closed);
        builder.build()
    };
    for (n, mark) in page.marks.iter().enumerate() {
        let z = n as f32 * 0.002;
        let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
        let colour = match mark {
            Mark::Line {
                points,
                closed,
                colour,
                width,
                ..
            } => {
                // A hairline (0) draws a quarter point wide, as the PDF does.
                let mm = if *width > 0.0 {
                    *width
                } else {
                    0.25 * 25.4 / 72.0
                };
                let options = StrokeOptions::tolerance(TOLERANCE)
                    .with_line_width(mm as f32)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round);
                stroke
                    .tessellate_path(
                        &path_of(points, *closed),
                        &options,
                        &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex| {
                            v.position().to_array()
                        }),
                    )
                    .unwrap();
                colour
            }
            Mark::Fill { contours, colour } => {
                let mut builder = Path::builder();
                for contour in contours {
                    for (i, p) in contour.iter().enumerate() {
                        let at = point(p[0] as f32, p[1] as f32);
                        if i == 0 {
                            builder.begin(at);
                        } else {
                            builder.line_to(at);
                        }
                    }
                    builder.end(true);
                }
                let options = FillOptions::tolerance(TOLERANCE).with_fill_rule(FillRule::EvenOdd);
                fill.tessellate_path(
                    &builder.build(),
                    &options,
                    &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position().to_array()),
                )
                .unwrap();
                colour
            }
        };
        // Colours as stored, mixed as stored at edges, as PDF viewers and printers do (and Curva's
        // own drawing): handed to Bevy as if already linear, drawn into a plain picture.
        let linear = [colour[0], colour[1], colour[2], 1.0];
        let base = positions.len() as u32;
        positions.extend(buffers.vertices.iter().map(|p| [p[0], p[1], z]));
        colours.extend(std::iter::repeat_n(linear, buffers.vertices.len()));
        indices.extend(buffers.indices.iter().map(|i| base + i));
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

/// Bevy on a device made as Curva makes its own, drawing the page with an orthographic camera
/// looking straight down at it.
struct Viewer {
    apps: SubApps,
    target: Handle<Image>,
    camera: Entity,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: [u32; 2],
}

impl Viewer {
    fn new(mesh: Mesh, size: [u32; 2], samples: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        // The recommended card's full features (decision 0026), for eight samples per pixel.
        let mut descriptor = curva_viewport::device_descriptor(&adapter, true);
        descriptor.required_features = adapter.features()
            - wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
            - wgpu::Features::all_experimental_mask();
        descriptor.required_limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).unwrap();
        let render = RenderPlugin {
            render_creation: RenderCreation::manual(
                RenderDevice::new(device.clone()),
                RenderQueue::new(queue.clone()),
                RenderAdapterInfo::new(adapter.get_info()),
                RenderAdapter::new(adapter),
                RenderInstance::new(instance),
            ),
            synchronous_pipeline_compilation: true,
            ..default()
        };
        let window = WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        };
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(window)
                .set(render)
                .disable::<PipelinedRenderingPlugin>()
                .disable::<TerminalCtrlCHandlerPlugin>()
                .disable::<PanicHandlerPlugin>(),
        );
        app.finish();
        app.cleanup();
        let mut apps = std::mem::take(app.sub_apps_mut());
        let world = apps.main.world_mut();
        let mut image =
            Image::new_target_texture(size[0], size[1], TextureFormat::Rgba8Unorm, None);
        image.asset_usage = RenderAssetUsages::RENDER_WORLD;
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = world.resource_mut::<Assets<Image>>().add(image);
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                // lyon's triangles can face either way; both sides are drawn.
                cull_mode: None,
                double_sided: true,
                ..default()
            });
        world.spawn((Mesh3d(mesh), MeshMaterial3d(material)));
        let camera = world
            .spawn((
                Camera3d::default(),
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::WHITE),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Tonemapping::None,
                DebandDither::Disabled,
                if samples == 8 {
                    Msaa::Sample8
                } else {
                    Msaa::Sample4
                },
                Projection::Orthographic(OrthographicProjection {
                    near: -100.0,
                    far: 100.0,
                    ..OrthographicProjection::default_3d()
                }),
            ))
            .id();
        let mut viewer = Viewer {
            apps,
            target,
            camera,
            device,
            queue,
            size,
        };
        viewer.look(1.0, [PAGE[0] / 2.0, PAGE[1] / 2.0]);
        viewer
    }

    /// Shows the page at `zoom` (1 is the whole page) around `centre`, in page millimetres.
    fn look(&mut self, zoom: f64, centre: [f64; 2]) {
        let world = self.apps.main.world_mut();
        let projection = Projection::Orthographic(OrthographicProjection {
            near: -100.0,
            far: 100.0,
            scaling_mode: ScalingMode::Fixed {
                width: (PAGE[0] / zoom) as f32,
                height: (PAGE[1] / zoom) as f32,
            },
            ..OrthographicProjection::default_3d()
        });
        world.entity_mut(self.camera).insert((
            projection,
            Transform::from_xyz(centre[0] as f32, centre[1] as f32, 50.0),
        ));
    }

    /// One frame, waited for.
    fn update(&mut self) {
        self.apps.update();
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }

    /// Draws a frame and reads it back.
    fn picture(&mut self) -> Vec<u8> {
        // The first updates send the page and the picture to the card.
        for _ in 0..3 {
            self.update();
        }
        let render = self.apps.sub_apps.get(&RenderApp.intern()).unwrap();
        let images = render.world().resource::<RenderAssets<GpuImage>>();
        let texture: wgpu::Texture = (*images.get(&self.target).unwrap().texture).clone();
        let [width, height] = self.size;
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let mapped = slice.get_mapped_range().unwrap();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for line in mapped.chunks(row as usize) {
            pixels.extend_from_slice(&line[..(width * 4) as usize]);
        }
        pixels
    }

    fn gpu_bytes(&self) -> Option<u64> {
        self.device
            .generate_allocator_report()
            .map(|r| r.total_allocated_bytes)
    }
}
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


/// The PDF rasterised by hayro at `zoom` times the picture's scale, cropped to the picture's size
/// from `low` (page millimetres from the left and from the top).
fn rasterise_pdf(pdf: &[u8], size: [u32; 2], zoom: f64, low: [f64; 2]) -> Vec<u8> {
    use hayro::hayro_syntax::Pdf;
    let pdf = Pdf::new(std::sync::Arc::new(pdf.to_vec())).unwrap();
    let page = &pdf.pages()[0];
    let scale = (f64::from(size[0]) / (PAGE[0] * 72.0 / 25.4) * zoom) as f32;
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
    let (w, h) = (usize::from(pixmap.width()), usize::from(pixmap.height()));
    let source = pixmap.data_as_u8_slice();
    let (sw, sh) = (size[0] as usize, size[1] as usize);
    let px = PX_PER_MM * zoom;
    let (ox, oy) = ((low[0] * px).round() as usize, (low[1] * px).round() as usize);
    let mut data = vec![255u8; sw * sh * 4];
    for y in 0..sh {
        if oy + y >= h {
            break;
        }
        let n = sw.min(w.saturating_sub(ox)) * 4;
        let from = ((oy + y) * w + ox) * 4;
        data[y * sw * 4..y * sw * 4 + n].copy_from_slice(&source[from..from + n]);
    }
    data
}

/// How different two pictures are: the share of pixels differing by more than a quarter of the
/// colour range (64) in any channel, as in the Vello test, and by more than a sixteenth (16).
fn difference(a: &[u8], b: &[u8]) -> String {
    let pixels = a.len().min(b.len()) / 4;
    let share = |limit: u8| {
        let differ = (0..pixels)
            .filter(|&i| (0..3).any(|c| a[i * 4 + c].abs_diff(b[i * 4 + c]) > limit))
            .count();
        100.0 * differ as f64 / pixels as f64
    };
    format!(
        "{:.3}% of pixels off by more than a quarter, {:.2}% by more than a sixteenth",
        share(64),
        share(16)
    )
}

/// Where two pictures differ: white where they agree, red where off by more than a quarter,
/// orange where off by more than a sixteenth.
fn difference_picture(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.chunks(4)
        .zip(b.chunks(4))
        .flat_map(|(p, q)| {
            let most = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0);
            if most > 64 {
                [220, 0, 0, 255]
            } else if most > 16 {
                [255, 170, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .collect()
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
