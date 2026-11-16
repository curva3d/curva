//! Spike for #199: viewport-lib drawing Curva's benchmark scenes, on Curva's own graphics device,
//! from Curva's camera. Not part of Curva's build; not merged.
//!
//! Run: `cargo run --release -- <output folder> [pictures|orbit <scene>]`.

#[path = "../../common/scenes.rs"]
mod scenes;

use glam::{DVec3, Mat3, Vec3};
use scenes::{Point, Scene};
use std::time::Instant;
use viewport_lib::Uploads;
use viewport_lib::{
    ClipObject, ClipShape, Colour, FrameData, Material, MeshItem, PolylineItem, RenderCamera,
    RendererConfig, StrokePattern, SurfaceSubmission, Transform, ViewportRenderer, wgpu,
};

/// The orbit picture: 1920 by 1080, or with SMALL=1 the 1280 by 800 Curva's window has here.
fn size() -> [u32; 2] {
    if std::env::var("SMALL").is_ok_and(|v| v == "1") {
        [1280, 800]
    } else {
        [1920, 1080]
    }
}
const PICTURE: [u32; 2] = [1200, 800];
const FRAMES: usize = 240;
const FOV: f64 = 0.7;
/// Lines per line item.
const PIECE: usize = 4_000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let what = args.get(2).map_or("pictures", String::as_str);

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    // Exactly the device Curva asks for, with the card's timer when it has one.
    let (device, queue) = pollster::block_on(
        adapter.request_device(&curva_viewport::device_descriptor(&adapter, true)),
    )
    .unwrap();
    println!(
        "card: {} ({:?})",
        adapter.get_info().name,
        adapter.get_info().backend
    );

    match what {
        "pictures" => pictures(&device, &queue, &out),
        "orbit" => {
            let name = args.get(3).map_or("medium", String::as_str);
            orbit(&device, &queue, name, &out);
        }
        "subd" => {
            let cage = args.get(3).and_then(|c| c.parse().ok()).unwrap_or(32);
            subd_edit(&device, &queue, cage);
        }
        other => panic!("unknown {other}"),
    }
}

/// Curva's camera as viewport-lib's: the same eye, target, field of view and depth range.
fn camera(c: &curva_viewport::Camera, size: [u32; 2]) -> RenderCamera {
    let (cp, sp) = (c.pitch.cos(), c.pitch.sin());
    let eye = DVec3::new(
        c.target[0] + c.distance * cp * c.yaw.cos(),
        c.target[1] + c.distance * cp * c.yaw.sin(),
        c.target[2] + c.distance * sp,
    );
    let target = DVec3::from_array(c.target);
    let cam = viewport_lib::Camera::look_at(eye, target, Vec3::Z)
        .with_projection(viewport_lib::Projection::Perspective {
            fov_y: FOV as f32,
            near: Some((c.distance * 0.01) as f32),
            far: Some((c.distance * 100.0) as f32),
        })
        .with_aspect(size[0] as f32 / size[1] as f32);
    RenderCamera::from_camera(&cam)
}

/// What was uploaded for a scene.
struct Uploaded {
    items: Vec<MeshItem>,
    lines: Vec<PolylineItem>,
}

/// How the lines go to viewport-lib.
#[derive(Clone, Copy, PartialEq)]
enum Lines {
    /// All in world coordinates, single precision, as viewport-lib takes them.
    World,
    /// Relative to a point near them that single precision holds exactly, with that point as
    /// the item's placement: what Curva would have to do far from the origin.
    Anchored,
}

fn upload(
    renderer: &mut ViewportRenderer,
    device: &wgpu::Device,
    scene: &Scene,
    lines: Lines,
    width: f32,
) -> Uploaded {
    let white = renderer
        .resources_mut()
        .upload_material(Material::from_colour(Colour::srgb_rgb(0.92, 0.92, 0.92)))
        .unwrap();
    let green = renderer
        .resources_mut()
        .upload_material(Material::from_colour(Colour::srgb_rgb(0.55, 0.7, 0.5)))
        .unwrap();
    let ids: Vec<_> = scene
        .shapes
        .iter()
        .map(|m| {
            let mut data = viewport_lib::MeshData::default();
            data.positions = m.positions.iter().map(|p| p.map(|v| v as f32)).collect();
            data.normals = m.normals.iter().map(|p| p.map(|v| v as f32)).collect();
            data.indices = m.triangles.iter().flatten().copied().collect();
            renderer
                .resources_mut()
                .upload_mesh_data(device, &data)
                .unwrap()
        })
        .collect();
    let first_tree = scene.placed.len() - scene.tree_parts;
    let items = scene
        .placed
        .iter()
        .enumerate()
        .map(|(i, (shape, at))| {
            let mut item = MeshItem::default();
            item.mesh_id = ids[*shape];
            item.model = Transform {
                translation: DVec3::from_array(*at),
                rotation_scale: Mat3::IDENTITY,
            };
            item.material = if i >= first_tree { green } else { white };
            item.settings.pick_id = viewport_lib::PickId(i as u64 + 1);
            item
        })
        .collect();
    let mut strips = scene.curves.clone();
    strips.extend(scene.edges());
    let line_items = match lines {
        // In pieces: viewport-lib puts one item's lines in one block of graphics memory, and
        // the card refuses blocks over 256 MB (Large's lines in one item need 820 MB).
        Lines::World => strips
            .chunks(PIECE)
            .map(|c| polyline(c, [0.0; 3], width))
            .collect(),
        Lines::Anchored => {
            // One item per 2^20 mm (about 1 km) square, its points relative to the square's
            // corner, which single precision holds exactly.
            let cell = 1048576.0;
            let mut groups: std::collections::BTreeMap<(i64, i64), Vec<Vec<Point>>> =
                Default::default();
            for s in strips {
                let key = (
                    (s[0][0] / cell).floor() as i64,
                    (s[0][1] / cell).floor() as i64,
                );
                groups.entry(key).or_default().push(s);
            }
            groups
                .into_iter()
                .flat_map(|((x, y), g)| {
                    g.chunks(PIECE)
                        .map(|c| polyline(c, [x as f64 * cell, y as f64 * cell, 0.0], width))
                        .collect::<Vec<_>>()
                })
                .collect()
        }
    };
    Uploaded {
        items,
        lines: line_items,
    }
}

fn polyline(strips: &[Vec<Point>], anchor: Point, width: f32) -> PolylineItem {
    let mut item = PolylineItem::default();
    for s in strips {
        item.strip_lengths.push(s.len() as u32);
        item.positions.extend(s.iter().map(|p| {
            [
                (p[0] - anchor[0]) as f32,
                (p[1] - anchor[1]) as f32,
                (p[2] - anchor[2]) as f32,
            ]
        }));
    }
    item.model =
        glam::Mat4::from_translation(DVec3::from_array(anchor).as_vec3()).to_cols_array_2d();
    item.default_colour = Colour::srgb_rgb(0.1, 0.1, 0.1);
    item.line_width = width;
    item
}

fn frame_for(up: &Uploaded, cam: RenderCamera, size: [u32; 2], ssao: bool) -> FrameData {
    let mut frame = FrameData::default();
    frame.camera.render_camera = cam;
    frame.camera.viewport_size = [size[0] as f32, size[1] as f32];
    frame.viewport.show_grid = false;
    frame.viewport.show_axes_indicator = false;
    frame.viewport.background_colour = Some(Colour::srgb_rgb(1.0, 1.0, 1.0));
    frame.scene.surfaces = SurfaceSubmission::Flat(up.items.clone().into());
    *frame.scene.items_mut::<PolylineItem>() = up.lines.clone();
    frame.effects.post_process.ssao = ssao;
    frame.effects.post_process.fxaa = true;
    frame.interaction.outline_selected = true;
    frame
}

fn renderer(device: &wgpu::Device) -> ViewportRenderer {
    // Four samples per pixel for edge smoothing, as Curva draws.
    ViewportRenderer::with_config(
        device,
        &RendererConfig::new(wgpu::TextureFormat::Rgba8UnormSrgb).with_sample_count(
            std::env::var("SAMPLES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
        ),
    )
}

fn framed(scene: &Scene, zoom: f64) -> curva_viewport::Camera {
    let (low, high) = scene.bounds();
    let mut c = curva_viewport::Camera::default();
    c.frame(low, high);
    c.zoom(zoom);
    c
}

fn save(path: &std::path::Path, pixels: &[u8], size: [u32; 2]) {
    let file = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(pixels).unwrap();
}

fn shot(
    r: &mut ViewportRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &FrameData,
    path: &std::path::Path,
) {
    let px = r.render_offscreen(device, queue, frame, PICTURE[0], PICTURE[1]);
    save(path, &px, PICTURE);
}

fn pictures(device: &wgpu::Device, queue: &wgpu::Queue, out: &std::path::Path) {
    // The test building and house, framed as Curva's `--picture --scene building` frames them.
    for (scene_name, clip) in [("building", false), ("house", false), ("house", true)] {
        let scene = scenes::named(scene_name);
        let mut r = renderer(device);
        let up = upload(&mut r, device, &scene, Lines::World, 1.5);
        let cam = framed(&scene, 0.74);
        for ssao in [false, true] {
            let mut frame = frame_for(&up, camera(&cam, PICTURE), PICTURE, ssao);
            if clip {
                // A plan cut at 1.2 m, filled where it cuts solids.
                let mut cut = ClipObject::default();
                cut.shape = ClipShape::Plane {
                    normal: [0.0, 0.0, -1.0],
                    distance: 1200.0,
                    cap_colour: Some([0.2, 0.2, 0.2, 1.0]),
                };
                frame.effects.clip.objects = vec![cut];
                frame.effects.clip.cap_fill_enabled = true;
            }
            let name = format!(
                "vl-{scene_name}{}{}.png",
                if clip { "-cut" } else { "" },
                if ssao { "-ssao" } else { "" }
            );
            shot(&mut r, device, queue, &frame, &out.join(name));
        }
        // Closer, to judge edges, lines and the cut.
        let mut near = curva_viewport::Camera::default();
        near.frame([0.0, 0.0, 0.0], [12000.0, 8000.0, 3000.0]);
        near.zoom(0.7);
        if clip {
            near.pitch = 1.25;
        }
        let mut frame = frame_for(&up, camera(&near, PICTURE), PICTURE, true);
        if clip {
            let mut cut = ClipObject::default();
            cut.shape = ClipShape::Plane {
                normal: [0.0, 0.0, -1.0],
                distance: 1200.0,
                cap_colour: Some([0.2, 0.2, 0.2, 1.0]),
            };
            frame.effects.clip.objects = vec![cut];
            frame.effects.clip.cap_fill_enabled = true;
        }
        let name = format!("vl-{scene_name}{}-near.png", if clip { "-cut" } else { "" });
        shot(&mut r, device, queue, &frame, &out.join(name));
    }

    // Lines up close: a corner of the Medium scene's circles, with a wide line, a dashed line,
    // and a circle zoomed far in (its straight pieces show).
    let scene = scenes::named("medium");
    let mut r = renderer(device);
    let mut up = upload(&mut r, device, &scene, Lines::World, 2.0);
    let mut wide = polyline(&scene.curves[1..2], [0.0; 3], 6.0);
    wide.default_colour = Colour::srgb_rgb(0.8, 0.1, 0.1);
    up.lines.push(wide);
    let mut dashed = polyline(&scene.curves[2..3], [0.0; 3], 2.0);
    dashed.stroke_pattern = StrokePattern::Dashed {
        dash_length: 10.0,
        gap_length: 6.0,
        offset: 0.0,
    };
    dashed.default_colour = Colour::srgb_rgb(0.1, 0.2, 0.8);
    up.lines.push(dashed);
    let mut close = curva_viewport::Camera::default();
    close.frame([0.0, 0.0, 0.0], [30.0, 30.0, 5.0]);
    let frame = frame_for(&up, camera(&close, PICTURE), PICTURE, false);
    shot(
        &mut r,
        device,
        queue,
        &frame,
        &out.join("vl-lines-close.png"),
    );
    let mut zoomed = curva_viewport::Camera::default();
    zoomed.frame([4.0, -1.0, 0.0], [6.0, 1.0, 0.0]);
    let frame = frame_for(&up, camera(&zoomed, PICTURE), PICTURE, false);
    shot(
        &mut r,
        device,
        queue,
        &frame,
        &out.join("vl-arc-zoomed.png"),
    );

    // Far from the origin: the same close-up 500 km away, lines as viewport-lib takes them and
    // anchored.
    for (scene_name, lines) in [
        ("large", Lines::World),
        ("far", Lines::World),
        ("far", Lines::Anchored),
    ] {
        let scene = scenes::named(scene_name);
        let shift = if scene_name == "far" { 5.0e8 } else { 0.0 };
        let mut r = renderer(device);
        let up = upload(&mut r, device, &scene, lines, 2.0);
        let mut close = curva_viewport::Camera::default();
        close.frame([shift, shift, 0.0], [shift + 40.0, shift + 40.0, 10.0]);
        let frame = frame_for(&up, camera(&close, PICTURE), PICTURE, false);
        let tag = if lines == Lines::Anchored {
            "-anchored"
        } else {
            ""
        };
        shot(
            &mut r,
            device,
            queue,
            &frame,
            &out.join(format!("vl-{scene_name}{tag}-close.png")),
        );
    }
}

fn gpu_bytes(device: &wgpu::Device) -> Option<u64> {
    device
        .generate_allocator_report()
        .map(|r| r.total_allocated_bytes)
}

fn orbit(device: &wgpu::Device, queue: &wgpu::Queue, name: &str, out: &std::path::Path) {
    let started = Instant::now();
    let scene = scenes::named(name);
    let strips = scene.curves.len() + scene.edges().len();
    println!(
        "scene {name}: {} solids ({} shapes), {} triangles, {strips} lines, built in {:.1} s",
        scene.placed.len(),
        scene.shapes.len(),
        scene.triangles(),
        started.elapsed().as_secs_f64()
    );
    let before = gpu_bytes(device);
    let started = Instant::now();
    let mut r = renderer(device);
    let lines = if name == "far" {
        Lines::Anchored
    } else {
        Lines::World
    };
    let up = upload(&mut r, device, &scene, lines, 2.0);
    let mut cam = framed(&scene, 1.0);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size()[0],
            height: size()[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut frame = frame_for(&up, camera(&cam, size()), size(), true);
    // With KEEP=1 the lines are uploaded once and referred to each frame, as Curva keeps its
    // lines; otherwise they are handed over each frame, viewport-lib's ordinary way.
    if std::env::var("KEEP").is_ok_and(|v| v == "1") {
        let refs = up
            .lines
            .iter()
            .map(|item| {
                let mut kept =
                    viewport_lib::PolylineRefItem::new(r.upload(device, queue, item).unwrap());
                kept.model = item.model;
                kept
            })
            .collect();
        frame.scene.items_mut::<PolylineItem>().clear();
        *frame.scene.items_mut::<viewport_lib::PolylineRefItem>() = refs;
        println!("lines kept on the card");
    }
    r.render_to_texture(device, queue, &view, &frame);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    println!("first frame: {:.2} s", started.elapsed().as_secs_f64());
    if let (Some(a), Some(b)) = (before, gpu_bytes(device)) {
        println!(
            "graphics memory: {:.0} MB",
            b.saturating_sub(a) as f64 / 1e6
        );
    }
    let stats = r.last_frame_stats();
    println!(
        "draw calls {}, instanced batches {}, visible {}",
        stats.draw_calls, stats.instanced_batches, stats.visible_objects
    );

    // Orbit: the benchmark's drag each frame; every tenth frame the selection changes.
    let (mut times, mut gpu, mut select_times) = (Vec::new(), Vec::new(), Vec::new());
    let mut generation = 0;
    let mut selected = false;
    for k in 0..FRAMES {
        cam.orbit(4.0, 0.0);
        let changing = k % 10 == 0 && k > 0;
        let t = Instant::now();
        frame.camera.render_camera = camera(&cam, size());
        if changing {
            selected = !selected;
            if let SurfaceSubmission::Flat(items) = &mut frame.scene.surfaces {
                let mut v: Vec<MeshItem> = items.to_vec();
                v[0].settings.selected = selected;
                *items = v.into();
            }
            frame.interaction.selection_generation += 1;
        }
        r.render_to_texture(device, queue, &view, &frame);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if changing {
            select_times.push(ms);
        } else if k >= 20 {
            times.push(ms);
        }
        let s = r.last_frame_stats();
        if let Some(g) = s.gpu_frame_ms {
            if s.gpu_sample_generation != generation && k >= 20 {
                gpu.push(f64::from(g));
            }
            generation = s.gpu_sample_generation;
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(f64::NAN)
    };
    println!(
        "orbit: {:.2} ms per frame (median), card {:.2} ms; frames changing the selection {:.2} ms",
        median(&mut times),
        median(&mut gpu),
        median(&mut select_times)
    );
    let px = r.render_offscreen(device, queue, &frame, size()[0], size()[1]);
    save(&out.join(format!("vl-orbit-{name}.png")), &px, size());
}

/// SubD while editing: every frame the limit surface and the cage change (worked out beforehand,
/// as Curva's subdivision would), are handed to viewport-lib and drawn. Only handing over and
/// drawing is timed.
fn subd_edit(device: &wgpu::Device, queue: &wgpu::Queue, cage: usize) {
    let frames: Vec<_> = (0..FRAMES).map(|k| scenes::subd(cage, k as f64 * 0.05)).collect();
    let f32s = |v: &[Point]| v.iter().map(|p| p.map(|x| x as f32)).collect::<Vec<_>>();
    let (first, lines) = &frames[0];
    let mut r = renderer(device);
    let mut data = viewport_lib::MeshData::default();
    data.positions = f32s(&first.positions);
    data.normals = f32s(&first.normals);
    data.indices = first.triangles.iter().flatten().copied().collect();
    let id = r.resources_mut().upload_mesh_data(device, &data).unwrap();
    let mut item = MeshItem::default();
    item.mesh_id = id;
    let up = Uploaded {
        items: vec![item],
        lines: vec![polyline(lines, [0.0; 3], 2.0)],
    };
    let mut cam = curva_viewport::Camera::default();
    cam.frame([0.0, 0.0, -800.0], [10_000.0, 10_000.0, 1_100.0]);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: size()[0], height: size()[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut frame = frame_for(&up, camera(&cam, size()), size(), true);
    let (mut still, mut edit) = (Vec::new(), Vec::new());
    for (k, (surface, lines)) in frames.iter().enumerate() {
        let editing = k >= FRAMES / 2;
        let t = Instant::now();
        if editing {
            r.resources_mut()
                .write_mesh_positions_normals(queue, id, &f32s(&surface.positions), &f32s(&surface.normals))
                .unwrap();
            *frame.scene.items_mut::<PolylineItem>() = vec![polyline(lines, [0.0; 3], 2.0)];
        }
        r.render_to_texture(device, queue, &view, &frame);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if k >= 20 && !editing {
            still.push(ms);
        } else if k >= FRAMES / 2 + 5 {
            edit.push(ms);
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    println!(
        "subd cage {cage}: {} triangles; still {:.2} ms per frame, editing {:.2} ms per frame",
        first.triangles.len(),
        median(&mut still),
        median(&mut edit)
    );
}
